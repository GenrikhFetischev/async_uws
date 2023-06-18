use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use uwebsockets_rs::app::Application as NativeApp;
use uwebsockets_rs::http_request::HttpRequest;
use uwebsockets_rs::http_response::HttpResponseStruct;
use uwebsockets_rs::us_socket_context_options::UsSocketContextOptions;
use uwebsockets_rs::uws_loop::{get_loop, UwsLoop};

use crate::http_response::HttpResponse;
use crate::send_ptr::SendPtr;

pub type App = AppStruct<false>;
pub type AppSSL = AppStruct<true>;

pub struct AppStruct<const SSL: bool> {
    uws_loop: UwsLoop,
    native_app: NativeApp<SSL>,
}

impl<const SSL: bool> AppStruct<SSL> {
    pub fn new(sockets_config: UsSocketContextOptions) -> Self {
        let uws_loop = get_loop();
        let native_app = NativeApp::<SSL>::new(sockets_config);
        AppStruct {
            uws_loop,
            native_app,
        }
    }

    pub fn ws(&mut self, _pattern: &str) {
        todo!()
    }

    pub fn get<T, W>(&mut self, pattern: &str, handler: T) -> &mut Self
    where
        T: (Fn(HttpResponse<SSL>, HttpRequest) -> W) + 'static + Send + Sync,
        W: Future<Output = ()> + 'static + Send,
    {
        let internal_handler = wrap_handler(handler, self.uws_loop);
        self.native_app.get(pattern, internal_handler);
        self
    }

    pub fn post<T, W>(&mut self, pattern: &str, handler: T) -> &mut Self
    where
        T: (Fn(HttpResponse<SSL>, HttpRequest) -> W) + 'static + Send + Sync,
        W: Future<Output = ()> + 'static + Send,
    {
        let internal_handler = wrap_handler(handler, self.uws_loop);
        self.native_app.post(pattern, internal_handler);
        self
    }

    pub fn patch<T, W>(&mut self, pattern: &str, handler: T) -> &mut Self
    where
        T: (Fn(HttpResponse<SSL>, HttpRequest) -> W) + 'static + Send + Sync,
        W: Future<Output = ()> + 'static + Send,
    {
        let internal_handler = wrap_handler(handler, self.uws_loop);
        self.native_app.patch(pattern, internal_handler);
        self
    }

    pub fn delete<T, W>(&mut self, pattern: &str, handler: T) -> &mut Self
    where
        T: (Fn(HttpResponse<SSL>, HttpRequest) -> W) + 'static + Send + Sync,
        W: Future<Output = ()> + 'static + Send,
    {
        let internal_handler = wrap_handler(handler, self.uws_loop);
        self.native_app.delete(pattern, internal_handler);
        self
    }

    pub fn options<T, W>(&mut self, pattern: &str, handler: T) -> &mut Self
    where
        T: (Fn(HttpResponse<SSL>, HttpRequest) -> W) + 'static + Send + Sync,
        W: Future<Output = ()> + 'static + Send,
    {
        let internal_handler = wrap_handler(handler, self.uws_loop);
        self.native_app.options(pattern, internal_handler);
        self
    }

    pub fn put<T, W>(&mut self, pattern: &str, handler: T) -> &mut Self
    where
        T: (Fn(HttpResponse<SSL>, HttpRequest) -> W) + 'static + Send + Sync,
        W: Future<Output = ()> + 'static + Send,
    {
        let internal_handler = wrap_handler(handler, self.uws_loop);
        self.native_app.put(pattern, internal_handler);
        self
    }

    pub fn trace<T, W>(&mut self, pattern: &str, handler: T) -> &mut Self
    where
        T: (Fn(HttpResponse<SSL>, HttpRequest) -> W) + 'static + Send + Sync,
        W: Future<Output = ()> + 'static + Send,
    {
        let internal_handler = wrap_handler(handler, self.uws_loop);
        self.native_app.trace(pattern, internal_handler);
        self
    }

    pub fn connect<T, W>(&mut self, pattern: &str, handler: T) -> &mut Self
    where
        T: (Fn(HttpResponse<SSL>, HttpRequest) -> W) + 'static + Send + Sync,
        W: Future<Output = ()> + 'static + Send,
    {
        let internal_handler = wrap_handler(handler, self.uws_loop);
        self.native_app.connect(pattern, internal_handler);
        self
    }

    pub fn any<T, W>(&mut self, pattern: &str, handler: T) -> &mut Self
    where
        T: (Fn(HttpResponse<SSL>, HttpRequest) -> W) + 'static + Send + Sync,
        W: Future<Output = ()> + 'static + Send,
    {
        let internal_handler = wrap_handler(handler, self.uws_loop);
        self.native_app.any(pattern, internal_handler);
        self
    }

    pub fn run(&mut self) {
        self.native_app.run();
    }

    pub fn listen(&mut self, port: u16, handler: Option<impl Fn() + Unpin + 'static>) -> &mut Self {
        self.native_app.listen(port as i32, handler);
        self
    }
}

pub fn wrap_handler<T, R, const SSL: bool>(
    handler: T,
    uws_loop: UwsLoop,
) -> Box<dyn Fn(HttpResponseStruct<SSL>, HttpRequest)>
where
    T: (Fn(HttpResponse<SSL>, HttpRequest) -> R) + 'static + Send + Sync,
    R: Future<Output = ()> + 'static + Send,
{
    let handler = Box::new(handler);
    let handler_wrapper = SendPtr {
        ptr: Box::into_raw(handler),
    };

    let handler = move |mut res: HttpResponseStruct<SSL>, req: HttpRequest| {
        let is_aborted = Arc::new(AtomicBool::new(false));

        let is_aborted_to_move = is_aborted.clone();
        res.on_aborted(move || {
            is_aborted_to_move.store(true, Ordering::Relaxed);
        });

        tokio::spawn(async move {
            let res = HttpResponse::new(res, uws_loop, is_aborted);
            let handler_wrapper = handler_wrapper;
            let handler = unsafe { handler_wrapper.ptr.as_ref().unwrap() };
            handler(res, req).await;
        });
    };
    Box::new(handler)
}
