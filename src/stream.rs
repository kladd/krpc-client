use std::{marker::PhantomData, sync::Arc};

use crate::{
    client::Client,
    error::RpcError,
    schema::{DecodeUntagged, ProcedureCall},
    services::krpc::KRPC,
};

pub struct Stream<T: DecodeUntagged> {
    pub(crate) id: u64,
    krpc: KRPC,
    client: Arc<Client>,
    phantom: PhantomData<T>,
}

impl<T: DecodeUntagged> Stream<T> {
    pub(crate) fn new(
        client: Arc<Client>,
        call: ProcedureCall,
    ) -> Result<Self, RpcError> {
        let krpc = KRPC::new(client.clone());
        let stream = krpc.add_stream(call, true)?;

        Ok(Self {
            id: stream.id,
            krpc,
            client,
            phantom: PhantomData,
        })
    }

    pub fn set_rate(&self, hz: f32) -> Result<(), RpcError> {
        self.krpc.set_stream_rate(self.id, hz)
    }

    pub fn remove(&self) -> Result<(), RpcError> {
        self.krpc.remove_stream(self.id)?;
        self.client.remove_stream(self.id)
    }

    pub fn get(&self) -> Result<T, RpcError> {
        self.client.stream_read(self.id)
    }
}
