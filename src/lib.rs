pub mod client;
pub mod error;

pub mod services {
    include!(concat!(env!("OUT_DIR"), "/services.rs"));
}

mod schema {
    use std::{
        collections::{HashMap, HashSet},
        hash::Hash,
        sync::Arc,
    };

    pub use krpc::*;
    use protobuf::Message;

    use crate::{client::Client, error::RpcError};

    include!(concat!(env!("OUT_DIR"), "/krpc.rs"));
    pub trait DecodeUntagged: Sized {
        fn decode_untagged(
            client: Arc<Client>,
            buf: &[u8],
        ) -> Result<Self, RpcError>;
    }

    pub trait FromResponse: Sized {
        fn from_response(
            response: Response,
            client: Arc<Client>,
        ) -> Result<Self, RpcError>;
    }

    impl FromResponse for () {
        fn from_response(
            _: Response,
            _: Arc<Client>,
        ) -> Result<Self, RpcError> {
            Ok(())
        }
    }

    impl<T: DecodeUntagged> FromResponse for T {
        fn from_response(
            response: Response,
            client: Arc<Client>,
        ) -> Result<T, RpcError> {
            Self::decode_untagged(client, &response.results[0].value)
        }
    }

    pub trait ToArgument {
        fn to_argument(&self, pos: u32) -> Result<Argument, RpcError>;
    }

    pub trait EncodeUntagged {
        fn encode_untagged(&self) -> Result<Vec<u8>, RpcError>;
    }

    impl<T: EncodeUntagged> ToArgument for T {
        fn to_argument(&self, pos: u32) -> Result<Argument, RpcError> {
            Ok(Argument {
                position: pos,
                value: self.encode_untagged()?,
                ..Default::default()
            })
        }
    }

    macro_rules! encode_untagged {
        ($t:ty, $fname:ident) => {
            impl EncodeUntagged for $t {
                fn encode_untagged(&self) -> Result<Vec<u8>, RpcError> {
                    let mut buf: Vec<u8> = Vec::new();
                    {
                        let mut outstream =
                            protobuf::CodedOutputStream::new(&mut buf);
                        outstream
                            .$fname(*self)
                            .map_err(|e| RpcError::from(e))?;
                        outstream.flush().map_err(|e| RpcError::from(e))?;
                    }

                    Ok(buf)
                }
            }
        };
    }

