use std::{io, sync::PoisonError};

use thiserror::Error;

/// The `RpcError` error indicates a failure originating
/// from the server or from the client internally.
#[derive(Error, Debug)]
pub enum RpcError {
    /// `Connection` indicates the client was unable to
    /// connect to the server.
    #[error("Connection failed")]
    Connection(#[from] io::Error),

    /// `Client` errors capture runtime errors from within
    /// the client.
    #[error("Unexpected client error")]
    Client,

    /// `Encoding` errors arise from failures to encode
    /// messages for transmission to the server.
    #[error("Encoding error: {0}")]
    Encoding(String),

    /// `ProtobufError` indicates an error parsing server
    /// messages.
    #[error(transparent)]
    ProtobufError(#[from] protobuf::Error),
}

impl<T> From<PoisonError<T>> for RpcError {
    fn from(_: PoisonError<T>) -> Self {
        RpcError::Client
    }
}
