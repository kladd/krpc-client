mod schema {
    include!(concat!(env!("OUT_DIR"), "/krpc.schema.rs"));

    impl From<ProcedureCall> for Request {
        fn from(proc_call: ProcedureCall) -> Self {
            Request {
                calls: vec![proc_call],
            }
        }
    }
}

mod client;
