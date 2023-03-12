use std::{fs, io::Error, path::Path};

use codegen::Function;
use convert_case::{Case, Casing};
use serde_json::{Map, Value};

/// Generate source code from JSON service definitions.
///
/// Creates a module for each RPC service that contains all
/// type and function definitions for that service.
///
/// # Examples
/// ```
/// {
///   "SpaceCenter": {
///     "procedures": {
///       "get_ActiveVessel": {
///         "paramters": [],
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
/// ```
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
pub fn build<O: std::io::Write>(
    service_definitions: impl AsRef<Path>,
    out: &mut O,
) -> Result<(), Error> {
    let mut scope = codegen::Scope::new();
    for def in fs::read_dir(service_definitions)? {
        let def_file = fs::File::open(def.unwrap().path())?;
        let json: serde_json::Value = serde_json::from_reader(def_file)?;

        for (name, props_json) in json.as_object().unwrap().into_iter() {
            let mut service = RpcService::new(&mut scope, name);

            let props = props_json.as_object().unwrap();

            let classes = props.get("classes").unwrap().as_object().unwrap();
            for class in classes.keys() {
                service.define_class(class)
            }

            let enums = props.get("enumerations").unwrap().as_object().unwrap();
            for (enum_name, values_json) in enums.into_iter() {
                service.define_enum(enum_name, values_json);
            }

            let procedures =
                props.get("procedures").unwrap().as_object().unwrap();
            for (procedure_name, definition) in procedures.into_iter() {
                service.define_call_procedure(procedure_name, definition);
                service.define_stream_procedure(procedure_name, definition);
                service.define_procedure(procedure_name, definition);
            }
        }
    }

    write!(out, "{}", scope.to_string())
}

struct RpcService<'a> {
    name: String,
    module: &'a mut codegen::Module,
}

struct RpcArgs {
    // Args formatted to pass directly to a function.
    call_local: String,

    // Args formatted to pass directly to a ProcedureCall.
    call_remote: String,
}

impl<'a> RpcService<'a> {
    const IMPORTS: &[(&'static str, &'static str)] = &[
        ("crate::schema", "ToArgument"),
        ("crate::schema", "FromResponse"),
        ("crate::error", "RpcError"),
    ];

    fn new(scope: &'a mut codegen::Scope, service_name: &str) -> Self {
        let module = scope
            .new_module(&service_name.to_case(Case::Snake))
            .attr("allow(clippy::type_complexity)")
            .vis("pub");

        for (path, type_name) in Self::IMPORTS {
            module.import(path, type_name);
        }

        module
            .new_struct(service_name)
            .vis("pub")
            .field("pub client", "::std::sync::Arc<crate::client::Client>")
            .allow("dead_code");

        // TODO: Remove new? Or derive it.
        module
            .new_impl(service_name)
            .new_fn("new")
            .vis("pub")
            .arg("client", "::std::sync::Arc<crate::client::Client>")
            .ret("Self")
            .line("Self { client }");

        Self {
            name: service_name.to_string(),
            module,
        }
    }

    fn define_class(&mut self, name: &str) {
        self.module
            .scope()
            .raw(&format!("crate::schema::rpc_object!({});", name));
    }

    fn define_enum(&mut self, name: &str, values: &Value) {
        let values = {
            let mut v = Vec::new();
            for d in values
                .as_object()
                .unwrap()
                .get("values")
                .unwrap()
                .as_array()
                .unwrap()
                .iter()
            {
                v.push(d.get("name").unwrap().as_str().unwrap())
            }
            v
        };

        self.module.scope().raw(&format!(
            "crate::schema::rpc_enum!({}, [{}]);",
            name,
            values.join(", ")
        ));
    }

