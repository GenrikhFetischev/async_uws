use std::time::Duration;

use tokio::time::sleep;
use uwebsockets_rs::websocket::Opcode;

use async_uws::app::App;
use async_uws::http_request::HttpRequest;
use async_uws::http_response::HttpResponse;
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

    let mut app = App::new(opts);
    let route_settings = WsRouteSettings::default();

    app.get("/get", get_handler)
        .post("/x", move |res, _req| async {
            res.end(Some("response post"), true);
        })
        .get(
            "/closure",
            move |res: HttpResponse<false>, _req: HttpRequest| async {
                println!("Closure Handler started");
                sleep(Duration::from_secs(1)).await;
                println!("Closure Ready to respond");
                res.end(Some("Closure it's the response"), true);
            },
        )
        .ws("/ws", route_settings.clone(), |mut ws| async move {
            let status = ws.send("hello".into()).await;
            println!("Send status: {status:#?}");
            while let Some(msg) = ws.stream.recv().await {
                println!("{msg:#?}");
                ws.send(msg).await;
            }
        })
        .ws("/ws-test", route_settings, handler_ws)
        .listen(3001, None::<fn()>)
        .run();
}

async fn get_handler(res: HttpResponse<false>, req: HttpRequest) {
    let path = req.get_full_url();
    println!("Handler started {path}");
    sleep(Duration::from_secs(1)).await;
    println!("Ready to respond");
    res.end(Some("it's the response"), true);
}

async fn handler_ws(mut ws: Websocket<false>) {
    while let Some(msg) = ws.stream.recv().await {
        match msg {
            WsMessage::Message(bin, opcode) => {
                if opcode == Opcode::Text {
                    let msg = String::from_utf8(bin).unwrap();
                    println!("{msg}");

                    if msg.contains("close") {
                        ws.send(WsMessage::Close(1003, Some("just close".to_string())))
                            .await;
                    }
                }
            }
            WsMessage::Ping(_) => {
                println!("Got ping");
            }
            WsMessage::Pong(_) => {
                println!("Got pong");
            }
            WsMessage::Close(code, reason) => {
                println!("Got close: {code}, {reason:#?}");
                break;
            }
        }
        ws.send(WsMessage::Message(
            Vec::from("response to your message".as_bytes()),
            Opcode::Text,
        ))
        .await;
    }
    println!("Done with that websocket!");
}
