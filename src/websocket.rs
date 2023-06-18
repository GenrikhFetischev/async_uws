use std::sync::mpsc::Sender;

use tokio::sync::mpsc::Receiver;
use uwebsockets_rs::websocket::{Opcode, WebSocketStruct};

pub struct Websocket<const SSL: bool> {
    pub stream: Receiver<(&'static [u8], Opcode)>,
    pub sink: Sender<(Vec<u8>, Opcode)>,
    native: WebSocketStruct<SSL>,
}
