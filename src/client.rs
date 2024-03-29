use std::{
    net::TcpStream,
    sync::{Arc, Mutex},
    thread,
};

use protobuf::CodedInputStream;

use crate::{
    error::RpcError,
    schema::{
        self, connection_request, connection_response::Status,
        ConnectionRequest, ConnectionResponse, DecodeUntagged, StreamUpdate,
    },
    stream::StreamWrangler,
};

/// The base kRPC client type.
///
/// ## Connecting to the kRPC server
///
/// Call [`new`][new] to establish a connection with the
/// kRPC server.
///
/// ```
/// use krpc_client::Client;
/// let client = Client::new("Test KRPC", "127.0.0.1", 50000, 50001);
/// ```
///
/// ## Using RPC services
///
/// Pass or clone the client instance returned by
/// [`Client::new`][new] to any RPC service in
/// [`krpc_client::services::*`][services].
///
/// ```
/// use krpc_client::{services::space_center::SpaceCenter, Client};
/// let space_center = SpaceCenter::new(client);
/// // Then call procedures with the created service.
/// println!("Hello, {}!", space_center.get_active_vessel()?.get_name()?);
/// ```
///
/// [new]: Client::new
/// [services]: crate::services
pub struct Client {
    rpc: Mutex<TcpStream>,
    stream: Mutex<TcpStream>,
    streams: StreamWrangler,
}

impl Client {
    /// Constructs a new `Client`.
    ///
    /// # Examples
    ///
    /// ```
    /// use krpc_client::Client;
    /// let client = Client::new("Test KRPC", "127.0.0.1", 50000, 50001);
    /// ```
    pub fn new(
        name: &str,
        ip_addr: &str,
        rpc_port: u16,
        stream_port: u16,
    ) -> Result<Arc<Self>, RpcError> {
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

        let client = Arc::new(Self {
            rpc: Mutex::new(rpc_stream),
            stream: Mutex::new(stream_stream),
            streams: StreamWrangler::default(),
        });

        // Spawn a thread to receive stream updates.
        let bg_client = client.clone();
        thread::spawn(move || loop {
            bg_client.update_streams().ok();
        });

        Ok(client)
    }

    pub(crate) fn call(
        &self,
        request: schema::Request,
    ) -> Result<schema::Response, RpcError> {
        let mut rpc = self.rpc.lock().map_err(|_| RpcError::Client)?;

        send(&mut rpc, request)?;
        recv(&mut rpc)
    }

    pub(crate) fn proc_call(
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

    pub(crate) fn update_streams(self: &Arc<Self>) -> Result<(), RpcError> {
        let mut stream = self.stream.lock()?;
        let update = recv::<StreamUpdate>(&mut stream)?;
        for result in update.results {
            self.streams.insert(
                result.id,
                result.result.into_option().ok_or(RpcError::Client)?,
            )?;
        }
        Ok(())
    }

    pub(crate) fn read_stream<T: DecodeUntagged>(
        self: &Arc<Self>,
        id: u64,
    ) -> Result<T, RpcError> {
        self.streams.get(self.clone(), id)
    }

    pub(crate) fn remove_stream(
        self: &Arc<Self>,
        id: u64,
    ) -> Result<(), RpcError> {
        self.streams.remove(id);
        Ok(())
    }

    pub(crate) fn await_stream(&self, id: u64) {
        self.streams.wait(id)
    }
}

fn connect(
    ip_addr: &str,
    port: u16,
    request: ConnectionRequest,
) -> Result<(TcpStream, ConnectionResponse), RpcError> {
    let mut conn = TcpStream::connect(format!("{ip_addr}:{port}"))
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
