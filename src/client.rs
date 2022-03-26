use std::{
    net::{SocketAddrV4, TcpStream},
    sync::Mutex,
};

use protobuf::CodedInputStream;

use crate::schema::{self, ConnectionResponse};

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
        request.field_type =
            protobuf::EnumOrUnknown::new(schema::connection_request::Type::RPC);
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

fn send<T: protobuf::Message>(rpc: &mut TcpStream, message: T) {
    message
        .write_length_delimited_to_writer(rpc)
        .expect("client::send")
}

fn recv<T: protobuf::Message + Default>(rpc: &mut TcpStream) -> T {
    CodedInputStream::new(rpc)
        .read_message()
        .expect("client::recv")
}
