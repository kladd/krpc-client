mod schema {
    include!(concat!(env!("OUT_DIR"), "/krpc.schema.rs"));

    use std::{collections::HashMap, hash::Hash};

    use prost::Message;
    use protobuf::types::ProtobufType;

    pub trait ToArgument {
        fn to_argument(&self, pos: u32) -> Argument;
    }

    pub trait DecodeUntagged {
        fn decode_untagged(buf: &Vec<u8>) -> Self;
    }

    // TODO: if String is the only type that doesn't need to be dereferenced, then implement ToArgument for String and get rid of one of these macros.
    macro_rules! to_argument_deref {
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

    macro_rules! to_argument {
        ($t:ty, $fname:ident) => {
            impl ToArgument for $t {
                fn to_argument(&self, pos: u32) -> Argument {
                    let mut buf: Vec<u8> = Vec::new();
                    {
                        let mut outstream =
                            protobuf::CodedOutputStream::new(&mut buf);
                        outstream.$fname(self).unwrap();
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
            #[derive(Debug, Default)]
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

            impl DecodeUntagged for $name {
                fn decode_untagged(buf: &Vec<u8>) -> Self {
                    $name {
                        id: u64::decode_untagged(buf),
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

    macro_rules! from_response {
        ($to:ty, $proto:ident) => {
            impl From<Response> for $to {
                fn from(response: Response) -> Self {
                    Self::decode_untagged(&response.results[0].value)
                }
            }

            impl DecodeUntagged for $to {
                fn decode_untagged(b: &Vec<u8>) -> Self {
                    ::protobuf::types::$proto::read(
                        &mut ::protobuf::CodedInputStream::from_bytes(b),
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

    impl<K, V> From<Response> for HashMap<K, V>
    where
        K: DecodeUntagged + Eq + Hash + Default,
        V: DecodeUntagged,
    {
        fn from(response: Response) -> Self {
            let mut map: HashMap<K, V> = HashMap::new();
            let dictionary = Dictionary::from(response);
            dictionary.entries.into_iter().for_each(|entry| {
                dbg!(&entry);
                map.insert(
                    K::decode_untagged(&entry.key),
                    V::decode_untagged(&entry.value),
                );
            });
            map
        }
    }

    impl From<Response> for Dictionary {
        fn from(response: Response) -> Self {
            Self::decode(&response.results[0].value[..])
                .expect("unexpected wire type")
        }
    }

    from_response!(String, ProtobufTypeString);
    from_response!(i32, ProtobufTypeInt32);
    from_response!(i64, ProtobufTypeInt64);
    from_response!(u32, ProtobufTypeUint32);
    from_response!(u64, ProtobufTypeUint64);
    from_response!(f32, ProtobufTypeFloat);
    from_response!(f64, ProtobufTypeDouble);
    from_response_numeric!(u8);

    to_argument!(String, write_string_no_tag);

    pub(crate) use rpc_enum;
    pub(crate) use rpc_object;
}

mod client;

mod services {
    use std::collections::HashMap;
    use std::sync::Arc;

    use prost::Message;

    use crate::client::Client;
    use crate::schema::{self, DecodeUntagged, ToArgument};

    schema::rpc_object!(Vessel);
    schema::rpc_object!(CelestialBody);
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

        pub fn get_bodies(&self) -> HashMap<String, CelestialBody> {
            let request = schema::Request::from(Client::proc_call(
                "SpaceCenter",
                "get_Bodies",
                vec![],
            ));

            let response = self.client.call(request);

            HashMap::from(response)
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

        dbg!(sc.save("test_save_two".into()));
        dbg!(sc.get_bodies());
    }
}
