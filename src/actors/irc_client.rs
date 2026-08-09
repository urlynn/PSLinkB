//! IRC Client Worker

use crate::core::channel::{DanmuMessage, DanmuReceiver};
use crate::core::error::AppError;
use crate::core::state::GlobalState;
#[cfg(feature = "channel-mpsc")]
use crate::log;
use blivemsg::types::Message;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::sync::watch;

pub struct IrcClientWorker {
    state_rx: watch::Receiver<GlobalState>,
    message_rx: Box<dyn DanmuReceiver>,
    #[cfg(feature = "channel-mpsc")]
    first: bool,
}

impl IrcClientWorker {
    pub fn new(
        state_rx: watch::Receiver<GlobalState>,
        message_rx: Box<dyn DanmuReceiver>,
    ) -> Self {
        Self {
            state_rx,
            message_rx,
            #[cfg(feature = "channel-mpsc")]
            first: true,
        }
    }

    pub async fn run(mut self) -> Result<(), AppError> {
        loop {
            let channel_name = loop {
                let name = self.state_rx.borrow().channel_name.clone();
                if !name.is_empty() {
                    break name;
                }
                if self.state_rx.changed().await.is_err() {
                    return Ok(());
                }
            };

            // 连接 IRC Server
            let stream = match TcpStream::connect("127.0.0.1:6667").await {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("[IRC:Cli] Connect failed: {}", e);
                    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                    continue;
                }
            };
            let (_, mut writer) = tokio::io::split(stream);
            eprintln!("[IRC:Cli] Connected (internal)");

            #[cfg(feature = "channel-mpsc")]
            { self.first = true; }

            loop {
                tokio::select! {
                    biased;
                    _ = self.state_rx.changed() => {
                        if self.state_rx.borrow().channel_name.is_empty() {
                            eprintln!("[IRC:Cli] Disconnected - channel_name cleared");
                            break;
                        }
                    }
                    msg = self.message_rx.recv_danmu() => {
                        match msg {
                            Some(danmu_msg) => {
                                #[cfg(feature = "channel-mpsc")]
                                if self.first
                                    && let DanmuMessage::Danmaku(Message::Danmu(ref d)) = danmu_msg
                                {
                                    self.first = false;
                                    log!(ok, "[Danmaku] {}: {} - ✓ 首条弹幕工作正常", d.username, d.content);
                                }
                                if let Some(irc_msg) = Self::format_message(&danmu_msg, &channel_name)
                                    && writer.write_all(irc_msg.as_bytes()).await.is_err()
                                {
                                    eprintln!("[IRC:Cli] Write failed - PS5 disconnected");
                                    break;
                                }
                            }
                            None => {
                                eprintln!("[IRC:Cli] Channel closed");
                                return Ok(());
                            }
                        }
                    }
                }
            }
        }
    }

    fn format_message(msg: &DanmuMessage, channel: &str) -> Option<String> {
        match msg {
            DanmuMessage::Danmaku(blive_msg) => Self::format_danmaku(blive_msg, channel),
        }
    }

    fn format_danmaku(msg: &Message, channel: &str) -> Option<String> {
        match msg {
            Message::Danmu(d) => Some(format!(
                ":{}!{}@{}.tmi.twitch.tv PRIVMSG {} :{}\r\n",
                d.username, d.username, d.username, channel, d.content
            )),
            Message::Gift(g) => Some(format!(
                ":{}!{}@{}.tmi.twitch.tv PRIVMSG {} :{}了 {} x{}\r\n",
                g.username, g.username, g.username, channel, g.action, g.gift_name, g.num
            )),
            Message::ComboSend(c) => Some(format!(
                ":{}!{}@{}.tmi.twitch.tv PRIVMSG {} :{} x{}\r\n",
                c.username, c.username, c.username, channel, c.action, c.combo_num
            )),
            Message::SuperChat(sc) => Some(format!(
                ":{}!{}@{}.tmi.twitch.tv PRIVMSG {} :[SC ¥{}] {}\r\n",
                sc.username, sc.username, sc.username, channel, sc.price, sc.content
            )),
            Message::GuardBuy(g) => Some(format!(
                ":{}!{}@{}.tmi.twitch.tv PRIVMSG {} :购买了 {} 个舰长\r\n",
                g.username, g.username, g.username, channel, g.num
            )),
            Message::LikeInfoV3Click(l) => Some(format!(
                ":{}!{}@{}.tmi.twitch.tv PRIVMSG {} :点了 {} 个赞\r\n",
                l.username, l.username, l.username, channel, l.click_count
            )),
            Message::WelcomeGuard(w) => Some(format!(
                ":{}!{}@{}.tmi.twitch.tv PRIVMSG {} :欢迎进入直播间\r\n",
                w.username, w.username, w.username, channel
            )),
            Message::EntryEffect(e) => Some(format!(
                ":entry!entry@entry.tmi.twitch.tv PRIVMSG {} :{}\r\n",
                channel, e.copy_writing
            )),
            #[cfg(feature = "protobuf-support")]
            Message::InteractWordV2(i) => Some(format!(
                ":{}!{}@{}.tmi.twitch.tv PRIVMSG {} :进入了直播间\r\n",
                i.username, i.username, i.username, channel
            )),
            _ => None,
        }
    }
}
