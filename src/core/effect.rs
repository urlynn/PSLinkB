//! 副作用枚举：状态机决策后的输出

/// 状态机的输出副作用
#[derive(Debug, Clone)]
pub enum Effect {
    // ── FFmpeg 控制 ──
    /// 启动 FFmpeg 推流
    StartFfmpeg {
        ps5_app: String,
        ps5_stream_key: String,
        bilibili_rtmp_url: String,
        bilibili_stream_key: String,
    },
    /// 停止 FFmpeg 推流
    StopFfmpeg,

    // ── B站 API 控制 ──
    /// 调用 B站 startLive API
    BilibiliStartLive {
        room_id: u64,
        area_v2: String,
        title: Option<String>,
    },
    /// 调用 B站 stopLive API
    BilibiliStopLive {
        room_id: u64,
    },

    // ── 弹幕服务控制 ──
    /// 启动弹幕 WebSocket 连接
    StartDanmaku {
        room_id: u64,
    },
    /// 停止弹幕连接
    StopDanmaku,

    // ── IRC 控制 ──
    /// 发送通知到 PS5 屏幕
    NotifyPs5(String),

    // ── 日志输出 ──
    /// 日志（终端输出）
    Log(String),

    // ── 系统控制 ──
    /// 通知后重启 - Cookie 失效时
    Restart,
}
