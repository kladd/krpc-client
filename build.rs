use std::{fs, io::Write};

use convert_case::{Case, Casing};
// use serde::Deserialize;

// #[derive(Deserialize)]
// struct Procedure {
//     id: u32,
//     parameters: Vec<Parameter>,
// }

// #[derive(Deserialize)]
// struct Parameter {
//     name: String,
//     #[serde(rename = "type")]
//     param_type: ParamType,
// }

// #[derive(Deserialize)]
// struct ParamType {
//     code: String,
//     service: String,
//     name: String,
// }

// #[derive(Deserialize)]
// struct ReturnType {
//     code: String,
// }

// #[derive(Deserialize)]
// #[serde(rename_all = "SCREAMING_SNAKE_CASE")]
// enum TypeCode {
//     Class,
//     String,
//     Bool,
//     Sint32,
//     Double,
//     Tuple,
//     Enumeration,
//     Float,
//     Dictionary
// }

fn main() {
    // println!("cargo:rerun-if-changed=proto/krpc.proto");
    prost_build::compile_protos(&["proto/krpc.proto"], &["proto/"]).unwrap();

    // TODO(kladd): scan service_definitions.
    let sc_file =
        fs::File::open("service_definitions/KRPC.SpaceCenter.json").unwrap();

    let mut out_file = fs::File::create("target/test.txt").unwrap();

    let json: serde_json::Value = serde_json::from_reader(sc_file).unwrap();

    // json.as_object().unwrap()._each(|k| {
    //     // write!(out_file, "{}", k.to_case(Case::Snake)).unwrap();
    // });

    json.as_object()
        .unwrap()
        .into_iter()
        .for_each(|(name, def)| {
            write!(out_file, "mod {}\n", name.to_case(Case::Snake)).unwrap();
            def.as_object()
                .unwrap()
                .get("procedures")
                .unwrap()
                .as_object()
                .unwrap()
                .into_iter()
                .for_each(|(proc_name, proc_def)| {
                    write!(out_file, "\tfn {}\n", proc_name).unwrap();
                })
        })
}
