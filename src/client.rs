use std::{
    collections::HashMap,
    net::TcpStream,
    sync::{Arc, Mutex, RwLock},
};

use protobuf::CodedInputStream;

use crate::{
    error::RpcError,
    schema::{
        self, connection_request, connection_response::Status,
        ConnectionRequest, ConnectionResponse, DecodeUntagged, ProcedureResult,
        StreamUpdate,
    },
};

pub struct Client {
    rpc: Mutex<TcpStream>,
    stream: Mutex<TcpStream>,
    streams: RwLock<HashMap<u64, ProcedureResult>>,
}

impl Client {
    pub fn new(
        name: &str,
        ip_addr: &str,
        rpc_port: u16,
        stream_port: u16,
    ) -> Result<Self, RpcError> {
        let rpc_request = schema::ConnectionRequest {
            type_: protobuf::EnumOrUnknown::new(connection_request::Type::RPC),
            client_name: String::from(name),
            ..Default::default()
        };
        let (rpc_stream, rpc_result) = connect(ip_addr, rpc_port, rpc_request)?;

        let stream_request = schema::ConnectionRequest {
            type_: protobuf::EnumOrUnknown::new(
                connection_request::Type::STREAM,
            ),
            client_name: String::from(name),
            client_identifier: rpc_result.client_identifier,
            ..Default::default()
        };
        let (stream_stream, _) = connect(ip_addr, stream_port, stream_request)?;

        Ok(Self {
            rpc: Mutex::new(rpc_stream),
            stream: Mutex::new(stream_stream),
            streams: RwLock::new(HashMap::new()),
        })
    }

    pub fn call(
        &self,
        request: schema::Request,
    ) -> Result<schema::Response, RpcError> {
        let mut rpc = self.rpc.lock().map_err(|_| RpcError::Client)?;

        send(&mut rpc, request)?;
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

    pub fn stream_update(self: &Arc<Self>) -> Result<(), RpcError> {
        let mut stream = self.stream.lock()?;
        let update = recv::<StreamUpdate>(&mut stream)?;
        for result in update.results {
            let mut streams = self.streams.write()?;
            streams.insert(
                result.id,
                result.result.into_option().ok_or(RpcError::Client)?,
            );
        }
        Ok(())
    }

    pub fn stream_read<T: DecodeUntagged>(
        self: &Arc<Self>,
        id: u64,
    ) -> Result<T, RpcError> {
        let streams = self.streams.read()?;
        T::decode_untagged(
            self.clone(),
            &streams.get(&id).ok_or(RpcError::Client)?.value,
        )
    }

    pub fn remove_stream(self: &Arc<Self>, id: u64) -> Result<(), RpcError> {
        let mut streams = self.streams.write()?;
        streams.remove(&id);
        Ok(())
    }
}

fn connect(
    ip_addr: &str,
    port: u16,
    request: ConnectionRequest,
) -> Result<(TcpStream, ConnectionResponse), RpcError> {
    let mut conn = TcpStream::connect(format!("{}:{}", ip_addr, port))
        .map_err(RpcError::Connection)?;

    send(&mut conn, request)?;
    let response = recv::<ConnectionResponse>(&mut conn)?;
    if response.status.value() != Status::OK as i32 {
        return Err(RpcError::Client);
    }

    Ok((conn, response))
}

fn send<T: protobuf::Message>(
    rpc: &mut TcpStream,
    message: T,
) -> Result<(), RpcError> {
    message
        .write_length_delimited_to_writer(rpc)
        .map_err(Into::into)
}

fn recv<T: protobuf::Message + Default>(
    rpc: &mut TcpStream,
) -> Result<T, RpcError> {
    CodedInputStream::new(rpc)
        .read_message()
        .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::services::space_center::SpaceCenter;

    #[test]
    fn streams() {
        let client = Arc::new(
            Client::new("RPC TEST", "127.0.0.1", 50000, 50001).unwrap(),
        );

        let space_center = SpaceCenter::new(client.clone());
        let ut_stream = space_center.get_ut_stream().unwrap();
        ut_stream.set_rate(1f32).unwrap();

        for _ in 0..10 {
            client.stream_update();
            dbg!(ut_stream.get());
        }
    }
}
