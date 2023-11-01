use std::collections::HashMap;
use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use uwebsockets_rs::http_request::HttpRequest;
use uwebsockets_rs::http_response::HttpResponseStruct;
use uwebsockets_rs::uws_loop::UwsLoop;
use uwebsockets_rs::websocket::{Opcode, WebSocketStruct};
use uwebsockets_rs::websocket_behavior::{
    CompressOptions, UpgradeContext, WebSocketBehavior as NativeWebSocketBehavior,
};

use crate::data_storage::SharedDataStorage;
use crate::http_response::HttpResponse;
use crate::websocket::Websocket;
use crate::ws_message::WsMessage;

pub type SharedWsPerSocketUserData = Box<WsPerSocketUserData>;
pub type WsPerSocketUserDataStorage = Arc<Mutex<HashMap<usize, SharedWsPerSocketUserData>>>;

#[derive(Debug)]
pub struct WsPerSocketUserData {
    pub(crate) id: Option<usize>,
    pub(crate) storage: WsPerSocketUserDataStorage,
    pub(crate) sink: UnboundedSender<WsMessage>,
    pub(crate) stream: Option<UnboundedReceiver<WsMessage>>,
    pub(crate) is_open: Arc<AtomicBool>,
    pub(crate) shared_data_storage: SharedDataStorage,
    pub(crate) custom_user_data: SharedDataStorage,
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
    pub fn new<H, R, U>(
        settings: WsRouteSettings,
        uws_loop: UwsLoop,
        ws_per_socket_data_storage: WsPerSocketUserDataStorage,
        handler: H,
        upgrade_hook: U,
        global_data_storage: SharedDataStorage,
    ) -> Self
    where
        H: (Fn(Websocket<SSL>) -> R) + 'static + Send + Sync + Clone,
        U: Fn(HttpRequest, HttpResponse<SSL>) + 'static + Send + Sync + Clone,
        R: Future<Output = ()> + 'static + Send,
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
                move |mut res: HttpResponseStruct<SSL>, req: HttpRequest, ctx: UpgradeContext| {
                    let is_aborted = Arc::new(AtomicBool::new(false));
                    let is_aborted_to_move = is_aborted.clone();
                    res.on_aborted(move || {
                        is_aborted_to_move.store(true, Ordering::Relaxed);
                    });

                    let res = HttpResponse::<SSL>::new(
                        res,
                        uws_loop,
                        is_aborted.clone(),
                        global_data_storage.clone(),
                        Some(ws_per_socket_data_storage.clone()),
                        Some(ctx),
                    );
                    upgrade_hook(req, res);
                },
            )),
            open: Some(Box::new(move |ws_connection| {
                let handler = handler.clone();
                let user_data = ws_connection
                    .get_user_data::<WsPerSocketUserData>()
                    .expect("[async_uws]: There is no receiver / sender pair in ws user data");

                let stream = user_data.stream.take().unwrap();
                let is_open = user_data.is_open.clone();
                let data_storage = user_data.shared_data_storage.clone();
                let per_connection_data_storage = user_data.custom_user_data.clone();
                tokio::spawn(async move {
                    let ws = Websocket::new(
                        ws_connection,
                        uws_loop,
                        stream,
                        is_open,
                        data_storage,
                        per_connection_data_storage,
                    );
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
        .get_user_data::<WsPerSocketUserData>()
        .expect("[async_uws]: There is no receiver / sender pair in ws user data");

    user_data
        .sink
        .send(WsMessage::Message(Vec::from(message), opcode))
        .unwrap_or_default();
}

fn close<const SSL: bool>(native_ws: WebSocketStruct<SSL>, code: i32, reason: Option<&str>) {
    let user_data = native_ws
        .get_user_data::<WsPerSocketUserData>()
        .expect("[async_uws]: There is no receiver / sender pair in ws user data");

    user_data
        .sink
        .send(WsMessage::Close(code, reason.map(String::from)))
        .unwrap_or_default();
    user_data.is_open.store(false, Ordering::Relaxed);

    let mut storage = user_data.storage.lock().unwrap();
    storage.remove(&user_data.id.unwrap());
}

fn ping<const SSL: bool>(native_ws: WebSocketStruct<SSL>, message: Option<&[u8]>) {
    let user_data = native_ws
        .get_user_data::<WsPerSocketUserData>()
        .expect("[async_uws]: There is no receiver / sender pair in ws user data");

    user_data
        .sink
        .send(WsMessage::Ping(message.map(Vec::from)))
        .unwrap_or_default();
}

fn pong<const SSL: bool>(native_ws: WebSocketStruct<SSL>, message: Option<&[u8]>) {
    let user_data = native_ws
        .get_user_data::<WsPerSocketUserData>()
        .expect("[async_uws]: There is no receiver / sender pair in ws user data");

    user_data
        .sink
        .send(WsMessage::Pong(message.map(Vec::from)))
        .unwrap_or_default();
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
