use std::{env, fs, io, path::Path};

use convert_case::{Case, Casing};
use proc_macro2::TokenStream;
use quote::{format_ident, quote, ToTokens};
use serde_json::Value;
use syn::Ident;

type TokenSet<'a> = Box<dyn Iterator<Item = TokenStream> + 'a>;

/// Generate source code from JSON service definitions.
///
/// Creates a module for each RPC service that contains all
/// type and function definitions for that service.
///
/// # Examples
/// ```json
/// {
///   "SpaceCenter": {
///     "procedures": {
///       "get_ActiveVessel": {
///         "parameters": [],
///         "return_type": {
///           "code": "CLASS",
///           "service": "SpaceCenter",
///           "name": "Vessel"
///         }
///       }
///     }
///   }
/// }
/// ```
/// becomes
/// ```rust
/// use std::sync::Arc;
///
/// use crate::{client::Client, error::RpcError, schema::rpc_object};
///
/// pub mod space_center {
///     rpc_object!(Vessel);
///
///     pub struct SpaceCenter {
///         pub client: Arc<Client>,
///     }
///
///     impl SpaceCenter {
///         pub fn get_active_vessel() -> Result<Vessel, RpcError> { ... }
///     }
/// }
/// ```
pub fn build<O: io::Write>(
    service_definitions: impl AsRef<Path>,
    out: &mut O,
) -> Result<(), io::Error> {
    for service_definition_path in fs::read_dir(service_definitions)? {
        let service_definition_file =
            fs::File::open(service_definition_path.unwrap().path())?;
        let service_definition_json: Value =
            serde_json::from_reader(service_definition_file)?;

        for (service_name, service_definition) in
            service_definition_json.as_object().unwrap().into_iter()
        {
            let service_module =
                generate_module_definition(service_name, service_definition);

            #[cfg(feature = "fmt")]
            let service_module =
                prettyplease::unparse(&syn::parse2(service_module).unwrap());

            write!(out, "{service_module}").unwrap();
        }
    }
    Ok(())
}

fn generate_module_definition(
    service_name: &str,
    service_definition: &Value,
) -> TokenStream {
    let service_mod_name =
        format_ident!("{}", service_name.to_case(Case::Snake));
    let q_service_name = format_ident!("{}", service_name);

    let classes = generate_class_definitions(service_definition);
    let enums = generate_enum_definitions(service_definition);
    let procedures = generate_procedure_definitions(
        service_definition,
        service_name,
        &q_service_name,
    );

    let arc_client = quote! {
        ::std::sync::Arc<crate::Client>
    };

    quote! {
        #[allow(clippy::type_complexity)]
        pub mod #service_mod_name {
            use crate::{
                schema::{ToArgument, FromResponse},
                error::RpcError,
            };

            #[derive(Clone)]
            pub struct #q_service_name {
                pub client: #arc_client,
            }

            impl #q_service_name {
                pub fn new(client: #arc_client) -> Self {
                    Self { client }
                }
            }

            #(#classes)*
            #(#enums)*
            #(#procedures)*
        }
    }
}

