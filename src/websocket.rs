use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Context, Poll, Waker};

use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use uwebsockets_rs::uws_loop::{loop_defer, UwsLoop};
use uwebsockets_rs::websocket::{Opcode, SendStatus, WebSocketStruct};

use crate::data_storage::SharedDataStorage;
use crate::ws_message::WsMessage;

pub struct Websocket<const SSL: bool> {
    pub stream: UnboundedReceiver<WsMessage>,
    native: WebSocketStruct<SSL>,
    uws_loop: UwsLoop,
    is_open: Arc<AtomicBool>,
    global_data_storage: SharedDataStorage,
    per_connection_data_storage: SharedDataStorage,
}

unsafe impl<const SSL: bool> Send for Websocket<SSL> {}
unsafe impl<const SSL: bool> Sync for Websocket<SSL> {}

impl<const SSL: bool> Websocket<SSL> {
    pub fn new(
        native: WebSocketStruct<SSL>,
        uws_loop: UwsLoop,
        from_native_stream: UnboundedReceiver<WsMessage>,
        is_open: Arc<AtomicBool>,
        global_data_storage: SharedDataStorage,
        per_connection_data_storage: SharedDataStorage,
    ) -> Self {
        Websocket {
            stream: from_native_stream,
            native,
            uws_loop,
            is_open,
            global_data_storage,
            per_connection_data_storage,
        }
    }

    /***
     * Returns sink & stream. Sink accepts (WsMessage, bool, bool) where fist bool is 'compress' param and second 'fin' (Like in 'send_with_option' method)
     ***/
    pub fn split(
        self,
    ) -> (
        UnboundedSender<(WsMessage, bool, bool)>,
        UnboundedReceiver<WsMessage>,
    ) {
        let (to_client_sink, mut to_client_stream) = unbounded_channel::<(WsMessage, bool, bool)>();

        let uws_loop = self.uws_loop;
        tokio::spawn(async move {
            while let Some((message, compress, fin)) = to_client_stream.recv().await {
                let websocket = self.native.clone();

                let status = send_to_socket(
                    message,
                    compress,
                    fin,
                    websocket,
                    uws_loop,
                    self.is_open.clone(),
                )
                .await;

                if let Err(e) = status {
                    eprintln!("[async_uws] Error sending message to client: {e:#?}");
                    break;
                }

                let status = status.unwrap();
                if status != SendStatus::Success {
                    eprintln!(
                        "[async_uws] Non Success status in attempt to send message to client: {status:#?}"
                    );
                    break;
                }
            }
        });

        (to_client_sink, self.stream)
    }

    pub fn data<T: Send + Sync + Clone + 'static>(&self) -> Option<&T> {
        self.global_data_storage.as_ref().get_data::<T>()
    }

    pub fn connection_data<T: Send + Sync + Clone + 'static>(&self) -> Option<&T> {
        self.per_connection_data_storage.as_ref().get_data::<T>()
    }

    pub fn is_open(&self) -> bool {
        self.is_open.load(Ordering::SeqCst)
    }

    pub async fn send(&mut self, message: WsMessage) -> Result<SendStatus, String> {
        send_to_socket(
            message,
            false,
            true,
            self.native.clone(),
            self.uws_loop,
            self.is_open.clone(),
        )
        .await
    }

    pub async fn send_with_options(
        &mut self,
        message: WsMessage,
        compress: bool,
        fin: bool,
    ) -> Result<SendStatus, String> {
        let is_open = self.is_open.load(Ordering::SeqCst);
        if !is_open {
            return Err("WebSocket is closed!".to_string());
        }
        send_to_socket(
            message,
            compress,
            fin,
            self.native.clone(),
            self.uws_loop,
            self.is_open.clone(),
        )
        .await
    }
}

#[derive(Default)]
struct WebsocketSendFutureState {
    waker: Option<Waker>,
    send_status: Option<SendStatus>,
}
pub struct WebsocketSendFuture {
    state: Arc<Mutex<WebsocketSendFutureState>>,
}

impl WebsocketSendFuture {
    pub fn new(callback: Box<dyn FnOnce() -> SendStatus + Send>, uws_loop: UwsLoop) -> Self {
        let state: Arc<Mutex<WebsocketSendFutureState>> = Default::default();
        let state_to_move = state.clone();
        let closure = move || {
            let send_status = callback();
            let mut state = state_to_move.lock().unwrap();
            state.send_status = Some(send_status);
            if let Some(waker) = state.waker.take() {
                waker.wake()
            }
        };

        tokio::spawn(async move {
            loop_defer(uws_loop, closure);
        });

        WebsocketSendFuture { state }
    }
}

impl Future for WebsocketSendFuture {
    type Output = SendStatus;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut state = self.state.lock().unwrap();
        if let Some(status) = state.send_status.take() {
            return Poll::Ready(status);
        }
        state.waker = Some(cx.waker().clone());
        Poll::Pending
    }
}

async fn send_to_socket<const SSL: bool>(
    message: WsMessage,
    compress: bool,
    fin: bool,
    websocket: WebSocketStruct<SSL>,
    uws_loop: UwsLoop,
    is_open: Arc<AtomicBool>,
) -> Result<SendStatus, String> {
    let is_open = is_open.load(Ordering::SeqCst);
    if !is_open {
        return Err("WebSocket is closed!".to_string());
    }
    let send_status = match message {
        WsMessage::Message(msg, opcode) => {
            let callback = move || websocket.send_with_options(&msg, opcode, compress, fin);
            WebsocketSendFuture::new(Box::new(callback), uws_loop).await
        }
        WsMessage::Ping(msg) => {
            let callback = move || {
                websocket.send_with_options(&msg.unwrap_or_default(), Opcode::Ping, false, true)
            };
            WebsocketSendFuture::new(Box::new(callback), uws_loop).await
        }
        WsMessage::Pong(msg) => {
            let callback = move || {
                websocket.send_with_options(&msg.unwrap_or_default(), Opcode::Pong, false, true)
            };
            WebsocketSendFuture::new(Box::new(callback), uws_loop).await
        }
        WsMessage::Close(code, reason) => {
            let callback = move || {
                websocket.end(code, reason.as_deref());
                SendStatus::Success
            };
            WebsocketSendFuture::new(Box::new(callback), uws_loop).await
        }
    };
    Ok(send_status)
}
