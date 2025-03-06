use std::collections::VecDeque;
use uwebsockets_rs::uws_loop::UwsLoop;
use uwebsockets_rs::websocket::Opcode;

#[derive(Clone, Debug)]
pub enum WsMessage {
    Message(Vec<u8>, Opcode),
    Fragmented(Vec<u8>, Opcode, usize),
    Ping(Option<Vec<u8>>),
    Pong(Option<Vec<u8>>),
    Close(i32, Option<String>),
}

impl WsMessage {
    pub fn is_fragmented(&self) -> bool {
        match self {
            WsMessage::Message(_, _) => false,
            WsMessage::Ping(_) => false,
            WsMessage::Pong(_) => false,
            WsMessage::Close(_, _) => false,
            WsMessage::Fragmented(_, _, _) => true,
        }
    }
    pub fn is_msg(&self) -> bool {
        match self {
            WsMessage::Message(_, _) => true,
            WsMessage::Ping(_) => false,
            WsMessage::Pong(_) => false,
            WsMessage::Close(_, _) => false,
            WsMessage::Fragmented(_, _, _) => false,
        }
    }
    pub fn is_ping(&self) -> bool {
        match self {
            WsMessage::Message(_, _) => false,
            WsMessage::Ping(_) => true,
            WsMessage::Pong(_) => false,
            WsMessage::Close(_, _) => false,
            WsMessage::Fragmented(_, _, _) => false,
        }
    }
    pub fn is_pong(&self) -> bool {
        match self {
            WsMessage::Message(_, _) => false,
            WsMessage::Ping(_) => false,
            WsMessage::Pong(_) => true,
            WsMessage::Close(_, _) => false,
            WsMessage::Fragmented(_, _, _) => false,
        }
    }
    pub fn is_close(&self) -> bool {
        match self {
            WsMessage::Message(_, _) => false,
            WsMessage::Ping(_) => false,
            WsMessage::Pong(_) => false,
            WsMessage::Close(_, _) => true,
            WsMessage::Fragmented(_, _, _) => false,
        }
    }

    pub fn is_chunk(&self) -> bool {
        match self {
            WsMessage::Message(_, _) => false,
            WsMessage::Ping(_) => false,
            WsMessage::Pong(_) => false,
            WsMessage::Close(_, _) => false,
            WsMessage::Fragmented(_, _, _) => false,
        }
    }

    pub fn new_fragmented(payload: Vec<u8>, opcode: Opcode, chunk_size: usize) -> Self {
        WsMessage::Fragmented(payload, opcode, chunk_size)
    }
}

impl From<String> for WsMessage {
    fn from(value: String) -> Self {
        WsMessage::Message(Vec::from(value.as_bytes()), Opcode::Text)
    }
}
impl From<&str> for WsMessage {
    fn from(value: &str) -> Self {
        WsMessage::Message(Vec::from(value.as_bytes()), Opcode::Text)
    }
}

impl From<Vec<u8>> for WsMessage {
    fn from(value: Vec<u8>) -> Self {
        WsMessage::Message(value, Opcode::Binary)
    }
}

impl From<&[u8]> for WsMessage {
    fn from(value: &[u8]) -> Self {
        WsMessage::Message(value.into(), Opcode::Binary)
    }
}

#[derive(Debug)]
pub struct PendingChunks {
    pub uws_loop: UwsLoop,
    pub chunks: VecDeque<(Vec<u8>, Opcode, bool, bool)>,
}
unsafe impl Send for PendingChunks {}
