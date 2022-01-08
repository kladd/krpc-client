use std::{path::Path, fs};
use std::io::Error;
use convert_case::{Casing, Case};

// use std::io::Write;

// pub fn build<T: ::std::io::Write>(out: &mut T, service: String) {
//     let new_struct = format!("
// struct {} {{
//     client: ::std::async::Arc<crate::client::Client>
// }}", service);
//     println!("{}", new_struct);

//     write!(out, "hello");
// }

pub fn build(service_definitions: impl AsRef<Path>) -> Result<(), Error> {
    for def in fs::read_dir(service_definitions)? {
	let def_file = fs::File::open(def.unwrap().path())?;
	let json: serde_json::Value = serde_json::from_reader(def_file)?;

	for (name, props) in json.as_object().unwrap().into_iter() {
	    build_json(name, props)?;
	}
    }

    Ok(())
}

pub fn build_json(name: &String, props_json: &serde_json::Value) -> Result<(), Error> {
    println!("
// {}.rs
struct {} {{
    client: ::std::async::Arc<crate::client::Client>
}}
", name.to_case(Case::Snake), name.to_case(Case::Pascal));
    let props = props_json.as_object().unwrap();

    let classes = props.get("classes").unwrap().as_object().unwrap();
    for class in classes.keys() {
	println!("crate::schema::rpc_object!({});", class);
    }
    println!("");

    let enums = props.get("enumerations").unwrap().as_object().unwrap();
    for (name, values_json) in enums.into_iter() {
	// dbg!(&values_json);
	let values = {
	    let mut v = Vec::new();
	    for d in values_json.as_object().unwrap().get("values").unwrap().as_array().unwrap().into_iter() {
		v.push(d.get("name").unwrap().as_str().unwrap())
	    }
	    v
	};

	println!("crate::schema::rpc_enum!({}, [{}]);", name, values.join(", "));
    }
    println!("");

    // class_list = props.as_object().get
    // for (class, class_def) in props.as_object().unwrap().get("classes").unwrap().as_object().unwrap().into_iter() {
    // 	println!("crate::schema::rpc_object!({});", class);

    // }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_build() {
	crate::build("../service_definitions/");
    }
}
