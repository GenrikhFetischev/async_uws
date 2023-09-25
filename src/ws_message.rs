use uwebsockets_rs::websocket::Opcode;

#[derive(Clone, Debug)]
pub enum WsMessage {
    Message(Vec<u8>, Opcode),
    Ping(Option<Vec<u8>>),
    Pong(Option<Vec<u8>>),
    Close(i32, Option<String>),
}
