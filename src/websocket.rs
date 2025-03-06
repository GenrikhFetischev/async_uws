use log::error;
use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use uwebsockets_rs::uws_loop::{loop_defer, UwsLoop};
use uwebsockets_rs::websocket::{Opcode, SendStatus as NativeSendStatus, WebSocketStruct};

use crate::data_storage::SharedDataStorage;
use crate::ws_message::{PendingChunks, WsMessage};

pub struct Websocket<const SSL: bool> {
    pub stream: UnboundedReceiver<WsMessage>,
    native: WebSocketStruct<SSL>,
    uws_loop: UwsLoop,
    is_open: Arc<AtomicBool>,
    global_data_storage: SharedDataStorage,
    per_connection_data_storage: SharedDataStorage,
    pending_chunks: Arc<Mutex<PendingChunks>>,
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
        pending_chunks: Arc<Mutex<PendingChunks>>,
    ) -> Self {
        Websocket {
            stream: from_native_stream,
            native,
            uws_loop,
            is_open,
            global_data_storage,
            per_connection_data_storage,
            pending_chunks,
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

        let pending_chunks = self.pending_chunks;
        tokio::spawn(async move {
            while let Some((message, compress, fin)) = to_client_stream.recv().await {
                let websocket = self.native.clone();

                let status = send_to_socket(
                    message.clone(),
                    compress,
                    fin,
                    websocket,
                    uws_loop,
                    self.is_open.clone(),
                    pending_chunks.clone(),
                )
                .await;

                if let Err(e) = status {
                    error!("[async_uws] Error sending message to client: {e:#?}");
                    break;
                }

                let status = status.unwrap();
                match status {
                    SendStatus::Success => {}
                    SendStatus::Backpressure => {
                        todo!("Must not be here");
                    }
                    SendStatus::Dropped | SendStatus::WsDisconnected => {
                        error!(
                        "[async_uws] Non Success status in attempt to send message to client: {status:#?}"
                    );
                        break;
                    }
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
            self.pending_chunks.clone(),
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
            self.pending_chunks.clone(),
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

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum SendStatus {
    Backpressure,
    Success,
    Dropped,
    WsDisconnected,
}

impl From<NativeSendStatus> for SendStatus {
    fn from(value: NativeSendStatus) -> Self {
        match value {
            NativeSendStatus::Backpressure => SendStatus::Backpressure,
            NativeSendStatus::Success => SendStatus::Success,
            NativeSendStatus::Dropped => SendStatus::Dropped,
        }
    }
}

async fn send_to_socket<const SSL: bool>(
    message: WsMessage,
    compress: bool,
    fin: bool,
    websocket: WebSocketStruct<SSL>,
    uws_loop: UwsLoop,
    is_open: Arc<AtomicBool>,
    pending_chunks: Arc<Mutex<PendingChunks>>,
) -> Result<SendStatus, String> {
    let send_status = match message {
        WsMessage::Message(msg, opcode) => {
            let callback = move || {
                if !is_open.load(Ordering::Relaxed) {
                    return SendStatus::WsDisconnected;
                }
                websocket
                    .send_with_options(&msg, opcode, compress, fin)
                    .into()
            };
            WebsocketSendFuture::new(Box::new(callback), uws_loop).await
        }
        WsMessage::Fragmented(msg, opcode, chunk_size) => {
            let mut chunks = split_into_chunks(msg, chunk_size, opcode.clone(), compress);
            let callback = move || {
                if !is_open.load(Ordering::Relaxed) {
                    return SendStatus::WsDisconnected;
                }

                while let Some((chunk, opcode, compress, is_last)) = chunks.pop_front() {
                    let status =
                        websocket.send_with_options(&chunk, opcode.clone(), compress, is_last);
                    if status == NativeSendStatus::Backpressure {
                        let mut pending_chunks = pending_chunks.lock().unwrap();
                        *pending_chunks = PendingChunks { chunks, uws_loop };
                        break;
                    }
                }

                SendStatus::Success
            };
            WebsocketSendFuture::new(Box::new(callback), uws_loop).await
        }

        WsMessage::Ping(msg) => {
            let callback = move || {
                if !is_open.load(Ordering::Relaxed) {
                    return SendStatus::WsDisconnected;
                }
                websocket
                    .send_with_options(&msg.unwrap_or_default(), Opcode::Ping, false, true)
                    .into()
            };
            WebsocketSendFuture::new(Box::new(callback), uws_loop).await
        }
        WsMessage::Pong(msg) => {
            let callback = move || {
                if !is_open.load(Ordering::Relaxed) {
                    return SendStatus::WsDisconnected;
                }
                websocket
                    .send_with_options(&msg.unwrap_or_default(), Opcode::Pong, false, true)
                    .into()
            };
            WebsocketSendFuture::new(Box::new(callback), uws_loop).await
        }
        WsMessage::Close(code, reason) => {
            let callback = move || {
                if !is_open.load(Ordering::Relaxed) {
                    return SendStatus::WsDisconnected;
                }
                websocket.end(code, reason.as_deref());
                SendStatus::Success
            };
            WebsocketSendFuture::new(Box::new(callback), uws_loop).await
        }
    };
    Ok(send_status)
}

fn split_into_chunks(
    payload: Vec<u8>,
    chunk_size: usize,
    opcode: Opcode,
    compress: bool,
) -> VecDeque<(Vec<u8>, Opcode, bool, bool)> {
    let payload_chunks: Vec<Vec<u8>> = payload
        .chunks(chunk_size)
        .map(|chunk| chunk.to_vec())
        .collect();

    let len = payload_chunks.len();
    payload_chunks
        .into_iter()
        .enumerate()
        .map(|(index, chunk)| {
            let is_last = index == len - 1;
            let is_first = index == 0;
            let opcode = if is_first {
                opcode.clone()
            } else {
                Opcode::Continuation
            };

            (chunk, opcode, compress, is_last)
        })
        .collect()
}
