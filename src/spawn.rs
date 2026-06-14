//! Worker 启动函数

use crate::core::error::AppError;
use crate::core::biliapi;
use crate::log_error;
#[cfg(feature = "channel-broadcast")]
use crate::log_warn;

use tokio::sync::mpsc;

use crate::config::{RTMP_PORT, IRC_PORT};
use crate::core::channel::DanmuTx;
use crate::core::event::Event;

// ————————————————————————————————————————————————————————————
// 24/7 服务
// ————————————————————————————————————————————————————————————

pub fn spawn_rtmp_server(event_tx: mpsc::Sender<Event>) {
    let (rtmp_tx, mut rtmp_rx) = mpsc::channel::<crate::actors::rtmp::StreamEvent>(32);

    tokio::spawn(async move {
        let actor = crate::actors::rtmp::RtmpActor::new(RTMP_PORT, rtmp_tx);
        if let Err(e) = actor.run().await {
            log_error!("RTMP: {}", AppError::crash("Server", e.to_string()));
        }
    });

    // 转换器: StreamEvent -> Event
    tokio::spawn(async move {
        while let Some(se) = rtmp_rx.recv().await {
            let ev = match se.event_type {
                crate::actors::rtmp::StreamEventType::Started => Event::RtmpPublish {
                    app: se.app,
                    stream_key: se.stream_key,
                },
                crate::actors::rtmp::StreamEventType::Stopped => Event::RtmpUnpublish,
            };
            if event_tx.send(ev).await.is_err() {
                break;
            }
        }
    });
}

pub fn spawn_irc_server(
    irc_state_tx: tokio::sync::watch::Sender<crate::core::state::GlobalState>,
    irc_notify_rx: mpsc::Receiver<String>,
    event_tx: mpsc::Sender<Event>,
) {
    tokio::spawn(async move {
        let actor = crate::actors::irc_server::IrcServerActor::new(IRC_PORT, irc_state_tx, event_tx, irc_notify_rx);
        if let Err(e) = actor.run().await {
            log_error!("IRC: {}", AppError::crash("Server", e.to_string()));
        }
    });
}

// ————————————————————————————————————————————————————————————
// 按需 Workers
// ————————————————————————————————————————————————————————————

pub fn spawn_ffmpeg_worker(mut cmd_rx: mpsc::Receiver<crate::system::FfmpegCmd>, event_tx: mpsc::Sender<Event>) {
    tokio::spawn(async move {
        let mut actor_tx: Option<mpsc::Sender<crate::actors::ffmpeg::FfmpegCommand>> = None;
        let mut sd_tx: Option<tokio::sync::broadcast::Sender<()>> = None;

        while let Some(cmd) = cmd_rx.recv().await {
            match cmd {
                crate::system::FfmpegCmd::Start {
                    ps5_app,
                    ps5_stream_key,
                    bilibili_rtmp_url,
                    bilibili_stream_key,
                } => {
                    let (tx, rx) = mpsc::channel(8);
                    let actor = crate::actors::ffmpeg::FfmpegActor::new(rx, event_tx.clone());
                    let (sdt, sdr) = tokio::sync::broadcast::channel(1);

                    tokio::spawn(async move {
                        if let Err(e) = actor.run(sdr).await {
                            log_error!("FFmpeg: {}", AppError::crash("Worker", e.to_string()));
                        }
                    });

                    let _ = tx
                        .send(crate::actors::ffmpeg::FfmpegCommand::Start {
                            ps5_app,
                            ps5_stream_key,
                            bilibili_rtmp_url,
                            bilibili_stream_key,
                        })
                        .await;

                    actor_tx = Some(tx);
                    sd_tx = Some(sdt);
                }
                crate::system::FfmpegCmd::Stop => {
                    if let Some(tx) = &actor_tx {
                        let _ = tx
                            .send(crate::actors::ffmpeg::FfmpegCommand::Stop)
                            .await;
                    }
                    if let Some(tx) = sd_tx.take() {
                        let _ = tx.send(());
                    }
                    actor_tx = None;
                }
            }
        }
    });
}

