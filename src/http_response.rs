use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use uwebsockets_rs::http_response::HttpResponseStruct;
use uwebsockets_rs::uws_loop::{loop_defer, UwsLoop};

use crate::data_storage::SharedDataStorage;

pub struct HttpResponse<const SSL: bool> {
    pub(crate) native: Option<HttpResponseStruct<SSL>>,
    pub(crate) uws_loop: UwsLoop,
    pub is_aborted: Arc<AtomicBool>,
    data_storage: SharedDataStorage,
}

unsafe impl<const SSL: bool> Sync for HttpResponse<SSL> {}
unsafe impl<const SSL: bool> Send for HttpResponse<SSL> {}

impl<const SSL: bool> HttpResponse<SSL> {
    pub fn new(
        native_response: HttpResponseStruct<SSL>,
        uws_loop: UwsLoop,
        is_aborted: Arc<AtomicBool>,
        data_storage: SharedDataStorage,
    ) -> Self {
        HttpResponse {
            native: Some(native_response),
            is_aborted,
            uws_loop,
            data_storage,
        }
    }

    pub fn data<T: Send + Sync + Clone + 'static>(&self) -> Option<&T> {
        self.data_storage.as_ref().get_data::<T>()
    }

    pub fn end(mut self, data: Option<&'static str>, close_connection: bool) {
        tokio::spawn(async move {
            let uws_loop = self.uws_loop;

            let callback = move || {
                let res = self.native.take().unwrap();
                res.end(data, close_connection);
            };

            loop_defer(uws_loop, callback);
        });
    }

    pub fn write_status(&self, status: &str) {
        if let Some(response) = self.native.as_ref() {
            response.write_status(status);
        }
    }

    pub fn write_header(&self, key: &str, value: &str) {
        if let Some(response) = self.native.as_ref() {
            response.write_header(key, value);
        }
    }

    pub fn write_header_int(&self, key: &str, value: u64) {
        if let Some(response) = self.native.as_ref() {
            response.write_header_int(key, value);
        }
    }

    pub fn end_without_body(&self, close_connection: bool) {
        if let Some(response) = self.native.as_ref() {
            response.end_without_body(close_connection);
        }
    }

    pub fn has_responded(&self) -> bool {
        if let Some(response) = self.native.as_ref() {
            response.has_responded()
        } else {
            true
        }
    }
}