    fn define_procedure(&mut self, name: &str, definition: &Value) {
        let name_tokens = name.split('_').collect::<Vec<&str>>();
        let class_name =
            get_struct(&name_tokens).unwrap_or_else(|| self.name.clone());
        let class = self.module.new_impl(&class_name);

        let fn_name = get_fn_name(&name_tokens);
        let fn_block = class
            .new_fn(&fn_name)
            .vis("pub")
            .arg_ref_self()
            .allow("dead_code");

        let RpcArgs {
            call_local,
            call_remote: _,
        } = fn_set_args(fn_block, definition);

        let ret = get_return_type(definition);
        fn_block.ret(format!("Result<{}, RpcError>", ret));

        let body = format!(
            r#"
        let request =
        crate::schema::Request::from(self.{fn_name}_call({call_local})?);

        let response = self.client.call(request)?;

        <{ret}>::from_response(response, self.client.clone())"#
        );

        fn_block.line(body);
    }

    fn define_call_procedure(&mut self, proc_name: &str, definition: &Value) {
        let name_tokens = proc_name.split('_').collect::<Vec<&str>>();
        let class_name =
            get_struct(&name_tokens).unwrap_or_else(|| self.name.clone());
        let class = self.module.new_impl(&class_name);

        let fn_name =
            format!("{}_call", get_fn_name(&proc_name.split('_').collect()));
        let fn_block = class
            .new_fn(&fn_name)
            .vis("pub(crate)")
            .arg_ref_self()
            .allow("dead_code");
        let RpcArgs {
            call_local: _,
            call_remote,
        } = fn_set_args(fn_block, definition);
        fn_block.ret("Result<crate::schema::ProcedureCall, RpcError>");

        let body = format!(
            r#"
        Ok(crate::client::Client::proc_call(
            "{service}",
            "{proc_name}",
            vec![{call_remote}]
        ))"#,
            service = self.name,
        );

        fn_block.line(body);
    }

    fn define_stream_procedure(&mut self, proc_name: &str, definition: &Value) {
        let name_tokens = proc_name.split('_').collect::<Vec<&str>>();
        let class_name =
            get_struct(&name_tokens).unwrap_or_else(|| self.name.clone());
        let class = self.module.new_impl(&class_name);

        let fn_base_name = get_fn_name(&proc_name.split('_').collect());
        let fn_name = format!("{}_stream", fn_base_name);
        let fn_block = class
            .new_fn(&fn_name)
            .vis("pub")
            .arg_ref_self()
            .allow("dead_code");
        let RpcArgs {
            call_local,
            call_remote: _,
        } = fn_set_args(fn_block, definition);
        let ret = get_return_type(definition);
        fn_block
            .ret(format!("Result<crate::stream::Stream<{}>, RpcError>", ret));

        let body = format!(
            r#"crate::stream::Stream::new(self.client.clone(), self.{fn_base_name}_call({call_local})?)"#,
        );

        fn_block.line(body);
    }
}

fn fn_set_args(fn_block: &mut Function, definition: &Value) -> RpcArgs {
    let mut arg_names = Vec::new();
    let mut arg_values = Vec::new();

    let params = get_params(definition);
    for (pos, param_json) in params.iter().enumerate() {
        let param = param_json.as_object().unwrap();
        let name = get_param_name(param);

        if name.eq_ignore_ascii_case("this") {
            arg_values.push(format!("self.to_argument({})?", pos));
        } else {
            let ty = decode_type(
                param.get("type").unwrap().as_object().unwrap(),
                true,
            );
            arg_names.push(name.clone());
            arg_values.push(format!("{}.to_argument({})?", &name, pos));
            fn_block.arg(&name, ty);
        }
    }

    RpcArgs {
        call_local: arg_names.join(","),
        call_remote: arg_values.join(","),
    }
}

fn get_return_type(definition: &Value) -> String {
    let mut ret = String::from("()");
    if let Some(return_value) = definition.get("return_type") {
        let ty = return_value.as_object().unwrap();
        ret = decode_type(ty, false);
    }
    ret
}

fn get_params(json: &Value) -> &Vec<Value> {
    json.as_object()
        .unwrap()
        .get("parameters")
        .unwrap()
        .as_array()
        .unwrap()
}

fn get_param_name(json: &Map<String, Value>) -> String {
    rewrite_keywords(
        json.get("name")
            .unwrap()
            .as_str()
            .unwrap()
            .to_case(Case::Snake),
    )
}

