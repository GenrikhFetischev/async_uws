use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use uwebsockets_rs::uws_loop::{loop_defer, UwsLoop};
use uwebsockets_rs::websocket::{Opcode, SendStatus, WebSocketStruct};

use crate::ws_message::WsMessage;

pub struct Websocket<const SSL: bool> {
    pub sink: UnboundedSender<WsMessage>,
    pub stream: UnboundedReceiver<WsMessage>,
}

unsafe impl<const SSL: bool> Send for Websocket<SSL> {}
unsafe impl<const SSL: bool> Sync for Websocket<SSL> {}

impl<const SSL: bool> Websocket<SSL> {
    pub fn new(
        native: WebSocketStruct<SSL>,
        uws_loop: UwsLoop,
        from_native_stream: UnboundedReceiver<WsMessage>,
    ) -> Self {
        let (from_server_to_client_sink, mut from_server_to_client_stream) =
            unbounded_channel::<WsMessage>();

        tokio::spawn(async move {
            while let Some(msg) = from_server_to_client_stream.recv().await {
                let websocket = native.clone();
                match msg {
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
                            let send_status =
                                websocket.send(&msg.unwrap_or_default(), Opcode::Ping);
                            if send_status != SendStatus::Success {
                                eprintln!("Error sending websocket ping");
                            }
                        };

                        loop_defer(uws_loop, callback);
                    }
                    WsMessage::Pong(msg) => {
                        let callback = move || {
                            let send_status =
                                websocket.send(&msg.unwrap_or_default(), Opcode::Pong);
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
        });

        Websocket {
            sink: from_server_to_client_sink,
            stream: from_native_stream,
        }
    }
}
