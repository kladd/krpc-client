mod schema {
    include!(concat!(env!("OUT_DIR"), "/krpc.rs"));
    use std::{
        collections::{HashMap, HashSet},
        hash::Hash,
    };

    pub use krpc::*;
    use protobuf::{reflect::types::ProtobufType, Message};

    pub trait DecodeUntagged {
        fn decode_untagged(buf: &[u8]) -> Self;
    }

    pub trait FromResponse {
        fn from_response(response: Response) -> Self;
    }

    impl FromResponse for () {
        fn from_response(_: Response) -> Self {}
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
                ..Default::default()
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
                fn decode_untagged(buf: &[u8]) -> Self {
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
                fn decode_untagged(buf: &[u8]) -> Self {
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
                fn decode_untagged(b: &[u8]) -> Self {
                    ::protobuf::reflect::types::$proto::read(
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
                fn decode_untagged(b: &[u8]) -> Self {
                    Self::parse_from_bytes(&b[..]).expect("parse_from_bytes: unexpected wire type")
                }
            }

            impl EncodeUntagged for $m {
                fn encode_untagged(&self) -> Vec<u8> {
                    let mut v = Vec::new();
		            self.write_to_vec(&mut v).expect("encode_untagged");
                    v
                }
            })+
        };
    }

    impl From<ProcedureCall> for Request {
        fn from(proc_call: ProcedureCall) -> Self {
            Request {
                calls: vec![proc_call],
                ..Default::default()
            }
        }
    }

    impl<T0, T1> DecodeUntagged for (T0, T1)
    where
        T0: DecodeUntagged,
        T1: DecodeUntagged,
    {
        fn decode_untagged(buf: &[u8]) -> Self {
            let tuple = Tuple::decode_untagged(buf);
            (
                T0::decode_untagged(tuple.items.get(0).unwrap()),
                T1::decode_untagged(tuple.items.get(1).unwrap()),
            )
        }
    }

    impl<T0, T1> EncodeUntagged for (T0, T1)
    where
        T0: EncodeUntagged,
        T1: EncodeUntagged,
    {
        fn encode_untagged(&self) -> Vec<u8> {
            Tuple {
                items: vec![self.0.encode_untagged(), self.1.encode_untagged()],
                ..Default::default()
            }
            .encode_untagged()
        }
    }

    impl<T0, T1, T2> DecodeUntagged for (T0, T1, T2)
    where
        T0: DecodeUntagged,
        T1: DecodeUntagged,
        T2: DecodeUntagged,
    {
        fn decode_untagged(buf: &[u8]) -> Self {
            let tuple = Tuple::decode_untagged(buf);
            (
                T0::decode_untagged(tuple.items.get(0).unwrap()),
                T1::decode_untagged(tuple.items.get(1).unwrap()),
                T2::decode_untagged(tuple.items.get(2).unwrap()),
            )
        }
    }

    impl<T0, T1, T2> EncodeUntagged for (T0, T1, T2)
    where
        T0: EncodeUntagged,
        T1: EncodeUntagged,
        T2: EncodeUntagged,
    {
        fn encode_untagged(&self) -> Vec<u8> {
            Tuple {
                items: vec![
                    self.0.encode_untagged(),
                    self.1.encode_untagged(),
                    self.2.encode_untagged(),
                ],
                ..Default::default()
            }
            .encode_untagged()
        }
    }

    impl<T0, T1, T2, T3> DecodeUntagged for (T0, T1, T2, T3)
    where
        T0: DecodeUntagged,
        T1: DecodeUntagged,
        T2: DecodeUntagged,
        T3: DecodeUntagged,
    {
        fn decode_untagged(buf: &[u8]) -> Self {
            let tuple = Tuple::decode_untagged(buf);
            (
                T0::decode_untagged(tuple.items.get(0).unwrap()),
                T1::decode_untagged(tuple.items.get(1).unwrap()),
                T2::decode_untagged(tuple.items.get(2).unwrap()),
                T3::decode_untagged(tuple.items.get(3).unwrap()),
            )
        }
    }

    impl<T0, T1, T2, T3> EncodeUntagged for (T0, T1, T2, T3)
    where
        T0: EncodeUntagged,
        T1: EncodeUntagged,
        T2: EncodeUntagged,
        T3: EncodeUntagged,
    {
        fn encode_untagged(&self) -> Vec<u8> {
            Tuple {
                items: vec![
                    self.0.encode_untagged(),
                    self.1.encode_untagged(),
                    self.2.encode_untagged(),
                    self.3.encode_untagged(),
                ],
                ..Default::default()
            }
            .encode_untagged()
        }
    }

    impl<K, V> DecodeUntagged for HashMap<K, V>
    where
        K: DecodeUntagged + Eq + Hash + Default,
        V: DecodeUntagged,
    {
        fn decode_untagged(buf: &[u8]) -> Self {
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
                    ..Default::default()
                })
            }

            Dictionary {
                entries,
                ..Default::default()
            }
            .encode_untagged()
        }
    }

    impl<T> DecodeUntagged for HashSet<T>
    where
        T: DecodeUntagged + Eq + Hash,
    {
        fn decode_untagged(buf: &[u8]) -> Self {
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
            let items: Vec<Vec<u8>> =
                self.iter().map(|item| item.encode_untagged()).collect();
            Set {
                items,
                ..Default::default()
            }
            .encode_untagged()
        }
    }

    impl<T> DecodeUntagged for Vec<T>
    where
        T: DecodeUntagged,
    {
        fn decode_untagged(buf: &[u8]) -> Self {
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
            let items: Vec<Vec<u8>> =
                self.iter().map(|item| item.encode_untagged()).collect();
            List {
                items,
                ..Default::default()
            }
            .encode_untagged()
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
        Event,
        Tuple
    );

    encode_untagged!(String, write_string_no_tag);
    encode_untagged_deref!(bool, write_bool_no_tag);
    encode_untagged_deref!(i32, write_int32_no_tag);
    encode_untagged_deref!(u32, write_uint32_no_tag);
    encode_untagged_deref!(f32, write_float_no_tag);
    encode_untagged_deref!(f64, write_double_no_tag);
    encode_untagged_deref!(u64, write_uint64_no_tag);

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

    use crate::{client::Client, services};

    #[test]
    fn call() {
        eprintln!("connecting");

        let client =
            Arc::new(Client::new("rpc test", "127.0.0.1", 50000, 50001));

        eprintln!("connected");

        let sc = services::space_center::SpaceCenter::new(Arc::clone(&client));

        let ship = sc.get_active_vessel();
        let ap = sc.vessel_get_auto_pilot(&ship);

        let svrf = sc.vessel_get_orbital_reference_frame(&ship);
        let aprf = sc.auto_pilot_get_reference_frame(&ap);

        let x = sc.transform_direction((0.0, 1.0, 0.0), &svrf, &aprf);

        sc.auto_pilot_set_target_direction(&ap, x);
        sc.auto_pilot_engage(&ap);
        sc.auto_pilot_wait(&ap);
        sc.auto_pilot_disengage(&ap);
    }
}
