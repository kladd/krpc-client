mod schema {
    include!(concat!(env!("OUT_DIR"), "/krpc.schema.rs"));

    use std::{collections::HashMap, hash::Hash};

    use prost::Message;
    use protobuf::{types::ProtobufType};

    pub trait ToArgument {
        fn to_argument(&self, pos: u32) -> Argument;
    }

    pub trait DecodeUntagged {
        fn decode_untagged(buf: &Vec<u8>) -> Self;
    }

    // TODO: if String is the only type that doesn't need to be dereferenced,
    //       then implement ToArgument for String and get rid of one of these
    //       macros.
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

            impl DecodeUntagged for $name {
                fn decode_untagged(b: &Vec<u8>) -> Self {
                    $name::from_le_bytes(b.to_owned().try_into().unwrap())
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

            impl crate::schema::DecodeUntagged for $name {
                fn decode_untagged(buf: &Vec<u8>) -> Self {
                    $name {
                        id: u64::decode_untagged(buf),
                    }
                }
            }

            impl crate::schema::ToArgument for $name {
                fn to_argument(&self, pos: u32) -> crate::schema::Argument {
                    self.id.to_argument(pos)
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

	    impl crate::schema::DecodeUntagged for $name {
		fn decode_untagged(buf: &Vec<u8>) -> Self {
		    ::num_traits::FromPrimitive::from_u8(u8::decode_untagged(buf)).unwrap()
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

    macro_rules! from_response_message {
	($($m:ty),+$(,)?) => {
	    $(
	    impl From<Response> for $m {
		fn from(response: Response) -> Self {
		    Self::decode(&response.results[0].value[..])
			.expect("unexpected wire type")
		}
	    })+
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
                map.insert(
                    K::decode_untagged(&entry.key),
                    V::decode_untagged(&entry.value),
                );
            });
            map
        }
    }

    impl<T> From<Response> for Vec<T>
    where
        T: DecodeUntagged,
    {
        fn from(response: Response) -> Self {
            List::from(response)
                .items
                .into_iter()
                .map(|item| T::decode_untagged(&item))
                .collect()
        }
    }

    impl From<Response> for () {
        fn from(_: Response) -> Self {
            ()
        }
    }

    from_response_message!(Dictionary, List);
    from_response!(String, ProtobufTypeString);
    from_response!(i32, ProtobufTypeInt32);
    from_response!(i64, ProtobufTypeInt64);
    from_response!(u32, ProtobufTypeUint32);
    from_response!(u64, ProtobufTypeUint64);
    from_response!(f32, ProtobufTypeFloat);
    from_response!(f64, ProtobufTypeDouble);
    from_response!(bool, ProtobufTypeBool);
    from_response_numeric!(u8);

    to_argument!(String, write_string_no_tag);
    to_argument_deref!(bool, write_bool_no_tag);
    to_argument_deref!(i32, write_int32_no_tag);
    to_argument_deref!(f32, write_float_no_tag);
    to_argument_deref!(f64, write_double_no_tag);
    to_argument_deref!(u64, write_uint64_no_tag);

    impl From<Response> for (f64, f64, f64) {
        fn from(response: Response) -> Self {
            let mut is: ::protobuf::CodedInputStream =
                ::protobuf::CodedInputStream::from_bytes(
                    &response.results[0].value[..],
                );

            (
                ::protobuf::types::ProtobufTypeDouble::read(&mut is).unwrap(),
                ::protobuf::types::ProtobufTypeDouble::read(&mut is).unwrap(),
                ::protobuf::types::ProtobufTypeDouble::read(&mut is).unwrap(),
            )
        }
    }

    impl From<Response> for (f64, f64, f64, f64) {
        fn from(response: Response) -> Self {
            let mut is: ::protobuf::CodedInputStream =
                ::protobuf::CodedInputStream::from_bytes(
                    &response.results[0].value[..],
                );

            (
                ::protobuf::types::ProtobufTypeDouble::read(&mut is).unwrap(),
                ::protobuf::types::ProtobufTypeDouble::read(&mut is).unwrap(),
                ::protobuf::types::ProtobufTypeDouble::read(&mut is).unwrap(),
                ::protobuf::types::ProtobufTypeDouble::read(&mut is).unwrap(),
            )
        }
    }

    impl ToArgument for (f64, f64, f64) {
        fn to_argument(&self, pos: u32) -> Argument {
            let mut buf: Vec<u8> = Vec::new();
            {
                let mut outstream = protobuf::CodedOutputStream::vec(&mut buf);

                outstream.write_double_no_tag(self.0).unwrap();
                outstream.write_double_no_tag(self.1).unwrap();
                outstream.write_double_no_tag(self.2).unwrap();
            }

            Argument {
                position: pos,
                value: buf,
            }
        }
    }

    impl ToArgument for (f64, f64, f64, f64) {
        fn to_argument(&self, pos: u32) -> Argument {
            let mut buf: Vec<u8> = Vec::new();
            {
                let mut outstream = protobuf::CodedOutputStream::vec(&mut buf);

                outstream.write_double_no_tag(self.0).unwrap();
                outstream.write_double_no_tag(self.1).unwrap();
                outstream.write_double_no_tag(self.2).unwrap();
                outstream.write_double_no_tag(self.3).unwrap();
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
    include!(concat!(env!("OUT_DIR"), "/services.rs"));
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

        let sc = services::space_center::SpaceCenter::new(Arc::clone(&client));

        dbg!(sc.get_ut());
        dbg!(sc.get_active_vessel());
        dbg!(sc.get_game_mode());
        dbg!(sc.launchable_vessels("VAB".into()));
        dbg!(sc.get_bodies());
        dbg!(sc.get_warp_mode());
    }
}
