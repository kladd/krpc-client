use std::io;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum RpcError {
    #[error("connection failed")]
    Connection(#[from] io::Error),

    #[error("unexpected client error")]
    Client,

    #[error("encoding error: {0}")]
    Encoding(String),

    #[error(transparent)]
    ProtobufError(#[from] protobuf::Error),
}
