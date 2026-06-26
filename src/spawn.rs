//! Worker 启动函数

use crate::core::error::AppError;
use crate::core::biliapi;
use crate::log;

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
            log!(error, "RTMP: {}", AppError::crash("Server", e.to_string()));
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
            log!(error, "IRC: {}", AppError::crash("Server", e.to_string()));
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
                            log!(error, "FFmpeg: {}", AppError::crash("Worker", e.to_string()));
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

pub fn spawn_bililive_cmds(
    mut cmd_rx: mpsc::Receiver<crate::system::BilibiliCmd>,
    event_tx: mpsc::Sender<Event>,
    cookie_string: String,
    csrf: String,
) {
    use std::sync::atomic::Ordering;
    // 人脸 watcher 取消标志
    let face_cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

    tokio::spawn(async move {
        while let Some(cmd) = cmd_rx.recv().await {
            let cookie_c = cookie_string.clone();
            let csrf_c = csrf.clone();
            let event_tx_c = event_tx.clone();
            let face_cancel_c = face_cancel.clone();

            tokio::spawn(async move {
                match cmd {
                    crate::system::BilibiliCmd::StartLive {
                        room_id,
                        area_v2,
                        title,
                    } => {
                        execute_start_live(room_id, area_v2, title, cookie_c, csrf_c, event_tx_c).await;
                    }
                    crate::system::BilibiliCmd::StopLive { room_id, client } => {
                        execute_stop_live(room_id, client, cookie_c, csrf_c, event_tx_c).await;
                    }
                    crate::system::BilibiliCmd::StartFaceWatch { room_id } => {
                        face_cancel_c.store(false, Ordering::Relaxed);
                        face_watcher(room_id, cookie_c, csrf_c, event_tx_c, face_cancel_c).await;
                    }
                    crate::system::BilibiliCmd::StopFaceWatch => {
                        face_cancel_c.store(true, Ordering::Relaxed);
                    }
                    crate::system::BilibiliCmd::SyncTwitchTitle { room_id, broadcaster_id } => {
                        sync_twitch_title(room_id, broadcaster_id, cookie_c, csrf_c, event_tx_c).await;
                    }
                    crate::system::BilibiliCmd::UpdateRoom { room_id, title, area } => {
                        update_room_info(room_id, title, area, cookie_c, csrf_c, event_tx_c).await;
                    }
                }
            });
        }
    });
}

/// 人脸验证 watcher: 1s 探测 -> Verified/10s fallback 发 TryStartLive; 3min 发 FaceTimeout。
async fn face_watcher(
    room_id: u64,
    cookie: String,
    csrf: String,
    event_tx: mpsc::Sender<Event>,
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
) {
    use std::sync::atomic::Ordering;
    let client = reqwest::Client::new();
    let start = tokio::time::Instant::now();
    let mut fallback = false;

    loop {
        if cancel.load(Ordering::Relaxed) { break; }
        if start.elapsed().as_secs_f64() > 180.0 {
            let _ = event_tx.send(Event::FaceTimeout).await;
            break;
        }
        if !fallback {
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            match crate::core::biliapi::check_face_status(&client, &cookie, &csrf, room_id).await {
                crate::core::biliapi::FaceStatus::Verified => {
                    let _ = event_tx.send(Event::TryStartLive).await;
                    fallback = true; // 验证已过
                }
                crate::core::biliapi::FaceStatus::NotYet => {}
                crate::core::biliapi::FaceStatus::Abnormal(payload) => {
                    log!(warn, "Bili:Live: 人脸状态检测异常 - {}", payload);
                    fallback = true;
                }
            }
        } else {
            tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
            let _ = event_tx.send(Event::TryStartLive).await;
        }
    }
}