pub fn spawn_bilibili_worker(
    mut cmd_rx: mpsc::Receiver<crate::system::BilibiliCmd>,
    event_tx: mpsc::Sender<Event>,
    cookie_string: String,
) {
    let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

    tokio::spawn(async move {
        while let Some(cmd) = cmd_rx.recv().await {
            let cookie_c = cookie_string.clone();
            let event_tx_c = event_tx.clone();
            let cancel_c = cancel.clone();

            tokio::spawn(async move {
                match cmd {
                    crate::system::BilibiliCmd::StartLive {
                        room_id,
                        area_v2,
                        title,
                    } => {
                        cancel_c.store(false, std::sync::atomic::Ordering::Relaxed);
                        execute_start_live(room_id, area_v2, title, cookie_c, event_tx_c, cancel_c).await;
                    }
                    crate::system::BilibiliCmd::StopLive { room_id } => {
                        cancel_c.store(true, std::sync::atomic::Ordering::Relaxed);
                        execute_stop_live(room_id, cookie_c, event_tx_c).await;
                    }
                }
            });
        }
    });
}

pub async fn execute_start_live(
    room_id: u64,
    area_v2: String,
    title: Option<String>,
    cookie: String,
    event_tx: mpsc::Sender<Event>,
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
) {
    use crate::actors::blive::{BLiveManager, LiveCommand, LiveEvent};

    let (cmd_tx, cmd_rx) = mpsc::channel(8);
    let (ev_tx, mut ev_rx) = mpsc::channel(8);
    let manager = BLiveManager::new(cmd_rx, ev_tx, cookie, cancel);
    let (sd_tx, _) = tokio::sync::broadcast::channel(1);

    tokio::spawn(async move {
        if let Err(e) = manager.run(sd_tx.subscribe()).await {
            log_error!("Bili:Live: {}", AppError::crash("Manager", e.to_string()));
        }
    });

    let _ = cmd_tx
        .send(LiveCommand::Start {
            room_id,
            area_v2,
            title,
        })
        .await;

    while let Some(ev) = ev_rx.recv().await {
        match ev {
            LiveEvent::Started {
                rtmp_url,
                stream_key,
            } => {
                let _ = event_tx
                    .send(Event::BilibiliLiveStarted {
                        rtmp_url: rtmp_url.clone(),
                        stream_key: stream_key.clone(),
                    })
                    .await;
                // 启动流状态监听
                let ev_tx2 = event_tx.clone();
                tokio::spawn(async move {
                    monitor_stream_status(room_id, ev_tx2).await;
                });
                break;
            }
            LiveEvent::AuthRequired { face_auth_url } => {
                let _ = event_tx
                    .send(Event::BilibiliAuthRequired { face_auth_url })
                    .await;
            }
            LiveEvent::StartFailed {
                error_code,
                message,
            } => {
                let _ = event_tx
                    .send(Event::BilibiliLiveStartFailed {
                        code: error_code,
                        message,
                    })
                    .await;
                break;
            }
            _ => {}
        }
    }
}

pub async fn execute_stop_live(
    room_id: u64,
    cookie: String,
    event_tx: mpsc::Sender<Event>,
) {
    use crate::actors::blive::{BLiveManager, LiveCommand, LiveEvent};

    let (cmd_tx, cmd_rx) = mpsc::channel(8);
    let (ev_tx, mut ev_rx) = mpsc::channel(8);
    let manager = BLiveManager::new(cmd_rx, ev_tx, cookie, std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)));
    let (sd_tx, _) = tokio::sync::broadcast::channel(1);

    tokio::spawn(async move {
        if let Err(e) = manager.run(sd_tx.subscribe()).await {
            log_error!("Bili:Live: {}", AppError::crash("Manager", e.to_string()));
        }
    });

    let _ = cmd_tx.send(LiveCommand::Stop { room_id }).await;

    while let Some(ev) = ev_rx.recv().await {
        match ev {
            LiveEvent::Stopped => {
                let _ = event_tx.send(Event::BilibiliLiveStopped).await;
                break;
            }
            LiveEvent::StopFailed {
                error_code,
                message,
            } => {
                let _ = event_tx
                    .send(Event::BilibiliLiveStopFailed {
                        code: error_code,
                        message,
                    })
                    .await;
                break;
            }
            _ => {}
        }
    }
}

