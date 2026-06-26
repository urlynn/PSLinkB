//! 副作用枚举：状态机决策后的输出

/// 状态机的输出副作用
#[derive(Debug, Clone)]
pub enum Effect {
    /// 启动 FFmpeg 推流
    StartFfmpeg {
        ps5_app: String,
        ps5_stream_key: String,
        bilibili_rtmp_url: String,
        bilibili_stream_key: String,
    },
    /// 停止 FFmpeg 推流
    StopFfmpeg,

    /// 调用 B站 startLive
    BilibiliStartLive {
        room_id: u64,
        area_v2: String,
        title: Option<String>,
    },
    /// 调用 B站 stopLive
    BilibiliStopLive {
        room_id: u64,
        client: crate::core::biliapi::LiveClient,
    },
    /// 启动人脸验证轮询 watcher
    StartFaceWatch {
        room_id: u64,
    },
    /// 停止人脸验证 watcher
    StopFaceWatch,
    /// 直播间标题同步
    SyncTwitchTitle {
        room_id: u64,
        broadcaster_id: String,
    },

    /// 启动弹幕库 
    StartDanmaku {
        room_id: u64,
    },
    /// 停止弹幕库
    StopDanmaku,

    /// 通知 PS5
    NotifyPs5(String),

    /// 日志（终端输出）
    Log(String),

    /// 通知后重启 - Cookie 失效时
    Restart,
}
