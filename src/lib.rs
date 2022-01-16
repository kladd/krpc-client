mod schema {
    include!(concat!(env!("OUT_DIR"), "/krpc.schema.rs"));

    use std::collections::HashSet;
    use std::{collections::HashMap, hash::Hash};

    use prost::Message;
    use protobuf::types::ProtobufType;

    pub trait DecodeUntagged {
        fn decode_untagged(buf: &Vec<u8>) -> Self;
    }

    pub trait FromResponse {
        fn from_response(response: Response) -> Self;
    }

    impl FromResponse for () {
        fn from_response(_: Response) -> Self {
            ()
        }
    }

    impl<T: DecodeUntagged> FromResponse for T {
        fn from_response(response: Response) -> Self {
            Self::decode_untagged(&response.results[0].value)
        }
    }

    pub trait ToArgument {
        fn to_argument(&self, pos: u32) -> Argument;
    }

    pub trait EncodeUntagged {
        fn encode_untagged(&self) -> Vec<u8>;
    }

    impl<T: EncodeUntagged> ToArgument for T {
        fn to_argument(&self, pos: u32) -> Argument {
            Argument {
                position: pos,
                value: self.encode_untagged(),
            }
        }
    }

    // TODO: if String is the only type that doesn't need to be dereferenced,
    //       then implement EncodeUntagged for String and get rid of one of
    //       these macros.
    macro_rules! encode_untagged_deref {
        ($t:ty, $fname:ident) => {
            impl EncodeUntagged for $t {
                fn encode_untagged(&self) -> Vec<u8> {
                    let mut buf: Vec<u8> = Vec::new();
                    {
                        let mut outstream =
                            protobuf::CodedOutputStream::new(&mut buf);
                        outstream.$fname(*self).unwrap();
                        outstream.flush().unwrap();
                    }

                    buf
                }
            }
        };
    }

    macro_rules! encode_untagged {
        ($t:ty, $fname:ident) => {
            impl EncodeUntagged for $t {
                fn encode_untagged(&self) -> Vec<u8> {
                    let mut buf: Vec<u8> = Vec::new();
                    {
                        let mut outstream =
                            protobuf::CodedOutputStream::new(&mut buf);
                        outstream.$fname(self).unwrap();
                        outstream.flush().unwrap();
                    }

                    buf
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

            impl crate::schema::DecodeUntagged for $name {
                fn decode_untagged(buf: &Vec<u8>) -> Self {
                    $name {
                        id: u64::decode_untagged(buf),
                    }
                }
            }

            impl crate::schema::EncodeUntagged for $name {
                fn encode_untagged(&self) -> Vec<u8> {
                    self.id.encode_untagged()
                }
            }
        };
    }

    macro_rules! rpc_enum {
        ($name:ident, [$($value:ident),+$(,)?]) => {
            #[derive(Debug, Copy, Clone)]
            pub enum $name {$(
                $value,
            )+}

            impl crate::schema::DecodeUntagged for $name {
                fn decode_untagged(buf: &Vec<u8>) -> Self {
                    Self::from(i32::decode_untagged(buf))
                }
            }

	    impl crate::schema::EncodeUntagged for $name {
		fn encode_untagged(&self) -> Vec<u8> {
		    (*self as i32).encode_untagged()
		}
	    }

            impl From<i32> for $name {
                fn from(val: i32) -> Self {
                    match val {
                        $(i if i == $name::$value as i32 => $name::$value,)+
                        _ => panic!("enum out of range")
                    }
                }
            }
        }
    }

    macro_rules! decode_untagged {
        ($to:ty, $proto:ident) => {
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

    macro_rules! encode_decode_message_untagged {
        ($($m:ty),+$(,)?) => {$(
            impl DecodeUntagged for $m {
                fn decode_untagged(b: &Vec<u8>) -> Self {
                    Self::decode(&b[..]).expect("unexpected wire type")
                }
            }

            impl EncodeUntagged for $m {
                fn encode_untagged(&self) -> Vec<u8> {
                    let mut buf = Vec::new();
                    self.encode_length_delimited(&mut buf).unwrap();
                    buf
                }
            })+
        };
    }

    // There will be tuples of more than one type. Where will your macro be
    // then?
    macro_rules! decode_untagged_tuple {
        (($($m:ty),+$(,)?), $proto:ident) => {
            impl DecodeUntagged for ($( $m, )+) {
                fn decode_untagged(b: &Vec<u8>) -> Self {
                    let mut is: ::protobuf::CodedInputStream =
                        ::protobuf::CodedInputStream::from_bytes(&b);
                    (
                        $(<$m>::from(::protobuf::types::$proto::read(&mut is).unwrap()),)+
                    )
                }
            }
        }
    }

    impl From<ProcedureCall> for Request {
        fn from(proc_call: ProcedureCall) -> Self {
            Request {
                calls: vec![proc_call],
            }
        }
    }

    impl<K, V> DecodeUntagged for HashMap<K, V>
    where
        K: DecodeUntagged + Eq + Hash + Default,
        V: DecodeUntagged,
    {
        fn decode_untagged(buf: &Vec<u8>) -> Self {
            let mut map: HashMap<K, V> = HashMap::new();
            let dictionary = Dictionary::decode_untagged(buf);
            dictionary.entries.into_iter().for_each(|entry| {
                map.insert(
                    K::decode_untagged(&entry.key),
                    V::decode_untagged(&entry.value),
                );
            });
            map
        }
    }

    impl<K, V> EncodeUntagged for HashMap<K, V>
    where
        K: EncodeUntagged,
        V: EncodeUntagged,
    {
        fn encode_untagged(&self) -> Vec<u8> {
            let mut entries = Vec::new();

            for (k, v) in self {
                entries.push(DictionaryEntry {
                    key: k.encode_untagged(),
                    value: v.encode_untagged(),
                })
            }

            Dictionary { entries }.encode_untagged()
        }
    }

    impl<T> DecodeUntagged for HashSet<T>
    where
        T: DecodeUntagged + Eq + Hash,
    {
        fn decode_untagged(buf: &Vec<u8>) -> Self {
            let mut set = HashSet::new();
            let protoset = Set::decode_untagged(buf);
            protoset.items.into_iter().for_each(|item| {
                set.insert(T::decode_untagged(&item));
            });
            set
        }
    }

    impl<T> EncodeUntagged for HashSet<T>
    where
        T: EncodeUntagged,
    {
        fn encode_untagged(&self) -> Vec<u8> {
            let items: Vec<Vec<u8>> = self
                .into_iter()
                .map(|item| item.encode_untagged())
                .collect();
            Set { items }.encode_untagged()
        }
    }

    impl<T> DecodeUntagged for Vec<T>
    where
        T: DecodeUntagged,
    {
        fn decode_untagged(buf: &Vec<u8>) -> Self {
            List::decode_untagged(buf)
                .items
                .into_iter()
                .map(|item| T::decode_untagged(&item))
                .collect()
        }
    }

    impl<T> EncodeUntagged for Vec<T>
    where
        T: EncodeUntagged,
    {
        fn encode_untagged(&self) -> Vec<u8> {
            let items: Vec<Vec<u8>> = self
                .into_iter()
                .map(|item| item.encode_untagged())
                .collect();
            List { items }.encode_untagged()
        }
    }

    decode_untagged!(String, ProtobufTypeString);
    decode_untagged!(bool, ProtobufTypeBool);
    decode_untagged!(f32, ProtobufTypeFloat);
    decode_untagged!(f64, ProtobufTypeDouble);
    decode_untagged!(i32, ProtobufTypeSint32);
    decode_untagged!(i64, ProtobufTypeSint64);
    decode_untagged!(u32, ProtobufTypeUint32);
    decode_untagged!(u64, ProtobufTypeUint64);
    decode_untagged!(Vec<u8>, ProtobufTypeBytes);

    encode_decode_message_untagged!(
        Dictionary,
        List,
        Set,
        Status,
        Stream,
        Services,
        ProcedureCall,
        Event
    );

    decode_untagged_tuple!((f32, f32, f32), ProtobufTypeFloat);
    decode_untagged_tuple!((f64, f64, f64), ProtobufTypeDouble);
    decode_untagged_tuple!((f64, f64, f64, f64), ProtobufTypeDouble);

    encode_untagged!(String, write_string_no_tag);
    encode_untagged_deref!(bool, write_bool_no_tag);
    encode_untagged_deref!(i32, write_int32_no_tag);
    encode_untagged_deref!(u32, write_uint32_no_tag);
    encode_untagged_deref!(f32, write_float_no_tag);
    encode_untagged_deref!(f64, write_double_no_tag);
    encode_untagged_deref!(u64, write_uint64_no_tag);

    impl EncodeUntagged for (f64, f64, f64) {
        fn encode_untagged(&self) -> Vec<u8> {
            let mut buf: Vec<u8> = Vec::new();
            {
                let mut outstream = protobuf::CodedOutputStream::vec(&mut buf);

                outstream.write_double_no_tag(self.0).unwrap();
                outstream.write_double_no_tag(self.1).unwrap();
                outstream.write_double_no_tag(self.2).unwrap();
            }

            buf
        }
    }

    impl EncodeUntagged for (f32, f32, f32) {
        fn encode_untagged(&self) -> Vec<u8> {
            let mut buf: Vec<u8> = Vec::new();
            {
                let mut outstream = protobuf::CodedOutputStream::vec(&mut buf);

                outstream.write_float_no_tag(self.0).unwrap();
                outstream.write_float_no_tag(self.1).unwrap();
                outstream.write_float_no_tag(self.2).unwrap();
            }

            buf
        }
    }

    impl EncodeUntagged for (f64, f64, f64, f64) {
        fn encode_untagged(&self) -> Vec<u8> {
            let mut buf: Vec<u8> = Vec::new();
            {
                let mut outstream = protobuf::CodedOutputStream::vec(&mut buf);

                outstream.write_double_no_tag(self.0).unwrap();
                outstream.write_double_no_tag(self.1).unwrap();
                outstream.write_double_no_tag(self.2).unwrap();
                outstream.write_double_no_tag(self.3).unwrap();
            }

            buf
        }
    }

    impl DecodeUntagged for ((f64, f64, f64), (f64, f64, f64)) {
        fn decode_untagged(buf: &Vec<u8>) -> Self {
            use protobuf::types::ProtobufTypeDouble;

            let mut is: ::protobuf::CodedInputStream =
                ::protobuf::CodedInputStream::from_bytes(&buf);
            (
                (
                    f64::from(ProtobufTypeDouble::read(&mut is).unwrap()),
                    f64::from(ProtobufTypeDouble::read(&mut is).unwrap()),
                    f64::from(ProtobufTypeDouble::read(&mut is).unwrap()),
                ),
                (
                    f64::from(ProtobufTypeDouble::read(&mut is).unwrap()),
                    f64::from(ProtobufTypeDouble::read(&mut is).unwrap()),
                    f64::from(ProtobufTypeDouble::read(&mut is).unwrap()),
                ),
            )
        }
    }

    impl DecodeUntagged for (Vec<u8>, String, String) {
        fn decode_untagged(buf: &Vec<u8>) -> Self {
            let mut is: ::protobuf::CodedInputStream =
                ::protobuf::CodedInputStream::from_bytes(&buf);
            (
                ::protobuf::types::ProtobufTypeBytes::read(&mut is).unwrap(),
                ::protobuf::types::ProtobufTypeString::read(&mut is).unwrap(),
                ::protobuf::types::ProtobufTypeString::read(&mut is).unwrap(),
            )
        }
    }

    pub(crate) use rpc_enum;
    pub(crate) use rpc_object;
}

pub mod client;

pub mod services {
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
        // dbg!(sc.get_active_vessel());
        dbg!(sc.get_game_mode());
    }
}
