// TODO: use async iterator as soon as it's stable
// use std::async_iter::AsyncIterator;

use std::time::Duration;

use tokio::sync::mpsc;
use tokio::sync::mpsc::Receiver;
use uwebsockets_rs::http_response::HttpResponseStruct;

pub type BodyChunk = (Vec<u8>, bool);

pub struct BodyReader<const SSL: bool> {
    body_stream: Receiver<BodyChunk>,
}

impl<const SSL: bool> BodyReader<SSL> {
    pub fn new(mut response: HttpResponseStruct<SSL>) -> Self {
        let (sink, stream) = mpsc::channel(1);
        response.on_data(move |chunk, end| {
            let chunk = chunk.to_vec();
            let sink = sink.clone();
            tokio::spawn(async move {
                let res = sink.send_timeout((chunk, end), Duration::from_millis(50))
                    .await;
                if let Err(e) = res {
                    eprintln!("[async_uws] Error sending body chunk to stream: {e:#?}");
                }

            });
        });

        BodyReader {
            body_stream: stream,
        }
    }

    pub fn take_stream(self) -> Receiver<BodyChunk> {
        self.body_stream
    }

    pub async fn collect(self) -> Option<Vec<u8>> {
        let mut data_collector = Vec::<u8>::new();
        let mut stream = self.take_stream();
        while let Some((chunk, is_fin)) = stream.recv().await {
            // TODO: Consider use append instead of extend, in order to avoid additional memory allocation
            data_collector.extend(&chunk);
            if is_fin {
                break;
            }
        }

        if data_collector.is_empty() {
            None
        } else {
            Some(data_collector)
        }
    }
}
