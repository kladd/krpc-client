mod schema {
    include!(concat!(env!("OUT_DIR"), "/krpc.schema.rs"));

    impl From<ProcedureCall> for Request {
        fn from(proc_call: ProcedureCall) -> Self {
            Request {
                calls: vec![proc_call],
            }
        }
    }

    impl From<Response> for f64 {
        fn from(r: Response) -> Self {
            let v = r.results[0].value.to_owned();
            f64::from_le_bytes(v.try_into().expect("expecting f64"))
        }
    }
}

mod client;

mod services {
    use std::sync::Arc;

    use prost::Message;

    use crate::client::Client;
    use crate::schema;

    macro_rules! rpc_object {
        ($name:ident) => {
            #[derive(Debug)]
            pub struct $name {
                id: u64,
            }
            impl From<crate::schema::Response> for $name {
                fn from(response: crate::schema::Response) -> Self {
                    $name {
                        id: u64::from(response),
                    }
                }
            }
        };
    }

    pub struct KRPC {
        client: Arc<Client>,
    }

    impl KRPC {
        pub fn new(client: Arc<Client>) -> Self {
            KRPC { client }
        }

        pub fn get_status(&self) -> schema::Status {
            let request = schema::Request::from(Client::proc_call(
                "KRPC",
                "GetStatus",
                Vec::new(),
            ));

            let response = self.client.call(request);

            schema::Status::decode(&response.results[0].value[..])
                .expect("decode status")
        }
    }

    pub struct SpaceCenter {
        client: Arc<Client>,
    }

    impl SpaceCenter {
        pub fn new(client: Arc<Client>) -> Self {
            SpaceCenter { client }
        }

        pub fn get_ut(&self) -> f64 {
            let request = schema::Request::from(Client::proc_call(
                "SpaceCenter",
                "get_UT",
                Vec::new(),
            ));

            let response = self.client.call(request);

            f64::from(response)
        }
    }
}

#[cfg(test)]
mod test {
    use std::sync::Arc;

    use crate::client::Client;
    use crate::services;

    #[test]
    fn call() {
        let client =
            Arc::new(Client::new("rpc test", "127.0.0.1", 50000, 50001));

        let krpc = services::KRPC::new(Arc::clone(&client));
        let sc = services::SpaceCenter::new(Arc::clone(&client));
        dbg!(sc.get_ut());
        dbg!(krpc.get_status());
    }
}
