/// Danmaku Formatter — 独立的弹幕日志组件（舍不得删）

use crate::core::channel::{DanmuMessage, DanmuReceiver};
use crate::core::error::AppError;
use blivemsg::types::Message;

pub struct DanmakuFormatter {
    danmu_rx: Box<dyn DanmuReceiver>,
}

impl DanmakuFormatter {
    pub fn new(danmu_rx: Box<dyn DanmuReceiver>) -> Self {
        Self { danmu_rx }
    }

    pub async fn run(mut self) -> Result<(), AppError> {
        loop {
            match self.danmu_rx.recv_danmu().await {
                Some(DanmuMessage::Danmaku(msg)) => {
                    if let Some(formatted) = Self::format_danmaku(&msg) {
                        eprintln!("[Danmu:Fmt] {}", formatted);
                    }
                }
                Some(DanmuMessage::Notify(_)) => {
                    // notifications go to PS5 via IRC, not terminal
                }
                None => break,
            }
        }
        Ok(())
    }

    fn format_danmaku(msg: &Message) -> Option<String> {
        match msg {
            Message::Danmu(d) => Some(format!("{}: {}", d.username, d.content)),
            Message::Gift(g) => Some(format!("[礼物] {} {}了 {} x{}", g.username, g.action, g.gift_name, g.num)),
            Message::ComboSend(c) => Some(format!("[连击] {} {} x{}", c.username, c.action, c.combo_num)),
            Message::SuperChat(sc) => Some(format!("[SC] {} ¥{}: {}", sc.username, sc.price, sc.content)),
            Message::GuardBuy(g) => Some(format!("[舰长] {} 购买了 {} 个", g.username, g.num)),
            Message::LikeInfoV3Click(l) => Some(format!("[点赞] {} 点了 {} 个赞", l.username, l.click_count)),
            Message::WelcomeGuard(w) => Some(format!("[欢迎] {} 进入直播间", w.username)),
            Message::EntryEffect(e) => Some(format!("[入场] {}", e.copy_writing)),

            #[cfg(feature = "protobuf-support")]
            Message::InteractWordV2(i) => Some(format!("[进入] {}", i.username)),

            _ => None,
        }
    }
}
