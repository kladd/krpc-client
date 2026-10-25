use std::{collections::HashSet, env, ffi::OsStr, fs, io, path::Path};

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
        let path = service_definition_path.unwrap().path();
        if !matches!(path.extension().and_then(OsStr::to_str), Some("json")) {
            continue;
        }
        let service_definition_file = fs::File::open(path)?;
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
    let structs = generate_struct_definitions(service_definition, service_name);
    let procedures = generate_procedure_definitions(
        service_definition,
        service_name,
        &q_service_name,
    );

    let arc_client = quote! {
        ::std::sync::Arc<crate::Client>
    };

    quote! {
        #[allow(clippy::type_complexity, clippy::too_many_arguments, deprecated)]
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
            #(#structs)*
            #(#procedures)*
        }
    }
}

fn generate_class_definitions(json: &Value) -> TokenSet<'_> {
    Box::new(
        json.get("classes")
            .unwrap()
            .as_object()
            .unwrap()
            .iter()
            .map(|(name, definition)| {
                let name = format_ident!("{}", name);
                let deprecated = deprecated_attribute(definition);
                quote! {crate::schema::rpc_object!(#deprecated #name);}
            }),
    )
}

fn generate_enum_definitions(json: &Value) -> TokenSet<'_> {
    let enums = json.get("enumerations").unwrap().as_object().unwrap();
    Box::new(enums.into_iter().map(|(name, values)| {
        let name = format_ident!("{name}");
        let deprecated = deprecated_attribute(values);
        let variants = generate_enum_variant_definitions(values);
        quote! {
            crate::schema::rpc_enum!(#deprecated #name, [#(#variants,)*]);
        }
    }))
}

fn generate_struct_definitions<'a>(
    json: &'a Value,
    service_name: &'a str,
) -> TokenSet<'a> {
    let structs = json
        .get("structs")
        .and_then(Value::as_object)
        .into_iter()
        .flatten();
    Box::new(structs.map(move |(name, definition)| {
        let fields = definition.get("fields").unwrap().as_array().unwrap();
        let field_names = fields
            .iter()
            .map(|field| {
                let name = field.get("name").unwrap().as_str().unwrap();
                format_ident!("{}", rewrite_keywords(name.to_case(Case::Snake)))
            })
            .collect::<Vec<_>>();
        let field_types = fields.iter().map(|field| {
            let field_name = field.get("name").unwrap().as_str().unwrap();
            decode_type(field.get("type").unwrap().as_object().unwrap(), false)
                .unwrap_or_else(|e| {
                    panic!("{service_name}.{name}.{field_name}: {e}")
                })
        });
        let deprecated = deprecated_attribute(definition);
        let name = format_ident!("{name}");
        quote! {
            #deprecated
            #[derive(Debug, Clone, PartialEq)]
            pub struct #name {
                #(pub #field_names: #field_types,)*
            }

            impl crate::schema::DecodeUntagged for #name {
                fn decode_untagged(
                    client: ::std::sync::Arc<crate::client::Client>,
                    buf: &[u8],
                ) -> Result<Self, RpcError> {
                    let tuple = <crate::schema::Tuple as crate::schema::DecodeUntagged>::decode_untagged(
                        client.clone(),
                        buf,
                    )?;
                    let mut items = tuple.items.iter();
                    Ok(Self {
                        #(#field_names: crate::schema::DecodeUntagged::decode_untagged(
                            client.clone(),
                            items.next().ok_or(RpcError::Encoding(
                                "structure field out of range".into(),
                            ))?,
                        )?,)*
                    })
                }
            }

            impl crate::schema::EncodeUntagged for #name {
                fn encode_untagged(&self) -> Result<Vec<u8>, RpcError> {
                    crate::schema::EncodeUntagged::encode_untagged(
                        &crate::schema::Tuple {
                            items: vec![
                                #(crate::schema::EncodeUntagged::encode_untagged(
                                    &self.#field_names,
                                )?),*
                            ],
                            ..Default::default()
                        },
                    )
                }
            }
        }
    }))
}

fn deprecated_attribute(json: &Value) -> TokenStream {
    if !json
        .get("deprecated")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return quote!();
    }
    match json
        .get("deprecated_reason")
        .and_then(Value::as_str)
        .filter(|reason| !reason.is_empty())
    {
        Some(reason) => quote!(#[deprecated(note = #reason)]),
        None => quote!(#[deprecated]),
    }
}

fn generate_enum_variant_definitions(json: &Value) -> TokenSet<'_> {
    Box::new(
        json.as_object()
            .unwrap()
            .get("values")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|value| {
                let ident = format_ident!(
                    "{}",
                    value.get("name").unwrap().as_str().unwrap()
                );
                let deprecated = deprecated_attribute(value);
                quote!(#deprecated #ident)
            }),
    )
}

