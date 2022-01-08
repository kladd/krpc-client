use std::{env, fs, path::Path};

fn main() {
    println!("cargo:rerun-if-changed=proto/krpc.proto");
    prost_build::compile_protos(&["proto/krpc.proto"], &["proto/"]).unwrap();

    let mut f = fs::File::create(
        Path::new(&env::var_os("OUT_DIR").unwrap()).join("services.rs"),
    )
    .unwrap();
    krpc_build::build("service_definitions/", &mut f).unwrap();
}
