use std::collections::HashMap;
use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use uwebsockets_rs::http_request::HttpRequest;
use uwebsockets_rs::http_response::HttpResponseStruct;
use uwebsockets_rs::uws_loop::UwsLoop;
use uwebsockets_rs::websocket::{Opcode, WebSocketStruct};
use uwebsockets_rs::websocket_behavior::{
    CompressOptions, UpgradeContext, WebSocketBehavior as NativeWebSocketBehavior,
};

use crate::websocket::{RequestData, Websocket};
use crate::ws_message::WsMessage;

#[derive(Debug)]
pub struct WsPerConnectionUserData {
    id: usize,
    storage: Arc<Mutex<HashMap<usize, WsPerConnectionUserData>>>,
    sink: UnboundedSender<WsMessage>,
    stream: Option<UnboundedReceiver<WsMessage>>,
    is_open: Arc<AtomicBool>,
    req_data: Option<RequestData>,
}

#[derive(Debug, Clone)]
pub struct WsRouteSettings {
    pub compression: Option<u32>,
    pub max_payload_length: Option<u32>,
    pub idle_timeout: Option<u16>,
    pub max_backpressure: Option<u32>,
    pub close_on_backpressure_limit: Option<bool>,
    pub reset_idle_timeout_on_send: Option<bool>,
    pub send_pings_automatically: Option<bool>,
    pub max_lifetime: Option<u16>,
}

impl Default for WsRouteSettings {
    fn default() -> Self {
        let compressor: u32 = CompressOptions::SharedCompressor.into();
        let decompressor: u32 = CompressOptions::SharedDecompressor.into();
        WsRouteSettings {
            compression: Some(compressor | decompressor),
            max_payload_length: Some(1024),
            idle_timeout: Some(800),
            max_backpressure: Some(10),
            close_on_backpressure_limit: Some(false),
            reset_idle_timeout_on_send: Some(true),
            send_pings_automatically: Some(true),
            max_lifetime: Some(111),
        }
    }
}

pub struct WebsocketBehavior<const SSL: bool> {
    pub native_ws_behaviour: NativeWebSocketBehavior<SSL>,
}

