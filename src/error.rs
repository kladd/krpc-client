use std::{io, sync::PoisonError};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum RpcError {
    #[error("Connection failed")]
    Connection(#[from] io::Error),

    #[error("Unexpected client error")]
    Client,

    #[error("Encoding error: {0}")]
    Encoding(String),

    #[error(transparent)]
    ProtobufError(#[from] protobuf::Error),
}

impl<T> From<PoisonError<T>> for RpcError {
    fn from(_: PoisonError<T>) -> Self {
        RpcError::Client
    }
}