    macro_rules! rpc_object {
        ($name:ident) => {
            pub struct $name {
                pub id: u64,
                pub client: ::std::sync::Arc<crate::client::Client>,
            }

            impl crate::schema::DecodeUntagged for $name {
                fn decode_untagged(
                    client: ::std::sync::Arc<crate::client::Client>,
                    buf: &[u8],
                ) -> Result<Self, RpcError> {
                    Ok($name {
                        id: u64::decode_untagged(client.clone(), buf)?,
                        client,
                    })
                }
            }

            impl crate::schema::EncodeUntagged for $name {
                fn encode_untagged(&self) -> Result<Vec<u8>, RpcError> {
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
                fn decode_untagged(
                    client: ::std::sync::Arc<crate::client::Client>,
                    buf: &[u8]
                ) -> Result<Self, RpcError> {
                    match i32::decode_untagged(client, buf)? {
                        $(i if i == $name::$value as i32 => Ok($name::$value),)+
                        _ => Err(RpcError::Encoding("invalid enum variant".into()))
                    }
                }
            }

            impl crate::schema::EncodeUntagged for $name {
                fn encode_untagged(&self) -> Result<Vec<u8>, RpcError> {
                    (*self as i32).encode_untagged()
                }
            }
        }
    }

    macro_rules! decode_untagged {
        ($to:ty, $proto:ident) => {
            impl DecodeUntagged for $to {
                fn decode_untagged(
                    _: ::std::sync::Arc<crate::client::Client>,
                    b: &[u8],
                ) -> Result<Self, RpcError> {
                    ::protobuf::CodedInputStream::from_bytes(b)
                        .$proto()
                        .map_err(|e| e.into())
                }
            }
        };
    }

    macro_rules! encode_decode_message_untagged {
        ($($m:ty),+$(,)?) => {$(
            impl DecodeUntagged for $m {
                fn decode_untagged(
                    _: ::std::sync::Arc<crate::client::Client>,
                    b: &[u8]
                ) -> Result<Self, RpcError> {
                    Self::parse_from_bytes(&b[..]).map_err(|e| RpcError::from(e))
                }
            }

            impl EncodeUntagged for $m {
                fn encode_untagged(&self) -> Result<Vec<u8>, RpcError> {
                    let mut v = Vec::new();
		            self.write_to_vec(&mut v)?;
                    Ok(v)
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
        fn decode_untagged(
            client: Arc<Client>,
            buf: &[u8],
        ) -> Result<Self, RpcError> {
            let tuple = Tuple::decode_untagged(client.clone(), buf)?;
            Ok((
                T0::decode_untagged(
                    client.clone(),
                    tuple.items.get(0).ok_or(RpcError::Encoding(
                        "tuple element out of range".into(),
                    ))?,
                )?,
                T1::decode_untagged(
                    client,
                    tuple.items.get(1).ok_or(RpcError::Encoding(
                        "tuple element out of range".into(),
                    ))?,
                )?,
            ))
        }
    }

    impl<T0, T1> EncodeUntagged for (T0, T1)
    where
        T0: EncodeUntagged,
        T1: EncodeUntagged,
    {
        fn encode_untagged(&self) -> Result<Vec<u8>, RpcError> {
            Tuple {
                items: vec![
                    self.0.encode_untagged()?,
                    self.1.encode_untagged()?,
                ],
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
        fn decode_untagged(
            client: Arc<Client>,
            buf: &[u8],
        ) -> Result<Self, RpcError> {
            let tuple = Tuple::decode_untagged(client.clone(), buf)?;
            Ok((
                T0::decode_untagged(
                    client.clone(),
                    tuple.items.get(0).ok_or(RpcError::Encoding(
                        "tuple element out of range".into(),
                    ))?,
                )?,
                T1::decode_untagged(
                    client.clone(),
                    tuple.items.get(1).ok_or(RpcError::Encoding(
                        "tuple element out of range".into(),
                    ))?,
                )?,
                T2::decode_untagged(
                    client,
                    tuple.items.get(2).ok_or(RpcError::Encoding(
                        "tuple element out of range".into(),
                    ))?,
                )?,
            ))
        }
    }

    impl<T0, T1, T2> EncodeUntagged for (T0, T1, T2)
    where
        T0: EncodeUntagged,
        T1: EncodeUntagged,
        T2: EncodeUntagged,
    {
        fn encode_untagged(&self) -> Result<Vec<u8>, RpcError> {
            Tuple {
                items: vec![
                    self.0.encode_untagged()?,
                    self.1.encode_untagged()?,
                    self.2.encode_untagged()?,
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
        fn decode_untagged(
            client: Arc<Client>,
            buf: &[u8],
        ) -> Result<Self, RpcError> {
            let tuple = Tuple::decode_untagged(client.clone(), buf)?;
            Ok((
                T0::decode_untagged(
                    client.clone(),
                    tuple.items.get(0).ok_or(RpcError::Encoding(
                        "tuple element out of range".into(),
                    ))?,
                )?,
                T1::decode_untagged(
                    client.clone(),
                    tuple.items.get(1).ok_or(RpcError::Encoding(
                        "tuple element out of range".into(),
                    ))?,
                )?,
                T2::decode_untagged(
                    client.clone(),
                    tuple.items.get(2).ok_or(RpcError::Encoding(
                        "tuple element out of range".into(),
                    ))?,
                )?,
                T3::decode_untagged(
                    client,
                    tuple.items.get(3).ok_or(RpcError::Encoding(
                        "tuple element out of range".into(),
                    ))?,
                )?,
            ))
        }
    }

    impl<T0, T1, T2, T3> EncodeUntagged for (T0, T1, T2, T3)
    where
        T0: EncodeUntagged,
        T1: EncodeUntagged,
        T2: EncodeUntagged,
        T3: EncodeUntagged,
    {
        fn encode_untagged(&self) -> Result<Vec<u8>, RpcError> {
            Tuple {
                items: vec![
                    self.0.encode_untagged()?,
                    self.1.encode_untagged()?,
                    self.2.encode_untagged()?,
                    self.3.encode_untagged()?,
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
        fn decode_untagged(
            client: Arc<Client>,
            buf: &[u8],
        ) -> Result<Self, RpcError> {
            let mut map: HashMap<K, V> = HashMap::new();
            let dictionary = Dictionary::decode_untagged(client.clone(), buf)?;
            for entry in dictionary.entries.into_iter() {
                map.insert(
                    K::decode_untagged(client.clone(), &entry.key)?,
                    V::decode_untagged(client.clone(), &entry.value)?,
                );
            }
            Ok(map)
        }
    }

    impl<K, V> EncodeUntagged for HashMap<K, V>
    where
        K: EncodeUntagged,
        V: EncodeUntagged,
    {
        fn encode_untagged(&self) -> Result<Vec<u8>, RpcError> {
            let mut entries = Vec::new();

            for (k, v) in self {
                entries.push(DictionaryEntry {
                    key: k.encode_untagged()?,
                    value: v.encode_untagged()?,
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
        fn decode_untagged(
            client: Arc<Client>,
            buf: &[u8],
        ) -> Result<Self, RpcError> {
            let protoset = Set::decode_untagged(client.clone(), buf)?;
            let mut set = HashSet::new();

            for item in protoset.items.into_iter() {
                set.insert(T::decode_untagged(client.clone(), &item)?);
            }

            Ok(set)
        }
    }

    impl<T> EncodeUntagged for HashSet<T>
    where
        T: EncodeUntagged,
    {
        fn encode_untagged(&self) -> Result<Vec<u8>, RpcError> {
            let mut items = Vec::new();
            for item in self.iter() {
                items.push(item.encode_untagged()?);
            }

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
        fn decode_untagged(
            client: Arc<Client>,
            buf: &[u8],
        ) -> Result<Self, RpcError> {
            let mut v = Vec::new();
            for item in List::decode_untagged(client.clone(), buf)?
                .items
                .into_iter()
            {
                v.push(T::decode_untagged(client.clone(), &item)?);
            }

            Ok(v)
        }
    }

    impl<T> EncodeUntagged for Vec<T>
    where
        T: EncodeUntagged,
    {
        fn encode_untagged(&self) -> Result<Vec<u8>, RpcError> {
            let mut items = Vec::new();
            for item in self.iter() {
                items.push(item.encode_untagged()?);
            }

            List {
                items,
                ..Default::default()
            }
            .encode_untagged()
        }
    }

    impl EncodeUntagged for String {
        fn encode_untagged(&self) -> Result<Vec<u8>, RpcError> {
            let mut buf: Vec<u8> = Vec::new();
            {
                let mut os = protobuf::CodedOutputStream::new(&mut buf);
                os.write_string_no_tag(self).map_err(RpcError::from)?;
                os.flush().map_err(RpcError::from)?;
            }

            Ok(buf)
        }
    }

    decode_untagged!(String, read_string);
    decode_untagged!(bool, read_bool);
    decode_untagged!(f32, read_float);
    decode_untagged!(f64, read_double);
    decode_untagged!(i32, read_sint32);
    decode_untagged!(u32, read_uint32);
    decode_untagged!(u64, read_uint64);
    decode_untagged!(Vec<u8>, read_bytes);

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

    encode_untagged!(bool, write_bool_no_tag);
    encode_untagged!(i32, write_sint32_no_tag);
    encode_untagged!(u32, write_uint32_no_tag);
    encode_untagged!(f32, write_float_no_tag);
    encode_untagged!(f64, write_double_no_tag);
    encode_untagged!(u64, write_uint64_no_tag);

    pub(crate) use rpc_enum;
    pub(crate) use rpc_object;
}

#[cfg(test)]
mod test {
    use std::sync::Arc;

    use services::space_center::SpaceCenter;

    use crate::{client::Client, error::RpcError, services};

    #[test]
    fn call() -> Result<(), RpcError> {
        eprintln!("connecting");

        let client = Arc::new(
            Client::new("rpc test", "127.0.0.1", 50000, 50001).unwrap(),
        );

        eprintln!("connected");

        let sc = SpaceCenter::new(client);
        let ship = sc.get_active_vessel()?;

        eprintln!("{}", ship.get_name()?);

        Ok(())
    }
}
