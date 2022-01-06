mod schema {
    include!(concat!(env!("OUT_DIR"), "/krpc.schema.rs"));

    use protobuf::types::ProtobufType;

    pub trait ToArgument {
        fn to_argument(&self, pos: u32) -> Argument;
    }

    macro_rules! from_response_numeric {
        ($name:ident) => {
            impl From<Response> for $name {
                fn from(r: Response) -> Self {
                    $name::from_le_bytes(
                        r.results[0]
                            .value
                            .to_owned()
                            .try_into()
                            .expect(concat!("expecting ", stringify!($name))),
                    )
                }
            }
        };
    }

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
	    #[derive(Debug, ::num_derive::FromPrimitive)]
	    pub enum $name {$(
		$value,
	    )+}

	    impl From<crate::schema::Response> for $name {
		fn from(response: crate::schema::Response) -> Self {
		    ::num_traits::FromPrimitive::from_u8(u8::from(response))
			.expect("invalid enum value")
		}
	    }
	}
    }

    macro_rules! to_argument {
        ($t:ty, $fname:ident) => {
            impl ToArgument for $t {
                fn to_argument(&self, pos: u32) -> Argument {
                    let mut buf: Vec<u8> = Vec::new();
                    {
                        let mut outstream =
                            protobuf::CodedOutputStream::new(&mut buf);
                        outstream.$fname(*self).unwrap();
                        outstream.flush().unwrap();
                    }

                    Argument {
                        position: pos,
                        value: buf,
                    }
                }
            }
        };
    }

    macro_rules! from_response {
        ($to:ty, $proto:ident) => {
            impl From<Response> for $to {
                fn from(response: Response) -> Self {
                    protobuf::types::$proto::read(
                        &mut protobuf::CodedInputStream::from_bytes(
                            &response.results[0].value,
                        ),
                    )
                    .unwrap()
                }
            }
        };
    }

    impl From<ProcedureCall> for Request {
        fn from(proc_call: ProcedureCall) -> Self {
            Request {
                calls: vec![proc_call],
            }
        }
    }

    from_response_numeric!(u8);
    from_response_numeric!(u32);
    from_response_numeric!(u64);
    from_response_numeric!(i32);
    from_response_numeric!(i64);
    from_response_numeric!(f32);
    from_response_numeric!(f64);
    from_response!(String, ProtobufTypeString);

    impl ToArgument for String {
        fn to_argument(&self, pos: u32) -> Argument {
            let mut buf: Vec<u8> = Vec::new();
            {
                let mut outstream = protobuf::CodedOutputStream::new(&mut buf);
                outstream.write_string_no_tag(&self).unwrap();
                outstream.flush().unwrap();
            }

            Argument {
                position: pos,
                value: buf,
            }
        }
    }

    pub(crate) use rpc_enum;
    pub(crate) use rpc_object;
}

mod client;

mod services {
    use std::sync::Arc;

    use prost::Message;

    use crate::client::Client;
    use crate::schema::{self, ToArgument};

    schema::rpc_object!(Vessel);
    schema::rpc_enum!(
        GameMode,
        [
            Sandbox,
            Career,
            Science,
            ScienceSandbox,
            Mission,
            MissionBuilder,
            Scenario,
            ScenarioNonResumable
        ]
    );

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

        pub fn save(&self, name: String) {
            let request = schema::Request::from(Client::proc_call(
                "SpaceCenter",
                "Save",
                vec![name.to_argument(0)],
            ));

            let response = self.client.call(request);

            dbg!(response);
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
        dbg!(sc.save("test_save".into()));
    }
}
