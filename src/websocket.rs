use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

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
    pub async fn send(&mut self, message: WsMessage) -> SendStatus {
        let websocket = self.native.clone();

        match message {
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
        }
    }

    pub async fn send_with_options(
        &mut self,
        message: WsMessage,
        compress: bool,
        fin: bool,
    ) -> SendStatus {
        let websocket = self.native.clone();
        match message {
            WsMessage::Message(msg, opcode) => {
                let callback = move || websocket.send_with_options(&msg, opcode, compress, fin);
                WebsocketSendFuture::new(Box::new(callback), self.uws_loop).await
            }
            msg => self.send(msg).await,
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