pub async fn execute_start_live(
    room_id: u64,
    area_v2: String,
    title: Option<String>,
    cookie: String,
    csrf: String,
    event_tx: mpsc::Sender<Event>,
) {
    use crate::core::blive::{self, StartOutcome};

    match blive::try_start_live(&cookie, &csrf, room_id, &area_v2, title.as_deref()).await {
        StartOutcome::Started { rtmp_url, stream_key, client } => {
            let _ = event_tx.send(Event::BilibiliLiveStarted { rtmp_url, stream_key, client }).await;
            // 启动流状态监听
            let ev_tx2 = event_tx.clone();
            tokio::spawn(async move { monitor_stream_status(room_id, ev_tx2).await; });
        }
        StartOutcome::AuthRequired { face_auth_url } => {
            let _ = event_tx.send(Event::BilibiliAuthRequired { face_auth_url }).await;
        }
        StartOutcome::Failed { code, message } => {
            let _ = event_tx.send(Event::BilibiliLiveStartFailed { code, message }).await;
        }
    }
}

pub async fn execute_stop_live(
    room_id: u64,
    client: crate::core::biliapi::LiveClient,
    cookie: String,
    csrf: String,
    event_tx: mpsc::Sender<Event>,
) {
    use crate::core::blive;

    match blive::try_stop_live(&cookie, &csrf, room_id, client).await {
        Ok(()) => { let _ = event_tx.send(Event::BilibiliLiveStopped).await; }
        Err((code, message)) => { let _ = event_tx.send(Event::BilibiliLiveStopFailed { code, message }).await; }
    }
}

/// Twitch 标题同步 B站直播间
async fn sync_twitch_title(
    room_id: u64,
    broadcaster_id: String,
    cookie: String,
    csrf: String,
    event_tx: mpsc::Sender<Event>,
) {
    let Some(title) = crate::core::twitch::fetch_live_title(&broadcaster_id).await else { return };
    update_room_info(room_id, Some(title), None, cookie, csrf, event_tx).await;
}

/// 更新直播间标题
async fn update_room_info(
    room_id: u64,
    title: Option<String>,
    area: Option<String>,
    cookie: String,
    csrf: String,
    event_tx: mpsc::Sender<Event>,
) {
    match biliapi::update_room(&cookie, &csrf, room_id, title.as_deref(), area.as_deref()).await {
        Ok(Some(audit_title)) => {
            log!(info, "[Bili:Live] 直播间标题已更新为: {}", audit_title);
            let _ = event_tx.send(Event::TitleUpdated(audit_title)).await;
        }
        Ok(None) => {}
        Err(e) => log!(warn, "Bili:Live: {}", e),
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
                log!(ok, "[Bili:Live] Live stream confirmed - GetStreamInfo, {} - ✓ 直播视频流验证成功", attempt);
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
        log!(ok, "[Bili:Live] Live stream confirmed - PlayUrl - ✓ 直播视频流验证成功");
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
                                log!(error, "Danmaku: {}", AppError::crash("Worker", e.to_string()));
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
    message_rx: impl crate::core::channel::DanmuReceiver + 'static,
    state_rx: tokio::sync::watch::Receiver<crate::core::state::GlobalState>,
) {
    tokio::spawn(async move {
        let worker = crate::actors::irc_client::IrcClientWorker::new(state_rx, Box::new(message_rx));
        if let Err(e) = worker.run().await {
            log!(error, "IRC:Cli: {}", AppError::crash("Worker", e.to_string()));
        }
    });
}

// ————————————————————————————————————————————————————————————
// DanmakuFormatter
// ————————————————————————————————————————————————————————————

#[cfg(feature = "channel-broadcast")]
pub fn spawn_danmaku_formatter(danmaku_rx: tokio::sync::broadcast::Receiver<crate::core::channel::DanmuMessage>) {
    let formatter = crate::utils::danmulog::DanmakuFormatter::new(Box::new(danmaku_rx));
    tokio::spawn(async move {
        if let Err(e) = formatter.run().await {
            log!(warn, "Danmu:Fmt: Format error - {}", e);
        }
    });
}