fn generate_procedure_definitions<'a>(
    json: &'a Value,
    service_name: &'a str,
    q_service_name: &'a Ident,
) -> TokenSet<'a> {
    let procedures = json.get("procedures").unwrap().as_object().unwrap();
    let accessors = procedures
        .keys()
        .filter(|name| is_property_accessor(name))
        .map(|name| procedure_method_name(name))
        .collect::<HashSet<_>>();
    Box::new(procedures.iter().map(move |(name, definition)| {
        let (class_name, mut fn_name) = procedure_method_name(name);
        if !is_property_accessor(name)
            && accessors.contains(&(class_name.clone(), fn_name.clone()))
        {
            let params = parameter_names(definition);
            if params.is_empty() {
                panic!(
                    "{service_name}.{name}: method name `{fn_name}` \
                     collides with a property accessor"
                );
            }
            fn_name = format!("{fn_name}_by_{}", params.join("_and_"));
        }
        generate_procedure_definition(
            name,
            definition,
            service_name,
            class_name
                .map(|class| format_ident!("{class}"))
                .unwrap_or_else(|| q_service_name.clone()),
            fn_name,
        )
        .to_token_stream()
    }))
}

fn procedure_method_name(name: &str) -> (Option<String>, String) {
    let name_tokens = name.split('_').collect::<Vec<&str>>();
    let class_name = get_struct(&name_tokens);
    let fn_name = get_fn_name(&name_tokens, &class_name);
    (class_name.map(|class| class.to_string()), fn_name)
}

fn is_property_accessor(name: &str) -> bool {
    let name_tokens = name.split('_').collect::<Vec<&str>>();
    let rest = match get_struct(&name_tokens) {
        Some(_) => &name_tokens[1..],
        None => &name_tokens[..],
    };
    rest.len() == 2 && matches!(rest[0], "get" | "set")
}

fn parameter_names(definition: &Value) -> Vec<String> {
    definition
        .get("parameters")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .map(|param| param.get("name").unwrap().as_str().unwrap())
        .filter(|name| !name.eq_ignore_ascii_case("this"))
        .map(|name| name.to_case(Case::Snake))
        .collect()
}

