use std::time::Duration;

use tokio::time::sleep;

use async_uws::app::App;
use async_uws::http_request::HttpRequest;
use async_uws::http_response::HttpResponse;
use async_uws::uwebsockets_rs::UsSocketContextOptions;

#[tokio::main]
async fn main() {
    print!("\x1B[2J\x1B[1;1H");
    println!("/////");
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
