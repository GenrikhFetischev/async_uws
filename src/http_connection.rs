use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tokio::sync::mpsc::{unbounded_channel, Receiver};
use uwebsockets_rs::http_response::HttpResponseStruct;
use uwebsockets_rs::uws_loop::{loop_defer, UwsLoop};
use uwebsockets_rs::websocket_behavior::UpgradeContext;

use crate::body_reader::{BodyChunk, BodyReader};
use crate::data_storage::SharedDataStorage;
use crate::http_request::HttpRequest;
use crate::loop_defer_future::LoopDeferFuture;
use crate::ws_behavior::{WsPerSocketUserData, WsPerSocketUserDataStorage};
use crate::ws_message::WsMessage;

pub struct HttpConnection<const SSL: bool> {
    pub(crate) native: Option<HttpResponseStruct<SSL>>,
    pub(crate) uws_loop: UwsLoop,
    pub(crate) body_reader: Option<BodyReader<SSL>>,
    pub is_aborted: Arc<AtomicBool>,
    data_storage: SharedDataStorage,
    per_socket_data_storage: Option<WsPerSocketUserDataStorage>,
    upgrade_context: Option<UpgradeContext>,
    headers: Option<Vec<(String, String)>>,
    response_status: Option<String>,
}

unsafe impl<const SSL: bool> Sync for HttpConnection<SSL> {}

unsafe impl<const SSL: bool> Send for HttpConnection<SSL> {}

impl<const SSL: bool> HttpConnection<SSL> {
    pub fn new(
        native_response: HttpResponseStruct<SSL>,
        uws_loop: UwsLoop,
        is_aborted: Arc<AtomicBool>,
        data_storage: SharedDataStorage,
        body_reader: Option<BodyReader<SSL>>,
        // Will be not None only for upgrade requests
        per_socket_data_storage: Option<WsPerSocketUserDataStorage>,
        // Will be not None only for upgrade requests
        upgrade_context: Option<UpgradeContext>,
    ) -> Self {
        HttpConnection {
            native: Some(native_response),
            is_aborted,
            uws_loop,
            data_storage,
            per_socket_data_storage,
            upgrade_context,
            body_reader,
            headers: None,
            response_status: None,
        }
    }

    // Will be none if there is no "content-length" header presented in request
    pub async fn get_body(&mut self) -> Option<Vec<u8>> {
        if let Some(body) = self.body_reader.take() {
            body.collect().await
        } else {
            None
        }
    }

    // Will be none if there is no "content-length" header presented in request
    pub fn get_body_stream(&mut self) -> Result<Receiver<BodyChunk>, String> {
        match self.body_reader.take() {
            None => Err("Body could be read only once".to_string()),
            Some(body) => Ok(body.take_stream()),
        }
    }

    pub fn data<T: Send + Sync + Clone + 'static>(&self) -> Option<&T> {
        self.data_storage.as_ref().get_data::<T>()
    }

    pub async fn end(self, data: Option<Vec<u8>>, close_connection: bool) {
        let callback = move || {
            let connection = self.native.unwrap();
            if let Some(status) = self.response_status.as_ref() {
                connection.write_status(status);
            }

            if let Some(headers) = self.headers {
                for (key, value) in headers.iter() {
                    connection.write_header(key, value);
                }
            }

            if data.is_some() {
                let response = data.as_deref();
                connection.end(response, close_connection);
            } else {
                connection.end_without_body(close_connection);
            }
        };
        LoopDeferFuture::new(callback, self.uws_loop).await;
    }

    pub fn write_status(&mut self, status: String) {
        self.response_status = Some(status);
    }

    pub fn write_header(&mut self, key: String, value: String) {
        if let Some(headers) = self.headers.as_mut() {
            headers.push((key, value))
        } else {
            self.headers = Some(vec![(key, value)])
        }
    }

    pub fn has_responded(&self) -> bool {
        if let Some(response) = self.native.as_ref() {
            response.has_responded()
        } else {
            true
        }
    }

    pub fn upgrade(
        self,
        ws_key_string: String,
        ws_protocol: Option<String>,
        ws_extensions: Option<String>,
        user_data: Option<SharedDataStorage>,
    ) {
        let (sink, stream) = unbounded_channel::<WsMessage>();

        let ws_per_socket_data_storage = self.per_socket_data_storage.clone().unwrap();
        let user_data = WsPerSocketUserData {
            sink,
            id: None,
            stream: Some(stream),
            storage: ws_per_socket_data_storage.clone(),
            is_open: Arc::new(AtomicBool::new(true)),
            shared_data_storage: self.data_storage.clone(),
            custom_user_data: user_data.unwrap_or_default(),
        };

        let mut user_data = Box::new(user_data);
        let user_data_id = user_data.as_mut() as *mut WsPerSocketUserData as usize;
        user_data.id = Some(user_data_id);

        {
            let mut storage = ws_per_socket_data_storage.lock().unwrap();
            storage.insert(user_data_id, user_data);
        }

        let is_aborted = self.is_aborted.clone();
        let callback = move || {
            let user_data_ptr = user_data_id as *mut WsPerSocketUserData;
            let mut non_null =
                NonNull::new(user_data_ptr).expect("[async_uws] WsPerSocketUserData is null :(");
            let user_data_ref: &mut WsPerSocketUserData = unsafe { non_null.as_mut() };

            let ws_protocol: Option<&str> = ws_protocol.as_deref();
            let ws_extensions: Option<&str> = ws_extensions.as_deref();

            if is_aborted.load(Ordering::SeqCst) {
                println!("[async_uws] Upgrade request is aborted");
                let mut storage = ws_per_socket_data_storage.lock().unwrap();
                storage.remove(&user_data_id);
                return;
            }
            self.native.unwrap().upgrade(
                &ws_key_string,
                ws_protocol,
                ws_extensions,
                self.upgrade_context.unwrap(),
                Some(user_data_ref),
            );
        };

        loop_defer(self.uws_loop, callback)
    }

    pub fn default_upgrade(req: HttpRequest, res: HttpConnection<SSL>) {
        let ws_key = req
            .get_header("sec-websocket-key")
            .map(String::from)
            .expect("[async_uws]: There is no sec-websocket-key in req headers");
        let ws_protocol = req.get_header("sec-websocket-protocol").map(String::from);
        let ws_extensions = req.get_header("sec-websocket-extensions").map(String::from);

        res.upgrade(ws_key, ws_protocol, ws_extensions, None);
    }
}