fn generate_class_definitions(json: &Value) -> TokenSet {
    Box::new(
        json.get("classes")
            .unwrap()
            .as_object()
            .unwrap()
            .keys()
            .map(|k| format_ident!("{}", k))
            .map(|name| quote! {crate::schema::rpc_object!(#name);}),
    )
}

fn generate_enum_definitions(json: &Value) -> TokenSet {
    let enums = json.get("enumerations").unwrap().as_object().unwrap();
    Box::new(enums.into_iter().map(|(name, values)| {
        let name = format_ident!("{name}");
        let variants = generate_enum_variant_definitions(values);
        quote! {
            crate::schema::rpc_enum!(#name, [#(#variants,)*]);
        }
    }))
}

fn generate_enum_variant_definitions(json: &Value) -> TokenSet {
    Box::new(
        json.as_object()
            .unwrap()
            .get("values")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|value| {
                format_ident!(
                    "{}",
                    value.get("name").unwrap().as_str().unwrap()
                )
            })
            .map(|ident| quote!(#ident)),
    )
}

fn generate_procedure_definitions<'a>(
    json: &'a Value,
    service_name: &'a str,
    q_service_name: &'a Ident,
) -> TokenSet<'a> {
    Box::new(
        json.get("procedures")
            .unwrap()
            .as_object()
            .unwrap()
            .iter()
            .map(|(name, definition)| {
                generate_procedure_definition(
                    name,
                    definition,
                    service_name,
                    q_service_name,
                )
                .to_token_stream()
            }),
    )
}

fn generate_procedure_definition(
    name: &str,
    definition: &Value,
    service_name: &str,
    q_service_name: &Ident,
) -> impl ToTokens {
    let name_tokens = name.split('_').collect::<Vec<&str>>();
    let class_name = get_struct(&name_tokens);

    let fn_name = get_fn_name(&name_tokens, &class_name);
    let q_class_name = class_name.unwrap_or_else(|| q_service_name.clone());

    let Parameters {
        names,
        types,
        as_args,
    } = Parameters::from_json(definition);

    let call_name = format_ident!("{fn_name}_call");
    let stream_name = format_ident!("{fn_name}_stream");
    let fn_name = format_ident!("{fn_name}");
    let ret = get_return_type(definition);
    if env::var("CARGO_FEATURE_ASYNC").is_ok() {
        quote! {
            impl #q_class_name {
                pub(crate) fn #call_name(
                    &self, #(#names: #types),*
                ) -> Result<crate::schema::ProcedureCall, RpcError> {
                    Ok(crate::client::Client::proc_call(
                        #service_name,
                        #name,
                        vec![#(#as_args),*]
                    ))
                }

                pub async fn #stream_name(
                    &self, #(#names: #types),*
                ) -> Result<crate::stream::Stream<#ret>, RpcError> {
                    crate::stream::Stream::new(
                        self.client.clone(),
                        self.#call_name(#(#names),*)?
                    ).await
                }

                pub async fn #fn_name(
                    &self, #(#names: #types),*
                ) -> Result<#ret, RpcError> {
                    let request = crate::schema::Request::from(
                        self.#call_name(#(#names),*)?);
                    let response = self.client.call(request).await?;

                    <#ret>::from_response(response, self.client.clone())
                }
            }
        }
    } else {
        quote! {
            impl #q_class_name {
                pub(crate) fn #call_name(
                    &self, #(#names: #types),*
                ) -> Result<crate::schema::ProcedureCall, RpcError> {
                    Ok(crate::client::Client::proc_call(
                        #service_name,
                        #name,
                        vec![#(#as_args),*]
                    ))
                }

                pub fn #stream_name(
                    &self, #(#names: #types),*
                ) -> Result<crate::stream::Stream<#ret>, RpcError> {
                    crate::stream::Stream::new(
                        self.client.clone(),
                        self.#call_name(#(#names),*)?
                    )
                }

                pub fn #fn_name(
                    &self, #(#names: #types),*
                ) -> Result<#ret, RpcError> {
                    let request = crate::schema::Request::from(
                        self.#call_name(#(#names),*)?);
                    let response = self.client.call(request)?;

                    <#ret>::from_response(response, self.client.clone())
                }
            }
        }
    }
}

fn get_struct(proc_tokens: &[&str]) -> Option<Ident> {
    proc_tokens
        .first()
        .filter(|segment| {
            proc_tokens.len() > 1 && !segment.is_case(Case::Lower)
        })
        .map(|segment| format_ident!("{segment}"))
}

struct Parameters {
    names: Vec<Ident>,
    as_args: Vec<TokenStream>,
    types: Vec<TokenStream>,
}

