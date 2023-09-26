use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

use tokio::sync::mpsc::UnboundedReceiver;
use uwebsockets_rs::uws_loop::{loop_defer, UwsLoop};
use uwebsockets_rs::websocket::{Opcode, SendStatus, WebSocketStruct};

use crate::ws_message::WsMessage;

pub struct Websocket<const SSL: bool> {
    pub stream: UnboundedReceiver<WsMessage>,
    pub req_data: RequestData,
    native: WebSocketStruct<SSL>,
    uws_loop: UwsLoop,
    is_open: Arc<AtomicBool>,
}

#[derive(Debug)]
pub struct RequestData {
    pub full_url: String,
    pub headers: Vec<(String, String)>,
}

unsafe impl<const SSL: bool> Send for Websocket<SSL> {}
unsafe impl<const SSL: bool> Sync for Websocket<SSL> {}

impl<const SSL: bool> Websocket<SSL> {
    pub fn new(
        native: WebSocketStruct<SSL>,
        uws_loop: UwsLoop,
        from_native_stream: UnboundedReceiver<WsMessage>,
        is_open: Arc<AtomicBool>,
        req_data: RequestData,
    ) -> Self {
        Websocket {
            stream: from_native_stream,
            native,
            uws_loop,
            is_open,
            req_data,
        }
    }

    pub async fn send(&mut self, message: WsMessage) -> Result<SendStatus, String> {
        let is_open = self.is_open.load(Ordering::SeqCst);
        if !is_open {
            return Err("WebSocket is closed!".to_string());
        }

        let websocket = self.native.clone();

        let status = match message {
            WsMessage::Message(msg, opcode) => {
                let callback = move || websocket.send(&msg, opcode);
                WebsocketSendFuture::new(Box::new(callback), self.uws_loop).await
            }
            WsMessage::Ping(msg) => {
                let callback = move || websocket.send(&msg.unwrap_or_default(), Opcode::Ping);
                WebsocketSendFuture::new(Box::new(callback), self.uws_loop).await
            }
            WsMessage::Pong(msg) => {
                let callback = move || websocket.send(&msg.unwrap_or_default(), Opcode::Pong);
                WebsocketSendFuture::new(Box::new(callback), self.uws_loop).await
            }
            WsMessage::Close(_code, reason) => {
                let callback = move || {
                    let reason_bytes = reason
                        .map(|reason| Vec::from(reason.as_bytes()))
                        .unwrap_or_default();
                    let send_status = websocket.send(&reason_bytes, Opcode::Close);
                    websocket.close();
                    send_status
                };
                WebsocketSendFuture::new(Box::new(callback), self.uws_loop).await
            }
        };

        Ok(status)
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
        let websocket = self.native.clone();
        match message {
            WsMessage::Message(msg, opcode) => {
                let callback = move || websocket.send_with_options(&msg, opcode, compress, fin);
                Ok(WebsocketSendFuture::new(Box::new(callback), self.uws_loop).await)
            }
            msg => self.send(msg).await,
        }
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
