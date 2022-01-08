fn main() {
    println!("cargo:rerun-if-changed=proto/krpc.proto");
    prost_build::compile_protos(&["proto/krpc.proto"], &["proto/"]).unwrap();
}
