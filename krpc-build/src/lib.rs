use convert_case::{Case, Casing};
use std::io::Error;
use std::{fs, path::Path};

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
        .import("crate::schema", "FromResponse");
    module
        .new_struct(&service_name.to_case(Case::Pascal))
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

    let service_impl = module.new_impl(&service_name.to_case(Case::Pascal));
    service_impl
        .new_fn("new")
        .vis("pub")
        .arg("client", "::std::sync::Arc<crate::client::Client>")
        .ret("Self")
        .line("Self { client }");

    let procedures = props.get("procedures").unwrap().as_object().unwrap();

    for (proc_name, def) in procedures.into_iter() {
        let sfn = service_impl
            .new_fn(&proc_name.to_case(Case::Snake))
            .vis("pub")
            .arg_ref_self()
            .allow("dead_code");

        let mut proc_args = Vec::new();
        let params = def
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
            let ty = param.get("type").unwrap().as_object().unwrap();

            proc_args.push(format!("{}.to_argument({})", &name, pos));
            sfn.arg(&name, decode_type(ty));
        }


        let mut ret = String::from("()");
        def.get("return_type").map(|return_value| {
            let ty = return_value.as_object().unwrap();
            ret = decode_type(ty);
            sfn.ret(&ret);
        });

        let body = format!(
            r#"
let request = crate::schema::Request::from(crate::client::Client::proc_call(
    "{service}",
    "{procedure}",
    vec![{args}],
));

let response = self.client.call(request);
dbg!(&response);

<{ret}>::from_response(response)
"#,
            service = service_name,
            procedure = proc_name,
            args = proc_args.join(","),
            ret = ret
        );

        sfn.line(body);

    }

    Ok(())
}

fn decode_type(ty: &serde_json::Map<String, serde_json::Value>) -> String {
    let code = ty.get("code").unwrap().as_str().unwrap();

    match code {
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
    }
}

fn decode_tuple(ty: &serde_json::Map<String, serde_json::Value>) -> String {
    let mut out = Vec::new();
    let types = ty.get("types").unwrap().as_array().unwrap();

    for t in types {
        out.push(decode_type(t.as_object().unwrap()));
    }

    format!("({})", out.join(", "))
}

fn decode_list(ty: &serde_json::Map<String, serde_json::Value>) -> String {
    let types = ty.get("types").unwrap().as_array().unwrap();

    format!(
        "Vec<{}>",
        decode_type(&types.first().unwrap().as_object().unwrap())
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

    let key_name = decode_type(types.get(0).unwrap().as_object().unwrap());
    let value_name = decode_type(types.get(1).unwrap().as_object().unwrap());

    format!("std::collections::HashMap<{}, {}>", key_name, value_name)
}

fn decode_set(ty: &serde_json::Map<String, serde_json::Value>) -> String {
    let types = ty.get("types").unwrap().as_array().unwrap();

    format!(
        "std::collections::HashSet<{}>",
        decode_type(&types.first().unwrap().as_object().unwrap())
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
    #[test]
    fn test_build() {
        crate::build("../service_definitions/", &mut std::io::stdout());
    }
}
