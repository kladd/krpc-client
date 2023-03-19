use std::{
    env,
    fs::File,
    io::{Read, Write},
    path::Path,
};
mod krpc_build;
// mod krpc_codegen;

use protobuf_codegen::Customize;

fn main() {
    println!("cargo:rerun-if-changed=proto/krpc.proto");

    let out_dir = env::var_os("OUT_DIR").unwrap();

    protobuf_codegen::Codegen::new()
        .pure()
        // LSP handles this fine, but IntelliJ Rust can't figure it out.
        .customize(Customize::default().gen_mod_rs(false))
        .includes(["proto"])
        .input("proto/krpc.proto")
        .out_dir(&out_dir)
        .run_from_script();

    let proto_path = Path::new(&out_dir);

    let mut contents = String::new();
    File::open(proto_path.join("krpc.rs"))
        .unwrap()
        .read_to_string(&mut contents)
        .unwrap();

    let new_contents = format!("pub mod krpc {{\n{}\n}}", contents);
    File::create(proto_path.join("krpc.rs"))
        .unwrap()
        .write_all(new_contents.as_bytes())
        .unwrap();

    let mut f = File::create(proto_path.join("services.rs")).unwrap();
    // krpc_codegen::build("service_definitions/", &mut
    // f).unwrap();

    // let mut x =
    // File::create(Path::new("services.rs")).unwrap();
    krpc_build::build("service_definitions/", &mut f).unwrap();
}
