use std::time::Duration;

use tokio::sync::{broadcast, oneshot};
use tokio::time::sleep;

use async_uws::app::App;
use async_uws::http_request::HttpRequest;
use async_uws::http_response::HttpResponse;
use async_uws::uwebsockets_rs::UsSocketContextOptions;

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

    let (sink, stream) = oneshot::channel::<()>();
    let (b_sink, mut b_stream) = broadcast::channel::<()>(1);
    tokio::spawn(async move {
        let _ = b_stream.recv().await;
        sink.send(()).unwrap();
    });
    let mut app = App::new(opts, Some(stream));
    app.data(shared_data);
    app.data(b_sink);

    app.get("/get", get_handler)
        .post("/post", post_handler)
        .post("/post/stream", body_stream)
        .get(
            "/closure",
            move |res: HttpResponse<false>, _req: HttpRequest| async {
                println!("Closure Handler started");
                sleep(Duration::from_secs(1)).await;
                println!("Closure Ready to respond");
                res.end(
                    Some("Closure it's the response".to_string().into_bytes()),
                    true,
                );
            },
        )
        .listen(
            3001,
            Some(|listen_socket| {
                println!("{listen_socket:#?}");
            }),
        )
        .run();
    println!("Server exiting");
}

async fn post_handler(mut res: HttpResponse<false>, _: HttpRequest) {
    let body = res.get_body().await.unwrap();
    let body_str = String::from_utf8(body).unwrap();
    println!("{body_str}");

    res.end(Some("THanks".into()), true);
}

async fn body_stream(mut res: HttpResponse<false>, _: HttpRequest) {
    let mut body_stream = res.get_body_stream().unwrap();

    while let Some((chunk, _)) = body_stream.recv().await {
        let chunk_str = match std::str::from_utf8(&chunk) {
            Ok(s) => s,
            Err(e) => {
                let valid_len = e.valid_up_to();
                std::str::from_utf8(&chunk[..valid_len])
                    .expect("[uwebsockets_rs] Can't read string from ptr")
            }
        };
        println!("{chunk_str}");
    }

    res.end(Some("Thanks".into()), true);
}

async fn get_handler(res: HttpResponse<false>, req: HttpRequest) {
    let data = res.data::<SharedData>().unwrap();
    println!("!!! Shared data: {}", data.data);
    let path = req.get_full_url();
    println!("Handler started {path}");
    sleep(Duration::from_secs(1)).await;
    println!("Ready to respond");
    res.end(Some("it's the response".into()), true);
}
