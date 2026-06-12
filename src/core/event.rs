//! 事件枚举：外部世界 -> 状态机的输入

/// 系统事件
#[derive(Debug, Clone)]
pub enum Event {
    // ── RTMP 推流事件 ──
    /// PS5 开始推流
    RtmpPublish {
        app: String,
        stream_key: String,
    },
    /// PS5 停止推流
    RtmpUnpublish,

    /// 弹幕连接成功
    DanmakuReady,

    /// 开播成功，返回推流地址
    BilibiliLiveStarted {
        rtmp_url: String,
        stream_key: String,
    },
    /// 关播成功
    BilibiliLiveStopped,
    /// 开播失败
    BilibiliLiveStartFailed {
        code: i64,
        message: String,
    },
    /// 开播需要人脸验证
    BilibiliAuthRequired {
        face_auth_url: Option<String>,
    },
    /// 关播失败
    BilibiliLiveStopFailed {
        code: i64,
        message: String,
    },
    /// 直播流状态确认
    BilibiliStreamConfirmed {
        room_id: u64,
    },
    /// 直播流确认超时
    BilibiliStreamTimeout {
        room_id: u64,
    },

    /// PS5 IRC 频道就绪
    Ps5IrcReady { 
        channel: String 
    },

    /// FFmpeg 推流出错
    FfmpegError(String),

    // ── 系统事件 ──
    /// 关闭信号
    Shutdown,
}
