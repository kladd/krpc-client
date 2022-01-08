use convert_case::{Case, Casing};
use std::io::Error;
use std::{fs, path::Path};

pub fn build<O: std::io::Write>(
    service_definitions: impl AsRef<Path>,
    out: &mut O,
) -> Result<(), Error> {
    for def in fs::read_dir(service_definitions)? {
        let def_file = fs::File::open(def.unwrap().path())?;
        let json: serde_json::Value = serde_json::from_reader(def_file)?;

        for (name, props) in json.as_object().unwrap().into_iter() {
            write!(out, "{}", build_json(name, props)?)?;
        }
    }

    Ok(())
}

fn build_json(
    name: &String,
    props_json: &serde_json::Value,
) -> Result<String, Error> {
    let mut mod_buffer: Vec<String> = Vec::new();

    mod_buffer.push(format!("pub mod {} {{", name.to_case(Case::Snake)));
    mod_buffer.push(format!(
        "
pub struct {} {{
    pub client: ::std::sync::Arc<crate::client::Client>
}}
",
        name.to_case(Case::Pascal)
    ));
    let props = props_json.as_object().unwrap();

    let classes = props.get("classes").unwrap().as_object().unwrap();
    for class in classes.keys() {
        mod_buffer
            .push(format!("crate::schema::rpc_object!({});", class).into());
    }
    mod_buffer.push("".into());

    let enums = props.get("enumerations").unwrap().as_object().unwrap();
    for (name, values_json) in enums.into_iter() {
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

        mod_buffer.push(format!(
            "crate::schema::rpc_enum!({}, [{}]);",
            name,
            values.join(", ")
        ));
    }
    mod_buffer.push("".into());
    mod_buffer.push("}".into());

    Ok(mod_buffer.join("\n"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_build() {
        crate::build("../service_definitions/", &mut std::io::stdout());
    }
}
