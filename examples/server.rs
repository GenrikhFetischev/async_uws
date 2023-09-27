use std::time::Duration;

use tokio::time::sleep;

use async_uws::app::App;
use async_uws::data_storage::DataStorage;
use async_uws::http_request::HttpRequest;
use async_uws::http_response::HttpResponse;
use async_uws::uwebsockets_rs::CompressOptions;
use async_uws::uwebsockets_rs::Opcode;
use async_uws::uwebsockets_rs::UsSocketContextOptions;
use async_uws::websocket::Websocket;
use async_uws::ws_behavior::WsRouteSettings;
use async_uws::ws_message::WsMessage;

#[derive(Clone)]
struct SharedData {
    pub data: String,
}

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

    let shared_data = SharedData {
        data: "String containing data".to_string(),
    };

    let mut app = App::new(opts);
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
    app.data(shared_data);

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
        .ws(
            "/ws",
            route_settings.clone(),
            |mut ws| async move {
                let status = ws.send("hello".into()).await;
                println!("Send status: {status:#?}");
                while let Some(msg) = ws.stream.recv().await {
                    println!("{msg:#?}");
                    ws.send(msg).await.unwrap();
                }
            },
            |req, per_connection_storage| {
                let full_url = req.get_full_url().to_string();
                let headers: Vec<(String, String)> = req
                    .get_headers()
                    .clone()
                    .into_iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect();

                let upgrade_req_info = UpgradeReqInfo { full_url, headers };

                per_connection_storage.add_data(upgrade_req_info);
            },
        )
        .ws("/ws-test", route_settings, handler_ws, upgrade_hook)
        .listen(3001, None::<fn()>)
        .run();
}

fn upgrade_hook(req: &mut HttpRequest, per_connection_storage: &mut DataStorage) {
    let full_url = req.get_full_url().to_string();
    let headers: Vec<(String, String)> = req
        .get_headers()
        .clone()
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();

    let upgrade_req_info = UpgradeReqInfo { full_url, headers };

    per_connection_storage.add_data(upgrade_req_info);
}

#[derive(Debug, Clone)]
struct UpgradeReqInfo {
    full_url: String,
    headers: Vec<(String, String)>,
}

async fn get_handler(res: HttpResponse<false>, req: HttpRequest) {
    let data = res.data::<SharedData>().unwrap();
    println!("!!! Shared data: {}", data.data);
    let path = req.get_full_url();
    println!("Handler started {path}");
    sleep(Duration::from_secs(1)).await;
    println!("Ready to respond");
    res.end(Some("it's the response"), true);
}

async fn handler_ws(mut ws: Websocket<false>) {
    let data = ws.data::<SharedData>().unwrap();
    println!("!!! Global Shared data: {}", data.data);
    let per_connection_data = ws.connection_data::<UpgradeReqInfo>().unwrap();
    println!("!!! Global Shared data: {:#?}", per_connection_data);

    while let Some(msg) = ws.stream.recv().await {
        match msg {
            WsMessage::Message(bin, opcode) => {
                if opcode == Opcode::Text {
                    let msg = String::from_utf8(bin).unwrap();
                    println!("{msg}");

                    if msg.contains("close") {
                        ws.send(WsMessage::Close(1003, Some("just close".to_string())))
                            .await
                            .unwrap();
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
        .await
        .unwrap();
    }
    println!("Done with that websocket!");
}
