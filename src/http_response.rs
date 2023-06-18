use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use uwebsockets_rs::http_response::HttpResponseStruct;
use uwebsockets_rs::uws_loop::{loop_defer, UwsLoop};

pub struct HttpResponse<const SSL: bool> {
    pub(crate) native: Option<HttpResponseStruct<SSL>>,
    pub(crate) uws_loop: UwsLoop,
    pub is_aborted: Arc<AtomicBool>,
}

unsafe impl<const SSL: bool> Sync for HttpResponse<SSL> {}
unsafe impl<const SSL: bool> Send for HttpResponse<SSL> {}

impl<const SSL: bool> HttpResponse<SSL> {
    pub fn new(
        native_response: HttpResponseStruct<SSL>,
        uws_loop: UwsLoop,
        is_aborted: Arc<AtomicBool>,
    ) -> Self {
        HttpResponse {
            native: Some(native_response),
            is_aborted,
            uws_loop,
        }
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
}