fn generate_procedure_definition(
    name: &str,
    definition: &Value,
    service_name: &str,
    q_class_name: Ident,
    fn_name: String,
) -> impl ToTokens {
    let Parameters {
        names,
        types,
        as_args,
    } = Parameters::from_json(definition)
        .unwrap_or_else(|e| panic!("{service_name}.{name}: {e}"));

    let call_name = format_ident!("{fn_name}_call");
    let stream_name = format_ident!("{fn_name}_stream");
    let fn_name = format_ident!("{}", rewrite_keywords(fn_name));
    let ret = get_return_type(definition)
        .unwrap_or_else(|e| panic!("{service_name}.{name}: {e}"));
    let deprecated = deprecated_attribute(definition);
    if env::var("CARGO_FEATURE_TOKIO").is_ok() {
        quote! {
            impl #q_class_name {
                #deprecated
                pub(crate) fn #call_name(
                    &self, #(#names: #types),*
                ) -> Result<crate::schema::ProcedureCall, RpcError> {
                    Ok(crate::client::Client::proc_call(
                        #service_name,
                        #name,
                        vec![#(#as_args),*]
                    ))
                }

                #deprecated
                pub async fn #stream_name(
                    &self, #(#names: #types),*
                ) -> Result<crate::stream::Stream<#ret>, RpcError> {
                    crate::stream::Stream::new(
                        self.client.clone(),
                        self.#call_name(#(#names),*)?
                    ).await
                }

                #deprecated
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
                #deprecated
                pub(crate) fn #call_name(
                    &self, #(#names: #types),*
                ) -> Result<crate::schema::ProcedureCall, RpcError> {
                    Ok(crate::client::Client::proc_call(
                        #service_name,
                        #name,
                        vec![#(#as_args),*]
                    ))
                }

                #deprecated
                pub fn #stream_name(
                    &self, #(#names: #types),*
                ) -> Result<crate::stream::Stream<#ret>, RpcError> {
                    crate::stream::Stream::new(
                        self.client.clone(),
                        self.#call_name(#(#names),*)?
                    )
                }

                #deprecated
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
    fn from_json(json: &Value) -> Result<Self, String> {
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

                let mut ty =
                    param.get("type").unwrap().as_object().unwrap().clone();
                if param.get("nullable").and_then(Value::as_bool) == Some(true)
                {
                    ty.insert("nullable".into(), Value::Bool(true));
                }
                types.push(decode_type(&ty, true)?);
            }
        }
        Ok(Self {
            names,
            types,
            as_args: args,
        })
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
) -> Result<TokenStream, String> {
    let code = ty.get("code").unwrap().as_str().unwrap();

    let mut type_stream = match code {
        "STRING" => quote!(String),
        "SINT32" => quote!(i32),
        "SINT64" => quote!(i64),
        "UINT32" => quote!(u32),
        "UINT64" => quote!(u64),
        "BOOL" => quote!(bool),
        "FLOAT" => quote!(f32),
        "DOUBLE" => quote!(f64),
        // TODO(kladd): maybe not Vec<u8>
        "BYTES" => quote!(Vec<u8>),
        "TUPLE" => decode_tuple(ty)?,
        "LIST" => decode_list(ty)?,
        "SET" => decode_set(ty)?,
        "DICTIONARY" => decode_dictionary(ty)?,
        "ENUMERATION" => decode_class(ty),
        "CLASS" => decode_class(ty),
        "STRUCT" => decode_class(ty),
        "EVENT" => quote!(crate::schema::Event),
        "PROCEDURE_CALL" => quote!(crate::schema::ProcedureCall),
        "STREAM" => quote!(crate::schema::Stream),
        "SERVICES" => quote!(crate::schema::Services),
        "STATUS" => quote!(crate::schema::Status),
        _ => return Err(format!("unsupported type code `{code}`")),
    };

    if borrow {
        type_stream = match code {
            "CLASS" => quote!(&#type_stream),
            _ => type_stream,
        }
    };

    if ty.get("nullable").and_then(Value::as_bool) == Some(true) {
        type_stream = quote!(Option<#type_stream>)
    }

    Ok(type_stream)
}

fn get_return_type(definition: &Value) -> Result<TokenStream, String> {
    let Some(return_value) = definition.get("return_type") else {
        return Ok(quote!(()));
    };
    let mut ty = return_value.as_object().unwrap().clone();
    if definition
        .get("return_is_nullable")
        .and_then(Value::as_bool)
        == Some(true)
    {
        ty.insert("nullable".into(), Value::Bool(true));
    }
    decode_type(&ty, false)
}

fn decode_tuple(
    ty: &serde_json::Map<String, Value>,
) -> Result<TokenStream, String> {
    let types = ty
        .get("types")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .map(|ty| decode_type(ty.as_object().unwrap(), false))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(quote! {(#(#types),*)})
}

fn decode_list(
    ty: &serde_json::Map<String, Value>,
) -> Result<TokenStream, String> {
    let types = ty.get("types").unwrap().as_array().unwrap();
    let ty = decode_type(types.first().unwrap().as_object().unwrap(), false)?;
    Ok(quote!( Vec<#ty> ))
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

fn decode_dictionary(
    ty: &serde_json::Map<String, Value>,
) -> Result<TokenStream, String> {
    let types = ty.get("types").unwrap().as_array().unwrap();

    let key_name =
        decode_type(types.first().unwrap().as_object().unwrap(), false)?;
    let value_name =
        decode_type(types.get(1).unwrap().as_object().unwrap(), false)?;

    Ok(quote!(std::collections::HashMap<#key_name, #value_name>))
}

fn decode_set(
    ty: &serde_json::Map<String, Value>,
) -> Result<TokenStream, String> {
    let types = ty.get("types").unwrap().as_array().unwrap();
    let ty = decode_type(types.first().unwrap().as_object().unwrap(), false)?;
    Ok(quote!(
        std::collections::HashSet<#ty>
    ))
}

fn rewrite_keywords(sample: String) -> String {
    match sample.as_str() {
        "as" | "break" | "const" | "continue" | "else" | "enum" | "extern"
        | "false" | "fn" | "for" | "if" | "impl" | "in" | "let" | "loop"
        | "match" | "mod" | "move" | "mut" | "pub" | "ref" | "return"
        | "static" | "struct" | "trait" | "true" | "type" | "unsafe"
        | "use" | "where" | "while" | "async" | "await" | "dyn"
        | "abstract" | "become" | "box" | "do" | "final" | "macro"
        | "override" | "priv" | "typeof" | "unsized" | "virtual" | "yield"
        | "try" | "gen" => format!("r#{sample}"),
        _ => sample,
    }
}
