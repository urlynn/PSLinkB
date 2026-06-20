/// Danmu Worker

use blivemsg::BliveClient;
use futures_util::StreamExt;
use tokio::sync::mpsc;
use crate::core::channel::{DanmuMessage, DanmuSender};
use crate::core::event::Event;
use crate::core::error::AppError;
use crate::log;

pub struct DanmuWorker {
    room_id: u64,
    cookie_string: String,
    sender: Box<dyn DanmuSender>,
    event_tx: mpsc::Sender<Event>,
}

impl DanmuWorker {
    pub fn new(
        room_id: u64,
        cookie_string: String,
        sender: Box<dyn DanmuSender>,
        event_tx: mpsc::Sender<Event>,
    ) -> Self {
        Self { room_id, cookie_string, sender, event_tx }
    }

    pub async fn run(mut self) -> Result<(), AppError> {
        loop {
            match self.connect_and_stream().await {
                Ok(()) => {
                    log!(warn, "Danmaku: Stream ended, reconnecting...");
                }
                Err(e) => {
                    log!(warn, "Danmaku: Connect failed: {}, reconnecting...", e);
                }
            }

            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        }
    }

    async fn connect_and_stream(&mut self) -> Result<(), AppError> {
        let mut client = BliveClient::from_cookie_string(self.room_id, self.cookie_string.clone())?;
        let mut stream = client.stream().await?;

        eprintln!("[Danmaku] Connected room {}", self.room_id);
        let _ = self.event_tx.send(Event::DanmakuReady).await;

        while let Some(msg) = stream.next().await {
            if self.sender.send_danmu(DanmuMessage::Danmaku(msg)).await.is_err() {
                eprintln!("[Danmaku] Receiver closed");
                break;
            }
        }

        eprintln!("[Danmaku] Stream ended");
        Ok(())
    }
}
