/// IRC Server Actor

use tokio::sync::{watch, mpsc};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use crate::core::error::AppError;
use crate::core::state::GlobalState;
use crate::core::event::Event;

/// IRC Server Actor
pub struct IrcServerActor {
    port: u16,
    state_tx: watch::Sender<GlobalState>,
    event_tx: mpsc::Sender<Event>,
    /// 系统通知接收端 — 状态机通知发送通道
    notify_rx: mpsc::Receiver<String>,
}

impl IrcServerActor {
    pub fn new(
        port: u16,
        state_tx: watch::Sender<GlobalState>,
        event_tx: mpsc::Sender<Event>,
        notify_rx: mpsc::Receiver<String>,
    ) -> Self {
        Self { port, state_tx, event_tx, notify_rx }
    }

    /// 运行 IRC Server Actor
    pub async fn run(mut self) -> Result<(), AppError> {
        let addr = format!("0.0.0.0:{}", self.port);
        let listener = tokio::net::TcpListener::bind(&addr).await?;
        eprintln!("[IRC:Srv] Listening on {}", addr);

        let writers: Arc<Mutex<Vec<(tokio::net::tcp::OwnedWriteHalf, String)>>> =
            Arc::new(Mutex::new(Vec::new()));

        loop {
            tokio::select! {
                // 新 TCP 连接
                accept_result = listener.accept() => {
                    let (stream, socket_addr) = accept_result?;
                    let client_addr = socket_addr.to_string();

                    let is_ps5 = is_ps5_connection(&socket_addr);

                    let (read_half, write_half) = stream.into_split();
                    let writers_clone = Arc::clone(&writers);
                    let state_tx_clone = self.state_tx.clone();
                    let event_tx_clone = self.event_tx.clone();
                    let is_ps5_conn = is_ps5;
                    let client_addr_clone = client_addr.clone();

                    {
                        let mut writers_guard = writers.lock().await;
                        writers_guard.push((write_half, client_addr.clone()));
                    }

                    tokio::spawn(async move {
                        handle_irc_client(
                            read_half,
                            writers_clone,
                            state_tx_clone,
                            event_tx_clone,
                            is_ps5_conn,
                            client_addr_clone,
                        ).await;
                    });
                }

                // 系统通知 - 来自状态机
                notify = self.notify_rx.recv() => {
                    match notify {
                        Some(msg) => {
                            // 广播通知
                            let irc_msg = format!(
                                ":PSLinkB!PSLinkB@PSLinkB.tmi.twitch.tv PRIVMSG {} :{}\r\n",
                                self.state_tx.borrow().channel_name, msg
                            );
                            broadcast_message(&writers, irc_msg.as_bytes()).await;
                        }
                        None => {
                            eprintln!("[IRC:Srv] Notify channel closed");
                        }
                    }
                }
            }
        }
    }
}

fn is_ps5_connection(addr: &std::net::SocketAddr) -> bool {
    !addr.ip().is_loopback()
}

/// 处理单个 IRC 客户端
async fn handle_irc_client(
    mut reader: tokio::net::tcp::OwnedReadHalf,
    writers: Arc<Mutex<Vec<(tokio::net::tcp::OwnedWriteHalf, String)>>>,
    state_tx: watch::Sender<GlobalState>,
    event_tx: mpsc::Sender<Event>,
    is_ps5: bool,
    client_addr: String,
) {
    let mut buffer = [0u8; 1024];
    
    loop {
        match reader.read(&mut buffer).await {
            Ok(0) => {
                if is_ps5 {
                    eprintln!("[IRC:Srv] PS5 disconnected: {}", client_addr);
                    state_tx.send_modify(|state| {
                        state.channel_name.clear();
                    });
                }

                break;
            }
            Ok(n) => {
                let message = String::from_utf8_lossy(&buffer[..n]);
                let message = message.trim();
                
                if message.is_empty() {
                    continue;
                }
                
                // 处理 PASS 命令 - 握手响应
                if message.starts_with("PASS ") {
                    // 欢迎消息
                    let welcome = ":tmi.twitch.tv 001 urlynn :Welcome, GLHF!\r\n";
                    broadcast_message(&writers, welcome.as_bytes()).await;
                }
                // 处理 JOIN 命令 - 提取频道名
                else if message.starts_with("JOIN ") {
                    let parts: Vec<&str> = message.split_whitespace().collect();
                    if parts.len() >= 2 {
                        let channel = parts[1].to_string();
                        eprintln!("[IRC:Srv] PS5 Connected - {} | Channel - {}", client_addr, channel);
                        
                        state_tx.send_modify(|state| {
                            state.channel_name = channel.clone();
                        });
                        let _ = event_tx.send(Event::Ps5IrcReady { channel: channel.clone() }).await;
                        
                        // === 标准 IRC 握手响应 ===
                        
                        // 1. 发送 JOIN 确认
                        let join_response = format!(
                            ":urlynn!urlynn@urlynn.tmi.twitch.tv JOIN {}\r\n",
                            channel
                        );
                        broadcast_message(&writers, join_response.as_bytes()).await;
                        
                        // 2. 发送房间主题
                        let topic = format!(
                            ":tmi.twitch.tv 332 urlynn {} :PSLinkB Live Streaming\r\n",
                            channel
                        );
                        broadcast_message(&writers, topic.as_bytes()).await;
                        
                        // 3. 发送服务就绪 PRIVMSG - 自定义为加入频道通知
                        let ready_msg = format!(
                            ":PSLinkB PRIVMSG {} :已加入频道{}\r\n",
                            channel, channel
                        );
                        broadcast_message(&writers, ready_msg.as_bytes()).await;
                    }
                }
                // 处理 PING 命令
                else if message.starts_with("PING ") {
                    let pong = message.replace("PING", "PONG");
                    let response = format!("{}\r\n", pong);
                    broadcast_message(&writers, response.as_bytes()).await;
                }
                // 其他消息原样广播
                else {
                    let response = format!("{}\r\n", message);
                    broadcast_message(&writers, response.as_bytes()).await;
                }
            }
            Err(e) => {
                eprintln!("[IRC:Srv] Read error from {}: {}", client_addr, e);
                break;
            }
        }
    }
}

/// 广播消息
async fn broadcast_message(
    writers: &Arc<Mutex<Vec<(tokio::net::tcp::OwnedWriteHalf, String)>>>,
    data: &[u8],
) {
    let mut writers_guard = writers.lock().await;
    let mut to_remove = Vec::new();
    
    for (i, (writer, _)) in writers_guard.iter_mut().enumerate() {
        if writer.write_all(data).await.is_err() {
            to_remove.push(i);
        }
    }
    
    for i in to_remove.iter().rev() {
        writers_guard.remove(*i);
    }
}
