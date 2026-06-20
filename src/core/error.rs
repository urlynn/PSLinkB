//! ## 错误类型
//! - `BiliAPI` — B站 API 返回错误（带操作名和 code），用于 log! 宏
//! - `Crash` — 服务崩溃（actor 返回 Err 时使用）
//! - `General` — 其他所有错误（网络、FFmpeg、IO 等，保留原始消息）
//! - `FfmpegExitStatus` — FFmpeg 退出状态分类

/// 统一错误枚举
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    /// B站 API 错误 — Display: "BiliAPI (StartLive, code=4): 没有权限"
    #[error("BiliAPI ({operation}, {code}) - {message}")]
    BiliAPI {
        operation: &'static str,
        code: i64,
        message: String,
    },

    /// 服务崩溃
    #[error("{0} crashed - {1}")]
    Crash(&'static str, String),

    /// 通用错误
    #[error("{0}")]
    General(String),
}

// ── 构造方法 ──

impl AppError {
    pub fn bili_api(
        operation: &'static str,
        code: i64,
        message: impl Into<String>,
    ) -> Self {
        AppError::BiliAPI { operation, code, message: message.into() }
    }

    pub fn crash(role: &'static str, detail: impl Into<String>) -> Self {
        AppError::Crash(role, detail.into())
    }
}

// ── 自动转换（用于 ? 运算符） ──

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        match err.kind() {
            std::io::ErrorKind::NotFound => AppError::General(format!("file not found: {}", err)),
            std::io::ErrorKind::PermissionDenied => AppError::General(format!("permission denied: {}", err)),
            std::io::ErrorKind::TimedOut => AppError::General(format!("IO timeout: {}", err)),
            std::io::ErrorKind::ConnectionRefused
            | std::io::ErrorKind::ConnectionReset
            | std::io::ErrorKind::ConnectionAborted
            | std::io::ErrorKind::NotConnected => AppError::General(format!("connection failed: {}", err)),
            _ => AppError::General(format!("IO error: {}", err)),
        }
    }
}

impl From<reqwest::Error> for AppError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            AppError::General(format!("HTTP timeout: {}", err))
        } else if err.is_connect() {
            AppError::General(format!("HTTP connect failed: {}", err))
        } else {
            AppError::General(format!("HTTP error: {}", err))
        }
    }
}

impl From<serde_json::Error> for AppError {
    fn from(err: serde_json::Error) -> Self {
        AppError::General(format!("JSON parse error: {}", err))
    }
}

#[cfg(feature = "cli")]
impl From<toml::de::Error> for AppError {
    fn from(err: toml::de::Error) -> Self {
        AppError::General(format!("TOML parse error: {}", err))
    }
}

#[cfg(feature = "cli")]
impl From<toml::ser::Error> for AppError {
    fn from(err: toml::ser::Error) -> Self {
        AppError::General(format!("TOML serialize error: {}", err))
    }
}

impl From<std::net::AddrParseError> for AppError {
    fn from(err: std::net::AddrParseError) -> Self {
        AppError::General(format!("address parse error: {}", err))
    }
}

impl From<std::ffi::NulError> for AppError {
    fn from(err: std::ffi::NulError) -> Self {
        AppError::General(format!("CString error: {}", err))
    }
}

impl From<blivemsg::Error> for AppError {
    fn from(err: blivemsg::Error) -> Self {
        AppError::General(format!("blivemsg: {}", err))
    }
}

impl From<rtmp_rs::Error> for AppError {
    fn from(err: rtmp_rs::Error) -> Self {
        AppError::General(format!("rtmp: {}", err))
    }
}

impl From<String> for AppError {
    fn from(s: String) -> Self { AppError::General(s) }
}

impl From<&str> for AppError {
    fn from(s: &str) -> Self { AppError::General(s.to_string()) }
}

impl From<Box<dyn std::error::Error + Send + Sync>> for AppError {
    fn from(err: Box<dyn std::error::Error + Send + Sync>) -> Self {
        AppError::General(err.to_string())
    }
}

// ── 业务工具 ──

/// B站 API 通用错误码
pub fn bili_common_error(code: i64) -> Option<&'static str> {
    match code {
        -400  => Some("请求参数错误"),
        -101  => Some("未登录"),
        -3    => Some("API校验密匙错误"),
        3     => Some("鉴权失败"),
        4     => Some("没有权限"),
        65530 => Some("登录已失效"),
        _     => None,
    }
}

/// StartLive 专用
pub fn start_live_error(code: i64, raw: &str) -> String {
    match code {
        60009 => "分区不存在".into(),
        60013 => "所在地区受实名认证限制无法开播".into(),
        60024 => "目标分区需要人脸认证".into(),
        60034 => "系统维护仅支持直播姬开播".into(),
        60037 => "在线开播已下线".into(),
        60043 => "本次开播需要身份验证".into(),
        _ => bili_common_error(code).unwrap_or(raw).to_string(),
    }
}

// ── FFmpeg 退出状态 ──

#[derive(Debug, Clone)]
pub enum FfmpegExitStatus {
    /// 正常结束（EOF / stop by user）
    Normal,
    /// system 层处理
    Error(FfmpegErrorKind),
}

#[derive(Debug, Clone)]
pub enum FfmpegErrorKind {
    /// 进程被信号杀死
    Crash(String),
    /// 连接中断 / 读写出错
    IoError(String),
}

impl FfmpegExitStatus {
    pub fn message(&self) -> String {
        match self {
            FfmpegExitStatus::Normal => unreachable!(),
            FfmpegExitStatus::Error(FfmpegErrorKind::Crash(s)) => format!("Crash: {}", s),
            FfmpegExitStatus::Error(FfmpegErrorKind::IoError(s)) => format!("I/O: {}", s),
        }
    }
}
