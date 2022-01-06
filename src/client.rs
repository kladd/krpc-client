use bytes::BytesMut;
use prost::Message;

use std::io::{Read, Write};
use std::net::{SocketAddrV4, TcpStream};
use std::sync::Mutex;

use crate::schema;

pub struct Client {
	rpc: Mutex<TcpStream>,
}

// TODO(kladd):
impl Client {
	pub fn new(
		name: &str,
		ip_addr: &str,
		rpc_port: u16,
		stream_port: u16,
	) -> Self {
		let mut rpc = TcpStream::connect(SocketAddrV4::new(
			ip_addr.parse().unwrap(),
			rpc_port,
		))
		.expect("binding address");

		// TODO(kladd): 512?
		let mut buf = BytesMut::with_capacity(512);

		// Send connection request.
		let mut request = schema::ConnectionRequest::default();
		request.set_type(schema::connection_request::Type::Rpc);
		request.client_name = String::from(name);

		request
			.encode_length_delimited(&mut buf)
			.expect("encoding request");

		rpc.write(&buf.split()).expect("sending request");
		rpc.flush().unwrap();

		// Read response.
		// TODO(kladd): fixed buffer size.
		let mut res_buf = vec![0u8; 28];
		let n = rpc.read(&mut res_buf[..]).expect("reading response");

		let response =
			schema::ConnectionResponse::decode_length_delimited(&res_buf[..n])
				.expect("decode response");

		Self {
			rpc: Mutex::new(rpc),
		}
	}

	pub fn call(&self, request: schema::Request) -> schema::Response {
		let mut rpc = self.rpc.lock().unwrap();

		let mut request_buf = Vec::new();
		request
			.encode_length_delimited(&mut request_buf)
			.expect("encode request");

		rpc.write(&request_buf).expect("rpc send request");
		rpc.flush().expect("rpc send flush");

		// TODO(kladd): fixed buffer size.
		let mut response_buf = vec![0u8; 256];
		let n = rpc
			.read(&mut response_buf[..])
			.expect("rpc recieve response");

		drop(rpc);

		schema::Response::decode_length_delimited(&response_buf[..n])
			.expect("decode response")
	}

	pub fn proc_call(
		service: &str,
		procedure: &str,
		args: Vec<schema::Argument>,
	) -> schema::ProcedureCall {
		let mut proc = schema::ProcedureCall::default();
		proc.service = String::from(service);
		proc.procedure = String::from(procedure);
		proc.arguments = args;

		proc
	}
}
