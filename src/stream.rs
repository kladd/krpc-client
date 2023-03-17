use std::{
    collections::HashMap,
    marker::PhantomData,
    sync::{Arc, Condvar, Mutex},
};

use crate::{
    client::Client,
    error::RpcError,
    schema::{DecodeUntagged, ProcedureCall, ProcedureResult},
    services::krpc::KRPC,
};

/// A streaming procedure call.
///
/// `Stream<T>` is created by calling any procedure with the
/// `_stream()` suffix. This will start the stream
/// automatically.
///
/// This type provides access to the procedure's
/// results of type `T` via [`get`][get]. Results are pushed
/// by the server at the rate selected by
/// [`set_rate`][set_rate]. And consumers may block until a
/// stream's value has changed with [`wait`][wait].
///
/// The stream will attempt to remove itself when dropped.
/// Otherwise the server will remove remaining streams when
/// the client disconnects.
///
/// [wait]: Stream::wait
/// [set_rate]: Stream::set_rate
/// [get]: Stream::get
pub struct Stream<T: DecodeUntagged> {
    pub(crate) id: u64,
    krpc: KRPC,
    client: Arc<Client>,
    phantom: PhantomData<T>,
}

type StreamEntry = Arc<(Mutex<ProcedureResult>, Condvar)>;
#[derive(Default)]
pub(crate) struct StreamWrangler {
    streams: Mutex<HashMap<u64, StreamEntry>>,
}

impl StreamWrangler {
    pub fn insert(
        &self,
        id: u64,
        procedure_result: ProcedureResult,
    ) -> Result<(), RpcError> {
        let mut map = self.streams.lock().unwrap();
        let (lock, cvar) =
            { &*map.entry(id).or_insert_with(Default::default).clone() };

        *lock.lock().unwrap() = procedure_result;
        cvar.notify_one();

        Ok(())
    }

    pub fn wait(&self, id: u64) {
        let (lock, cvar) = {
            let mut map = self.streams.lock().unwrap();
            &*map.entry(id).or_insert_with(Default::default).clone()
        };
        let result = lock.lock().unwrap();
        let _result = cvar.wait(result).unwrap();
    }

    pub fn remove(&self, id: u64) {
        let mut map = self.streams.lock().unwrap();
        map.remove(&id);
    }

    pub fn get<T: DecodeUntagged>(
        &self,
        client: Arc<Client>,
        id: u64,
    ) -> Result<T, RpcError> {
        let mut map = self.streams.lock().unwrap();
        let (lock, _) =
            { &*map.entry(id).or_insert_with(Default::default).clone() };
        let result = lock.lock().unwrap();
        T::decode_untagged(client, &result.value)
    }
}

impl<T: DecodeUntagged> Stream<T> {
    pub(crate) fn new(
        client: Arc<Client>,
        call: ProcedureCall,
    ) -> Result<Self, RpcError> {
        let krpc = KRPC::new(client.clone());
        let stream = krpc.add_stream(call, true)?;
        client.await_stream(stream.id);

        Ok(Self {
            id: stream.id,
            krpc,
            client,
            phantom: PhantomData,
        })
    }

    /// Set the update rate for this streaming procedure.
    pub fn set_rate(&self, hz: f32) -> Result<(), RpcError> {
        self.krpc.set_stream_rate(self.id, hz)
    }

    /// Retrieve the current result received for this
    /// procedure. This value is not guaranteed to have
    /// changed since the last call to [`get`][get]. Use
    /// [`wait`][wait] to block until the value has changed.
    ///
    /// [wait]: Stream::wait
    /// [get]: Stream::get
    pub fn get(&self) -> Result<T, RpcError> {
        self.client.read_stream(self.id)
    }

    /// Block the current thread of execution until this
    /// stream receives an update from the server.
    pub fn wait(&self) {
        self.client.await_stream(self.id);
    }
}

impl<T: DecodeUntagged> Drop for Stream<T> {
    // Try to remove the stream if it's dropped, but don't panic
    // if unable.
    fn drop(&mut self) {
        self.krpc.remove_stream(self.id).ok();
        self.client.remove_stream(self.id).ok();
    }
}