impl Parameters {
    fn from_json(json: &Value) -> Self {
        let mut names = Vec::new();
        let mut types = Vec::new();
        let mut args = Vec::new();

        let params = json
            .as_object()
            .unwrap()
            .get("parameters")
            .unwrap()
            .as_array()
            .unwrap();

        for (pos, param_json) in params.iter().enumerate() {
            let param = param_json.as_object().unwrap();
            let name: String = rewrite_keywords(
                param
                    .get("name")
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_case(Case::Snake),
            );
            if name.eq_ignore_ascii_case("this") {
                args.push(quote! { self.to_argument(#pos as u32)? });
            } else {
                let name = format_ident!("{name}");
                args.push(quote!(#name.to_argument(#pos as u32)?));
                names.push(name);

                let nullable = param
                    .get("nullable")
                    .map(|b| b.as_bool().unwrap())
                    .unwrap_or(false);
                types.push(decode_type(
                    param.get("type").unwrap().as_object().unwrap(),
                    true,
                    nullable,
                ));
            }
        }
        Self {
            names,
            types,
            as_args: args,
        }
    }
}

fn get_fn_name<T>(proc_tokens: &[&str], class: &Option<T>) -> String {
    match class {
        Some(_) => &proc_tokens[1..],
        None => proc_tokens,
    }
    .join("_")
    .to_case(Case::Snake)
}

fn decode_type(
    ty: &serde_json::Map<String, Value>,
    borrow: bool,
    nullable: bool,
) -> TokenStream {
    let code = ty.get("code").unwrap().as_str().unwrap();

    let mut type_stream = match code {
        "STRING" => quote!(String),
        "SINT32" => quote!(i32),
        "UINT32" => quote!(u32),
        "UINT64" => quote!(u64),
        "BOOL" => quote!(bool),
        "FLOAT" => quote!(f32),
        "DOUBLE" => quote!(f64),
        // TODO(kladd): maybe not Vec<u8>
        "BYTES" => quote!(Vec<u8>),
        "TUPLE" => decode_tuple(ty),
        "LIST" => decode_list(ty),
        "SET" => decode_set(ty),
        "DICTIONARY" => decode_dictionary(ty),
        "ENUMERATION" => decode_class(ty),
        "CLASS" => decode_class(ty),
        "EVENT" => quote!(crate::schema::Event),
        "PROCEDURE_CALL" => quote!(crate::schema::ProcedureCall),
        "STREAM" => quote!(crate::schema::Stream),
        "SERVICES" => quote!(crate::schema::Services),
        "STATUS" => quote!(crate::schema::Status),
        _ => quote!(),
    };

    if borrow {
        type_stream = match code {
            "CLASS" => quote!(&#type_stream),
            _ => type_stream,
        }
    };

    if nullable {
        type_stream = quote!(Option<#type_stream>)
    }

    type_stream
}

fn get_return_type(definition: &Value) -> TokenStream {
    let mut ret = quote!(());
    if let Some(return_value) = definition.get("return_type") {
        let nullable = definition
            .get("return_is_nullable")
            .map(|b| b.as_bool().unwrap())
            .unwrap_or(false);
        let ty = return_value.as_object().unwrap();
        ret = decode_type(ty, false, nullable);
    }
    ret
}

fn decode_tuple(ty: &serde_json::Map<String, Value>) -> TokenStream {
    let types = ty
        .get("types")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .map(|ty| decode_type(ty.as_object().unwrap(), false, false));
    quote! {(#(#types),*)}
}

fn decode_list(ty: &serde_json::Map<String, Value>) -> TokenStream {
    let types = ty.get("types").unwrap().as_array().unwrap();
    let ty =
        decode_type(types.first().unwrap().as_object().unwrap(), false, false);
    quote!( Vec<#ty> )
}

fn decode_class(ty: &serde_json::Map<String, Value>) -> TokenStream {
    let service = format_ident!(
        "{}",
        ty.get("service")
            .unwrap()
            .as_str()
            .unwrap()
            .to_case(Case::Snake)
    );
    let name = format_ident!("{}", ty.get("name").unwrap().as_str().unwrap());

    quote!(
        crate::services::#service::#name
    )
}

fn decode_dictionary(ty: &serde_json::Map<String, Value>) -> TokenStream {
    let types = ty.get("types").unwrap().as_array().unwrap();

    let key_name =
        decode_type(types.first().unwrap().as_object().unwrap(), false, false);
    let value_name =
        decode_type(types.get(1).unwrap().as_object().unwrap(), false, false);

    quote!(std::collections::HashMap<#key_name, #value_name>)
}

fn decode_set(ty: &serde_json::Map<String, Value>) -> TokenStream {
    let types = ty.get("types").unwrap().as_array().unwrap();
    let ty =
        decode_type(types.first().unwrap().as_object().unwrap(), false, false);
    quote!(
        std::collections::HashSet<#ty>
    )
}

fn rewrite_keywords(sample: String) -> String {
    match sample.as_str() {
        "type" => "r#type".into(),
        _ => sample,
    }
}
