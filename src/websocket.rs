use tokio::sync::mpsc::UnboundedReceiver;
use uwebsockets_rs::uws_loop::{loop_defer, UwsLoop};
use uwebsockets_rs::websocket::{Opcode, SendStatus, WebSocketStruct};

use crate::ws_message::WsMessage;

pub struct Websocket<const SSL: bool> {
    pub stream: UnboundedReceiver<WsMessage>,
    native: WebSocketStruct<SSL>,
    uws_loop: UwsLoop,
}

unsafe impl<const SSL: bool> Send for Websocket<SSL> {}
unsafe impl<const SSL: bool> Sync for Websocket<SSL> {}

impl<const SSL: bool> Websocket<SSL> {
    pub fn send(&mut self, message: WsMessage) {
        let websocket = self.native.clone();
        let uws_loop = self.uws_loop;
        match message {
            WsMessage::Message(msg, opcode) => {
                let callback = move || {
                    let send_status = websocket.send(&msg, opcode);
                    if send_status != SendStatus::Success {
                        eprintln!("Error sending websocket message");
                    }
                };

                loop_defer(uws_loop, callback);
            }
            WsMessage::Ping(msg) => {
                let callback = move || {
                    let send_status = websocket.send(&msg.unwrap_or_default(), Opcode::Ping);
                    if send_status != SendStatus::Success {
                        eprintln!("Error sending websocket ping");
                    }
                };

                loop_defer(uws_loop, callback);
            }
            WsMessage::Pong(msg) => {
                let callback = move || {
                    let send_status = websocket.send(&msg.unwrap_or_default(), Opcode::Pong);
                    if send_status != SendStatus::Success {
                        eprintln!("Error sending websocket ping");
                    }
                };

                loop_defer(uws_loop, callback);
            }
            WsMessage::Close(_code, reason) => {
                let callback = move || {
                    let reason_bytes = reason
                        .map(|reason| {
                            let x = reason.as_bytes();
                            Vec::from(x)
                        })
                        .unwrap_or_default();
                    let send_status = websocket.send(&reason_bytes, Opcode::Close);
                    if send_status != SendStatus::Success {
                        eprintln!("Error sending websocket ping");
                    }
                    websocket.close();
                };
                loop_defer(uws_loop, callback);
            }
        }
    }

    pub fn send_with_options(&mut self, message: WsMessage, compress: bool, fin: bool) {
        let websocket = self.native.clone();
        let uws_loop = self.uws_loop;
        match message {
            WsMessage::Message(msg, opcode) => {
                let callback = move || {
                    let send_status = websocket.send_with_options(&msg, opcode, compress, fin);
                    if send_status != SendStatus::Success {
                        eprintln!("Error sending websocket message");
                    }
                };

                loop_defer(uws_loop, callback);
            }
            msg => self.send(msg),
        }
    }

    pub fn new(
        native: WebSocketStruct<SSL>,
        uws_loop: UwsLoop,
        from_native_stream: UnboundedReceiver<WsMessage>,
    ) -> Self {
        Websocket {
            stream: from_native_stream,
            native,
            uws_loop,
        }
    }
}
