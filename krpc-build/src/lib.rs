use std::{fs, io::Error, path::Path};

use convert_case::{Case, Casing};

pub fn build<O: std::io::Write>(
    service_definitions: impl AsRef<Path>,
    out: &mut O,
) -> Result<(), Error> {
    let mut scope = codegen::Scope::new();
    for def in fs::read_dir(service_definitions)? {
        let def_file = fs::File::open(def.unwrap().path())?;
        let json: serde_json::Value = serde_json::from_reader(def_file)?;

        for (name, props) in json.as_object().unwrap().into_iter() {
            build_json(name, props, &mut scope)?;
        }
    }

    write!(out, "{}", scope.to_string())
}

fn build_json(
    service_name: &String,
    props_json: &serde_json::Value,
    root: &mut codegen::Scope,
) -> Result<(), Error> {
    let module = root
        .new_module(&service_name.to_case(Case::Snake))
        .vis("pub")
        .import("crate::schema", "ToArgument")
        .import("crate::schema", "FromResponse")
        .import("crate::error", "RpcError");
    module
        .new_struct(&service_name)
        .vis("pub")
        .field("pub client", "::std::sync::Arc<crate::client::Client>")
        .allow("dead_code");

    let props = props_json.as_object().unwrap();

    let classes = props.get("classes").unwrap().as_object().unwrap();
    for class in classes.keys() {
        module
            .scope()
            .raw(&format!("crate::schema::rpc_object!({});", class));
    }

    let enums = props.get("enumerations").unwrap().as_object().unwrap();
    for (enum_name, values_json) in enums.into_iter() {
        let values = {
            let mut v = Vec::new();
            for d in values_json
                .as_object()
                .unwrap()
                .get("values")
                .unwrap()
                .as_array()
                .unwrap()
                .into_iter()
            {
                v.push(d.get("name").unwrap().as_str().unwrap())
            }
            v
        };

        module.scope().raw(&format!(
            "crate::schema::rpc_enum!({}, [{}]);",
            enum_name,
            values.join(", ")
        ));
    }

    let service_impl = module.new_impl(&service_name);
    service_impl
        .new_fn("new")
        .vis("pub")
        .arg("client", "::std::sync::Arc<crate::client::Client>")
        .ret("Self")
        .line("Self { client }");

    let procedures = props.get("procedures").unwrap().as_object().unwrap();

    for (procedure, procedure_definition) in procedures.into_iter() {
        let procedure_name_tokens = procedure.split("_").collect::<Vec<&str>>();

        let impl_struct_name =
            get_struct(&procedure_name_tokens).unwrap_or(service_name.clone());
        let struct_impl = module.new_impl(&impl_struct_name);

        let procedure_fn_name = get_fn_name(&procedure_name_tokens);
        let procedure_fn = struct_impl
            .new_fn(&procedure_fn_name)
            .vis("pub")
            .arg_ref_self()
            .allow("dead_code");

        let mut procedure_args = Vec::new();
        let params = procedure_definition
            .as_object()
            .unwrap()
            .get("parameters")
            .unwrap()
            .as_array()
            .unwrap();
        for (pos, p) in params.iter().enumerate() {
            let param = p.as_object().unwrap();
            let name = rewrite_keywords(
                param
                    .get("name")
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_case(Case::Snake),
            );

            if name.eq_ignore_ascii_case("this") {
                procedure_args.push(format!("self.to_argument({})?", pos));
            } else {
                let ty = param.get("type").unwrap().as_object().unwrap();
                procedure_args.push(format!("{}.to_argument({})?", &name, pos));
                procedure_fn.arg(&name, decode_type(ty, true));
            }
        }

        let mut ret = String::from("()");
        procedure_definition.get("return_type").map(|return_value| {
            let ty = return_value.as_object().unwrap();
            ret = decode_type(ty, false);
        });
        procedure_fn.ret(format!("Result<{}, RpcError>", ret));

        let body = format!(
            r#"
        let request =
        crate::schema::Request::from(crate::client::Client::proc_call(
            "{service}",
            "{procedure}",
            vec![{args}],
        ));

        let response = self.client.call(request)?;

        <{ret}>::from_response(response, self.client.clone())
        "#,
            service = service_name,
            procedure = procedure,
            args = procedure_args.join(","),
            ret = ret
        );

        procedure_fn.line(body);
    }

    Ok(())
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
        "TUPLE" => decode_tuple(&ty),
        "LIST" => decode_list(&ty),
        "SET" => decode_set(&ty),
        "DICTIONARY" => decode_dictionary(&ty),
        "ENUMERATION" => decode_class(&ty),
        "CLASS" => decode_class(&ty),
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
        decode_type(&types.first().unwrap().as_object().unwrap(), false)
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
        decode_type(&types.first().unwrap().as_object().unwrap(), false)
    )
}

fn rewrite_keywords(sample: String) -> String {
    match sample.as_str() {
        "type" => "r#type".into(),
        _ => sample,
    }
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

    #[test]
    fn test_build() {
        crate::build("../service_definitions/", &mut std::io::stdout());
    }
}
