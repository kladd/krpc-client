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

    impl From<Response> for u64 {
        fn from(r: Response) -> Self {
            let v = r.results[0].value.to_owned();
            u64::from_le_bytes(v.try_into().expect("expecting u64"))
        }
    }

    impl From<Response> for u32 {
        fn from(r: Response) -> Self {
            let v = r.results[0].value.to_owned();
            u32::from_le_bytes(v.try_into().expect("expecting u32"))
        }
    }

    impl From<Response> for u8 {
        fn from(r: Response) -> Self {
            let v = r.results[0].value.to_owned();
            u8::from_le_bytes(v.try_into().expect("expecting u8"))
        }
    }

    impl From<Response> for i32 {
        fn from(r: Response) -> Self {
            let v = r.results[0].value.to_owned();
            i32::from_le_bytes(v.try_into().expect("expecting i32"))
        }
    }
}

mod client;

mod services {
    use std::sync::Arc;

    use num_derive::FromPrimitive;
    use num_traits::FromPrimitive;
    use prost::Message;

    use crate::client::Client;
    use crate::schema;

    macro_rules! rpc_object {
        ($name:ident) => {
            #[derive(Debug)]
            pub struct $name {
                pub id: u64,
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

    macro_rules! rpc_enum {
	($name:ident, [$($value:ident),+$(,)?]) => {
	    #[derive(Debug, FromPrimitive)]
	    pub enum $name {$(
		$value,
	    )+}

	    impl From<crate::schema::Response> for $name {
		fn from(response: crate::schema::Response) -> Self {
		    FromPrimitive::from_u8(u8::from(response))
			.expect("invalid enum value")
		}
	    }

	}
    }

    rpc_object!(Vessel);
    rpc_enum!(GameMode, [Sandbox, Career,]);

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

        pub fn get_game_mode(&self) -> GameMode {
            let request = schema::Request::from(Client::proc_call(
                "SpaceCenter",
                "get_GameMode",
                Vec::new(),
            ));

            let response = self.client.call(request);

            GameMode::from(response)
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
        dbg!(sc.get_game_mode());
    }
}
