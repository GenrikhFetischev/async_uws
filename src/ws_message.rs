use uwebsockets_rs::websocket::Opcode;

#[derive(Clone, Debug)]
pub enum WsMessage {
    Message(Vec<u8>, Opcode),
    Ping(Option<Vec<u8>>),
    Pong(Option<Vec<u8>>),
    Close(i32, Option<String>),
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
