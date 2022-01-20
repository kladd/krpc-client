use bytes::BytesMut;

use std::io::{Read, Write};
use std::net::{SocketAddrV4, TcpStream};
use std::sync::Mutex;

use crate::schema::{self, ConnectionResponse};

const LENGTH_DELIMITER_SIZE: usize = 10;

pub struct Client {
    rpc: Mutex<TcpStream>,
}

impl Client {
    pub fn new(
        name: &str,
        ip_addr: &str,
        rpc_port: u16,
        _stream_port: u16,
    ) -> Self {
        let mut rpc = TcpStream::connect(SocketAddrV4::new(
            ip_addr.parse().unwrap(),
            rpc_port,
        ))
        .expect("binding address");

        // Send connection request.
        let mut request = schema::ConnectionRequest::default();
        request.set_type(schema::connection_request::Type::Rpc);
        request.client_name = String::from(name);

        send(&mut rpc, request);
        let _response = recv::<ConnectionResponse>(&mut rpc);

        Self {
            rpc: Mutex::new(rpc),
        }
    }

    pub fn call(&self, request: schema::Request) -> schema::Response {
        let mut rpc = self.rpc.lock().unwrap();

        send(&mut rpc, request);
        recv(&mut rpc)
    }

    pub fn proc_call(
        service: &str,
        procedure: &str,
        args: Vec<schema::Argument>,
    ) -> schema::ProcedureCall {
        schema::ProcedureCall {
            service: service.into(),
            procedure: procedure.into(),
            arguments: args,
            ..Default::default()
        }
    }
}

fn send<T: prost::Message>(rpc: &mut TcpStream, message: T) {
    let mut buf = {
        let len = message.encoded_len();
        BytesMut::with_capacity(len + prost::length_delimiter_len(len))
    };

    message
        .encode_length_delimited(&mut buf)
        .expect("encoding request");

    rpc.write_all(&buf).expect("sending request");
    rpc.flush().unwrap();
}

fn recv<T: prost::Message + Default>(rpc: &mut TcpStream) -> T {
    let mut buf = BytesMut::new();
    buf.resize(LENGTH_DELIMITER_SIZE, 0);

    let n = rpc.read(&mut buf).expect("reading message length");
    buf.truncate(n);

    let msg_size = prost::decode_length_delimiter(&mut buf)
        .expect("decoding message length");

    buf.resize(msg_size, 0);

    if msg_size > LENGTH_DELIMITER_SIZE {
        let offset =
            LENGTH_DELIMITER_SIZE - prost::length_delimiter_len(msg_size);
        rpc.read_exact(&mut buf[offset..]).expect("reading message");
    }

    T::decode(buf).expect("decoding message")
}