/// 流状态监听：开播后轮询 format count，超时后 FLV 探测 fallback
async fn monitor_stream_status(room_id: u64, event_tx: mpsc::Sender<Event>) {
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    crate::luci::set("stream", "fake");

    // 主检测：format 计数 ≥3
    for attempt in 1..=8 {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        match biliapi::get_stream_info(room_id).await {
            Ok(biliapi::StreamInfo::Live) => {
                eprintln!("[Bili:Live] Live stream confirmed - GetStreamInfo, {} - ✓ 直播视频流验证成功", attempt);
                crate::luci::set("stream", "live");
                let _ = event_tx.send(Event::BilibiliStreamConfirmed { room_id }).await;
                return;
            }
            Ok(biliapi::StreamInfo::Offline) => {
                crate::luci::set("stream", "offline");
                return;
            }
            _ => continue,
        }
    }

    // 5s 超时 -> 确认未关播后才 fallback
    if let Ok(biliapi::StreamInfo::Offline) = biliapi::get_stream_info(room_id).await {
        crate::luci::set("stream", "offline");
        return;
    }
    crate::luci::set("stream", "probing");
    if biliapi::flv_probe(room_id).await {
        eprintln!("[Bili:Live] Live stream confirmed - PlayUrl - ✓ 直播视频流验证成功");
        crate::luci::set("stream", "live");
        let _ = event_tx.send(Event::BilibiliStreamConfirmed { room_id }).await;
    } else {
        crate::luci::set("stream", "timeout");
        let _ = event_tx.send(Event::BilibiliStreamTimeout { room_id }).await;
    }
}

pub fn spawn_danmaku_worker(
    mut cmd_rx: mpsc::Receiver<crate::system::DanmakuCmd>,
    danmaku_tx: DanmuTx,
    cookie_string: String,
    event_tx: mpsc::Sender<Event>,
) {
    tokio::spawn(async move {
        let mut danmaku_handle: Option<tokio::task::JoinHandle<()>> = None;

        while let Some(cmd) = cmd_rx.recv().await {
            match cmd {
                crate::system::DanmakuCmd::Start { room_id } => {
                    if danmaku_handle.is_none() {
                        let tx = danmaku_tx.clone();
                        let cookie = cookie_string.clone();
                        let ev_tx = event_tx.clone();

                        let handle = tokio::spawn(async move {
                            let sender = Box::new(tx);
                            let worker = crate::actors::danmaku::DanmuWorker::new(
                                room_id, cookie, sender, ev_tx,
                            );
                            if let Err(e) = worker.run().await {
                                log_error!("Danmaku: {}", AppError::crash("Worker", e.to_string()));
                            }
                        });
                        danmaku_handle = Some(handle);
                    }
                }
                crate::system::DanmakuCmd::Stop => {
                    if let Some(h) = danmaku_handle.take() {
                        h.abort();
                    }
                }
            }
        }
    });
}

// ————————————————————————————————————————————————————————————
// IRC 客户端
// ————————————————————————————————————————————————————————————

pub fn spawn_irc_client_worker(
    message_rx: impl crate::core::channel::DanmuReceiver + 'static + Send,
    state_rx: tokio::sync::watch::Receiver<crate::core::state::GlobalState>,
) {
    tokio::spawn(async move {
        let worker = crate::actors::irc_client::IrcClientWorker::new(state_rx, Box::new(message_rx));
        if let Err(e) = worker.run().await {
            log_error!("IRC:Cli: {}", AppError::crash("Worker", e.to_string()));
        }
    });
}

// ————————————————————————————————————————————————————————————
// DanmakuFormatter
// ————————————————————————————————————————————————————————————

#[cfg(feature = "channel-broadcast")]
pub fn spawn_danmaku_formatter(danmaku_rx: tokio::sync::broadcast::Receiver<crate::core::channel::DanmuMessage>) {
    let formatter = crate::utils::danmaku_formatter::DanmakuFormatter::new(Box::new(danmaku_rx));
    tokio::spawn(async move {
        if let Err(e) = formatter.run().await {
            log_warn!("Danmu:Fmt: Format error - {}", e);
        }
    });
}
