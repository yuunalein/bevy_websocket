use std::{
    collections::HashMap,
    net::{SocketAddr, TcpStream},
    sync::Arc,
};

use bevy::prelude::*;
use parking_lot::Mutex;
use websocket::{sync::Writer, CloseData, OwnedMessage, WebSocketError};

pub(crate) type SenderMap = Arc<Mutex<HashMap<SocketAddr, Arc<Mutex<Writer<TcpStream>>>>>>;

#[derive(Debug)]
pub enum SendError {
    NotFound,
    WriteError(WebSocketError),
    FailedToCloseStream,
}

#[derive(Resource, Clone)]
pub struct WebSocketWriter {
    map: SenderMap,
}
impl WebSocketWriter {
    pub(crate) fn new(map: SenderMap) -> Self {
        Self { map }
    }

    fn write<R, F: FnOnce(&mut Arc<Mutex<Writer<TcpStream>>>) -> R>(
        &mut self,
        addr: &SocketAddr,
        f: F,
    ) -> Option<R> {
        self.map.lock_arc().get_mut(addr).map(f)
    }

    pub fn send_message<M: ToString>(
        &mut self,
        message: M,
        to: &SocketAddr,
    ) -> Result<(), SendError> {
        match self.write(to, |w| {
            w.lock_arc()
                .send_message(&OwnedMessage::Text(message.to_string()))
                .map_err(SendError::WriteError)
        }) {
            Some(res) => res,
            None => Err(SendError::NotFound),
        }
    }

    pub fn send_binary(&mut self, data: Vec<u8>, to: &SocketAddr) -> Result<(), SendError> {
        match self.write(to, |w| {
            w.lock_arc()
                .send_message(&OwnedMessage::Binary(data))
                .map_err(SendError::WriteError)
        }) {
            Some(res) => res,
            None => Err(SendError::NotFound),
        }
    }

    pub fn close(&mut self, data: Option<CloseData>, to: &SocketAddr) -> Result<(), SendError> {
        match self.write(to, |w| {
            let mut w = w.lock_arc();
            match w.send_message(&OwnedMessage::Close(data)) {
                Ok(_) => Ok(()),
                Err(e) => Err(SendError::WriteError(e)),
            }?;
            w.shutdown_all().map_err(|_| SendError::FailedToCloseStream)
        }) {
            Some(res) => res,
            None => Err(SendError::NotFound),
        }
    }
}
