fn main() {
    prost_build::compile_protos(&["proto/krpc.proto"], &["proto/"]).unwrap();
}
