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

    #[error("Connection rejected ({status:?}): {message}")]
    ConnectionRejected {
        status: ConnectionStatus,
        message: String,
    },

    #[error(
        "Server protocol version {0} is not supported; this client \
         requires protocol version 1 (kRPC v0.7.0 or later)"
    )]
    UnsupportedProtocolVersion(u32),

    #[error("{service}.{name}: {description}")]
    Server {
        service: String,
        name: String,
        description: String,
    },

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionStatus {
    MalformedMessage,
    Timeout,
    WrongType,
    WrongProtocolVersion,
    Unknown(i32),
}

impl<T> From<PoisonError<T>> for RpcError {
    fn from(_: PoisonError<T>) -> Self {
        RpcError::Client
    }
}
