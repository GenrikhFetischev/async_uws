use uwebsockets_rs::listen_socket::ListenSocket;

use async_uws::app::App;
use async_uws::http_response::HttpResponse;
use async_uws::uwebsockets_rs::CompressOptions;
use async_uws::uwebsockets_rs::UsSocketContextOptions;
use async_uws::websocket::Websocket;
use async_uws::ws_behavior::WsRouteSettings;
use async_uws::ws_message::WsMessage;

#[tokio::main]
async fn main() {
    let opts = UsSocketContextOptions {
        key_file_name: None,
        cert_file_name: None,
        passphrase: None,
        dh_params_file_name: None,
        ca_file_name: None,
        ssl_ciphers: None,
        ssl_prefer_low_memory_usage: None,
    };

    let mut app = App::new(opts, None);
    let compressor: u32 = CompressOptions::SharedCompressor.into();
    let decompressor: u32 = CompressOptions::SharedDecompressor.into();
    let route_settings = WsRouteSettings {
        compression: Some(compressor | decompressor),
        max_payload_length: Some(1024),
        idle_timeout: Some(800),
        max_backpressure: Some(10),
        close_on_backpressure_limit: Some(false),
        reset_idle_timeout_on_send: Some(true),
        send_pings_automatically: Some(true),
        max_lifetime: Some(111),
    };

    app.ws(
        "/",
        route_settings.clone(),
        handler_ws,
        HttpResponse::default_upgrade,
    )
    .listen(9001, None::<fn(ListenSocket)>)
    .run();
    println!("Server exiting");
}

async fn handler_ws(mut ws: Websocket<false>) {
    println!("New connection");
    while let Some(msg) = ws.stream.recv().await {
        if let WsMessage::Close(_, _) = msg {
            break;
        }
        let res = ws.send(msg).await;
        if res.is_err() {
            println!("1");
        }
    }
    println!("Done with that websocket!");
}
