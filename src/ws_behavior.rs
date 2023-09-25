use std::future::Future;

use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use uwebsockets_rs::http_request::HttpRequest;
use uwebsockets_rs::http_response::HttpResponseStruct;
use uwebsockets_rs::uws_loop::UwsLoop;
use uwebsockets_rs::websocket::{Opcode, WebSocketStruct};
use uwebsockets_rs::websocket_behavior::{
    UpgradeContext, WebSocketBehavior as NativeWebSocketBehavior,
};

use crate::websocket::Websocket;
use crate::ws_message::WsMessage;

#[derive(Debug)]
struct WsUserData {
    sink: UnboundedSender<WsMessage>,
    stream: Option<UnboundedReceiver<WsMessage>>,
}

impl Drop for WsUserData {
    fn drop(&mut self) {
        println!("DROP!!!!!!!");
    }
}

pub struct WebsocketBehavior<const SSL: bool> {
    pub native_ws_behaviour: NativeWebSocketBehavior<SSL>,
}

impl<const SSL: bool> WebsocketBehavior<SSL> {
    #[allow(clippy::too_many_arguments)]
    pub fn new<T, W>(
        compression: Option<u32>,
        max_payload_length: Option<u32>,
        idle_timeout: Option<u16>,
        max_backpressure: Option<u32>,
        close_on_backpressure_limit: Option<bool>,
        reset_idle_timeout_on_send: Option<bool>,
        send_pings_automatically: Option<bool>,
        max_lifetime: Option<u16>,
        uws_loop: UwsLoop,
        handler: T,
    ) -> Self
    where
        T: (Fn(Websocket<SSL>) -> W) + 'static + Send + Sync + Clone,
        W: Future<Output = ()> + 'static + Send,
    {
        let native_ws_behaviour = NativeWebSocketBehavior {
            compression: compression.unwrap_or_default(),
            max_payload_length: max_payload_length.unwrap_or_default(),
            idle_timeout: idle_timeout.unwrap_or_default(),
            max_backpressure: max_backpressure.unwrap_or_default(),
            close_on_backpressure_limit: close_on_backpressure_limit.unwrap_or_default(),
            reset_idle_timeout_on_send: reset_idle_timeout_on_send.unwrap_or_default(),
            send_pings_automatically: send_pings_automatically.unwrap_or_default(),
            max_lifetime: max_lifetime.unwrap_or_default(),
            upgrade: Some(Box::new(
                move |res: HttpResponseStruct<SSL>, req: HttpRequest, ctx: UpgradeContext| {
                    let ws_key_string = req
                        .get_header("sec-websocket-key")
                        .expect("[async_uws]: There is no sec-websocket-key in req headers");
                    let ws_protocol = req.get_header("sec-websocket-protocol");
                    let ws_extensions = req.get_header("sec-websocket-extensions");

                    let (sink, stream) = unbounded_channel::<WsMessage>();
                    let user_data = WsUserData {
                        sink,
                        stream: Some(stream),
                    };

                    res.upgrade(
                        ws_key_string,
                        ws_protocol,
                        ws_extensions,
                        ctx,
                        Some(Box::new(user_data)),
                    );
                },
            )),
            open: Some(Box::new(move |ws_connection| {
                let handler = handler.clone();
                let uws_loop = uws_loop;

                let user_data = ws_connection
                    .get_user_data::<WsUserData>()
                    .expect("[async_uws]: There is no receiver / sender pair in ws user data");
                let stream = user_data.stream.take().unwrap();
                tokio::spawn(async move {
                    let ws = Websocket::new(ws_connection, uws_loop, stream);
                    handler(ws).await;
                });
            })),
            message: Some(Box::new(message)),
            ping: None,
            pong: None,
            close: Some(Box::new(close)),
            drain: None,
            subscription: None,
        };

        WebsocketBehavior {
            native_ws_behaviour,
        }
    }
}

fn message<const SSL: bool>(native_ws: WebSocketStruct<SSL>, message: &[u8], opcode: Opcode) {
    let user_data = native_ws
        .get_user_data::<WsUserData>()
        .expect("[async_uws]: There is no receiver / sender pair in ws user data");

    user_data
        .sink
        .send(WsMessage::Message(Vec::from(message), opcode))
        .unwrap();
}

fn close<const SSL: bool>(native_ws: WebSocketStruct<SSL>, code: i32, reason: Option<&str>) {
    let user_data = native_ws
        .get_user_data::<WsUserData>()
        .expect("[async_uws]: There is no receiver / sender pair in ws user data");

    user_data
        .sink
        .send(WsMessage::Close(code, reason.map(String::from)))
        .unwrap();
}