impl<const SSL: bool> WebsocketBehavior<SSL> {
    #[allow(clippy::too_many_arguments)]
    pub fn new<T, W>(
        settings: WsRouteSettings,
        uws_loop: UwsLoop,
        ws_per_socket_data_storage: Arc<Mutex<HashMap<usize, WsPerConnectionUserData>>>,
        handler: T,
    ) -> Self
    where
        T: (Fn(Websocket<SSL>) -> W) + 'static + Send + Sync + Clone,
        W: Future<Output = ()> + 'static + Send,
    {
        let native_ws_behaviour = NativeWebSocketBehavior {
            compression: settings.compression.unwrap_or_default(),
            max_payload_length: settings.max_payload_length.unwrap_or_default(),
            idle_timeout: settings.idle_timeout.unwrap_or_default(),
            max_backpressure: settings.max_backpressure.unwrap_or_default(),
            close_on_backpressure_limit: settings.close_on_backpressure_limit.unwrap_or_default(),
            reset_idle_timeout_on_send: settings.reset_idle_timeout_on_send.unwrap_or_default(),
            send_pings_automatically: settings.send_pings_automatically.unwrap_or_default(),
            max_lifetime: settings.max_lifetime.unwrap_or_default(),
            upgrade: Some(Box::new(
                move |res: HttpResponseStruct<SSL>, mut req: HttpRequest, ctx: UpgradeContext| {
                    let headers: Vec<(String, String)> = req
                        .get_headers()
                        .clone()
                        .into_iter()
                        .map(|(k, v)| (k.to_string(), v.to_string()))
                        .collect();
                    let (sink, stream) = unbounded_channel::<WsMessage>();
                    let user_data_id = ctx.get_context_ptr() as usize;

                    let req_data = RequestData {
                        full_url: req.get_full_url().to_string(),
                        headers,
                    };
                    let user_data = WsPerConnectionUserData {
                        sink,
                        id: user_data_id,
                        stream: Some(stream),
                        storage: ws_per_socket_data_storage.clone(),
                        is_open: Arc::new(AtomicBool::new(true)),
                        req_data: Some(req_data),
                    };

                    let mut storage = ws_per_socket_data_storage.lock().unwrap();
                    storage.insert(user_data_id, user_data);

                    let user_data_ref = storage.get_mut(&user_data_id).unwrap();

                    let ws_key_string = req
                        .get_header("sec-websocket-key")
                        .expect("[async_uws]: There is no sec-websocket-key in req headers");
                    let ws_protocol = req.get_header("sec-websocket-protocol");
                    let ws_extensions = req.get_header("sec-websocket-extensions");

                    res.upgrade(
                        ws_key_string,
                        ws_protocol,
                        ws_extensions,
                        ctx,
                        Some(user_data_ref),
                    );
                },
            )),
            open: Some(Box::new(move |ws_connection| {
                let handler = handler.clone();
                let uws_loop = uws_loop;

                let user_data = ws_connection
                    .get_user_data::<WsPerConnectionUserData>()
                    .expect("[async_uws]: There is no receiver / sender pair in ws user data");
                let stream = user_data.stream.take().unwrap();
                let is_open = user_data.is_open.clone();
                let req_data = user_data.req_data.take().unwrap();
                tokio::spawn(async move {
                    let ws = Websocket::new(ws_connection, uws_loop, stream, is_open, req_data);
                    handler(ws).await;
                });
            })),
            message: Some(Box::new(message)),
            ping: Some(Box::new(ping)),
            pong: Some(Box::new(pong)),
            close: Some(Box::new(close)),
            drain: Some(Box::new(drain)),
            subscription: Some(Box::new(subscription)),
        };

        WebsocketBehavior {
            native_ws_behaviour,
        }
    }
}

fn message<const SSL: bool>(native_ws: WebSocketStruct<SSL>, message: &[u8], opcode: Opcode) {
    let user_data = native_ws
        .get_user_data::<WsPerConnectionUserData>()
        .expect("[async_uws]: There is no receiver / sender pair in ws user data");

    user_data
        .sink
        .send(WsMessage::Message(Vec::from(message), opcode))
        .unwrap();
}

fn close<const SSL: bool>(native_ws: WebSocketStruct<SSL>, code: i32, reason: Option<&str>) {
    let user_data = native_ws
        .get_user_data::<WsPerConnectionUserData>()
        .expect("[async_uws]: There is no receiver / sender pair in ws user data");

    user_data
        .sink
        .send(WsMessage::Close(code, reason.map(String::from)))
        .unwrap();

    let mut storage = user_data.storage.lock().unwrap();
    storage.remove(&user_data.id);
    user_data.is_open.store(false, Ordering::SeqCst);
}

fn ping<const SSL: bool>(native_ws: WebSocketStruct<SSL>, message: Option<&[u8]>) {
    let user_data = native_ws
        .get_user_data::<WsPerConnectionUserData>()
        .expect("[async_uws]: There is no receiver / sender pair in ws user data");

    user_data
        .sink
        .send(WsMessage::Ping(message.map(Vec::from)))
        .unwrap();
}

fn pong<const SSL: bool>(native_ws: WebSocketStruct<SSL>, message: Option<&[u8]>) {
    let user_data = native_ws
        .get_user_data::<WsPerConnectionUserData>()
        .expect("[async_uws]: There is no receiver / sender pair in ws user data");

    user_data
        .sink
        .send(WsMessage::Pong(message.map(Vec::from)))
        .unwrap();
}

fn drain<const SSL: bool>(_native_ws: WebSocketStruct<SSL>) {
    todo!("Handle drain event")
}

fn subscription<const SSL: bool>(
    _native_ws: WebSocketStruct<SSL>,
    _topic: &str,
    _param1: i32,
    _param2: i32,
) {
    todo!("handle incoming subscription")
}
