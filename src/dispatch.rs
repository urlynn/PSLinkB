//! 副作用调度器：Effect

use tokio::sync::mpsc;

use crate::core::effect::Effect;

/// 执行一个副作用（发送到对应 Worker channel）
/// IRC 客户端在 main 中永恒运行，不在此处分发生死。
pub async fn dispatch(
    effect: Effect,
    ffmpeg_tx: &mpsc::Sender<crate::system::FfmpegCmd>,
    bilibili_tx: &mpsc::Sender<crate::system::BilibiliCmd>,
    danmaku_cmd_tx: &mpsc::Sender<crate::system::DanmakuCmd>,
    irc_notify_tx: &mpsc::Sender<String>,
) {
    match effect {
        Effect::StartFfmpeg {
            ps5_app,
            ps5_stream_key,
            bilibili_rtmp_url,
            bilibili_stream_key,
        } => {
            let _ = ffmpeg_tx
                .send(crate::system::FfmpegCmd::Start {
                    ps5_app,
                    ps5_stream_key,
                    bilibili_rtmp_url,
                    bilibili_stream_key,
                })
                .await;
        }
        Effect::StopFfmpeg => {
            let _ = ffmpeg_tx.send(crate::system::FfmpegCmd::Stop).await;
        }
        Effect::BilibiliStartLive {
            room_id,
            area_v2,
            title,
        } => {
            let _ = bilibili_tx
                .send(crate::system::BilibiliCmd::StartLive {
                    room_id,
                    area_v2,
                    title,
                })
                .await;
        }
        Effect::BilibiliStopLive { room_id } => {
            let _ = bilibili_tx
                .send(crate::system::BilibiliCmd::StopLive { room_id })
                .await;
        }
        Effect::StartDanmaku { room_id } => {
            let _ = danmaku_cmd_tx
                .send(crate::system::DanmakuCmd::Start { room_id })
                .await;
        }
        Effect::StopDanmaku => {
            let _ = danmaku_cmd_tx
                .send(crate::system::DanmakuCmd::Stop)
                .await;
        }
        Effect::NotifyPs5(msg) => {
            let _ = irc_notify_tx.send(msg).await;
        }
        Effect::Log(msg) => {
            eprintln!("[System] {}", msg);
        }
    }
}