fn get_struct(proc_tokens: &Vec<&str>) -> Option<String> {
    proc_tokens
        .first()
        .filter(|segment| {
            proc_tokens.len() > 1 && !segment.is_case(Case::Lower)
        })
        .map(|segment| String::from(*segment))
}

fn get_fn_name(proc_tokens: &Vec<&str>) -> String {
    match get_struct(proc_tokens) {
        Some(_) => &proc_tokens[1..],
        None => &proc_tokens[..],
    }
    .join("_")
    .to_case(Case::Snake)
}

fn decode_type(
    ty: &serde_json::Map<String, serde_json::Value>,
    borrow: bool,
) -> String {
    let code = ty.get("code").unwrap().as_str().unwrap();

    let str = match code {
        "STRING" => "String".to_string(),
        "SINT32" => "i32".to_string(),
        "UINT32" => "u32".into(),
        "UINT64" => "u64".into(),
        "BOOL" => "bool".to_string(),
        "FLOAT" => "f32".to_string(),
        "DOUBLE" => "f64".to_string(),
        // TODO(kladd): maybe not Vec<u8>
        "BYTES" => "Vec<u8>".to_string(),
        "TUPLE" => decode_tuple(ty),
        "LIST" => decode_list(ty),
        "SET" => decode_set(ty),
        "DICTIONARY" => decode_dictionary(ty),
        "ENUMERATION" => decode_class(ty),
        "CLASS" => decode_class(ty),
        "EVENT" => "crate::schema::Event".into(),
        "PROCEDURE_CALL" => "crate::schema::ProcedureCall".into(),
        "STREAM" => "crate::schema::Stream".into(),
        "SERVICES" => "crate::schema::Services".into(),
        "STATUS" => "crate::schema::Status".into(),
        _ => "".to_string(),
    };

    if borrow {
        return match code {
            "CLASS" => format!("&{}", str),
            _ => str,
        };
    }

    str
}

fn decode_tuple(ty: &serde_json::Map<String, serde_json::Value>) -> String {
    let mut out = Vec::new();
    let types = ty.get("types").unwrap().as_array().unwrap();

    for t in types {
        out.push(decode_type(t.as_object().unwrap(), false));
    }

    format!("({})", out.join(", "))
}

fn decode_list(ty: &serde_json::Map<String, serde_json::Value>) -> String {
    let types = ty.get("types").unwrap().as_array().unwrap();

    format!(
        "Vec<{}>",
        decode_type(types.first().unwrap().as_object().unwrap(), false)
    )
}

fn decode_class(ty: &serde_json::Map<String, serde_json::Value>) -> String {
    let service = ty.get("service").unwrap().as_str().unwrap();
    let name = ty.get("name").unwrap().as_str().unwrap();

    format!(
        "crate::services::{}::{}",
        service.to_case(Case::Snake),
        name
    )
}

fn decode_dictionary(
    ty: &serde_json::Map<String, serde_json::Value>,
) -> String {
    let types = ty.get("types").unwrap().as_array().unwrap();

    let key_name =
        decode_type(types.get(0).unwrap().as_object().unwrap(), false);
    let value_name =
        decode_type(types.get(1).unwrap().as_object().unwrap(), false);

    format!("std::collections::HashMap<{}, {}>", key_name, value_name)
}

fn decode_set(ty: &serde_json::Map<String, serde_json::Value>) -> String {
    let types = ty.get("types").unwrap().as_array().unwrap();

    format!(
        "std::collections::HashSet<{}>",
        decode_type(types.first().unwrap().as_object().unwrap(), false)
    )
}

fn rewrite_keywords(sample: String) -> String {
    match sample.as_str() {
        "type" => "r#type".into(),
        _ => sample,
    }
}

#[cfg(test)]
mod tests {
    use crate::get_fn_name;

    #[test]
    fn test_get_fn_name() {
        assert_eq!(
            get_fn_name(&"SpaceCenter_get_ActiveVessel".split("_").collect()),
            String::from("get_active_vessel")
        );
    }
}
