use std::time::Duration;

use tokio::time::sleep;
use uwebsockets_rs::websocket::Opcode;

use async_uws::app::App;
use async_uws::http_request::HttpRequest;
use async_uws::http_response::HttpResponse;
use async_uws::uwebsockets_rs::UsSocketContextOptions;
use async_uws::websocket::Websocket;
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

    app.get("/get", get_handler)
        .post("/x", move |res, req| async {
            res.end(Some("response post"), true);
        })
        .get(
            "/closure",
            move |res: HttpResponse<false>, req: HttpRequest| async {
                println!("Closure Handler started");
                sleep(Duration::from_secs(1)).await;
                println!("Closure Ready to respond");
                res.end(Some("Closure it's the response"), true);
            },
        )
        .ws("/ws", |mut ws| async move {
            let to_send = WsMessage::Message(Vec::from("hello".as_bytes()), Opcode::Binary);
            ws.sink.send(to_send).expect("FAIL");
            while let Some(msg) = ws.stream.recv().await {
                println!("{msg:#?}");
                ws.sink.send(msg).unwrap();
            }
        })
        .ws("/ws-echo", |mut ws| async move { ws_echo(&mut ws).await })
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

async fn ws_echo(ws: &mut Websocket<false>) {
    while let Some(msg) = ws.stream.recv().await {
        match msg {
            WsMessage::Message(bin, opcode) => {
                if opcode == Opcode::Text {
                    let msg = String::from_utf8(bin).unwrap();
                    println!("{msg}");

                    if msg.contains("close") {
                        let res = ws
                            .sink
                            .send(WsMessage::Close(1003, Some("just close".to_string())));
                        if let Err(e) = res {
                            println!("{e}");
                        }
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
        ws.sink
            .send(WsMessage::Message(
                Vec::from("response to your message".as_bytes()),
                Opcode::Text,
            ))
            .unwrap()
    }
    println!("Done with that websocket!");
}
