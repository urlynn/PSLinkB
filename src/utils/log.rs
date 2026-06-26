//! 统一日志
//!
//! log!(info, ...)     — 无色无标签
//! log!(ok, ...)       — 整行绿色输出
//! log!(warn, ...)     — [WARN] 橙色标签
//! log!(error, ...)    — [ERROR] 红色标签
//! log!(alert, ...)    — 整行粗体红色

/// 日志宏
#[macro_export]
macro_rules! log {
    (info, $($arg:tt)*) => {{
        $crate::log::_info(&format!($($arg)*));
    }};
    (ok, $($arg:tt)*) => {{
        $crate::log::_ok(&format!($($arg)*));
    }};
    (warn, $($arg:tt)*) => {{
        $crate::log::_warn(&format!($($arg)*));
    }};
    (error, $($arg:tt)*) => {{
        $crate::log::_error(&format!($($arg)*));
    }};
    (alert, $($arg:tt)*) => {{
        $crate::log::_alert(&format!($($arg)*));
    }};
}

/// 调试日志
#[macro_export]
macro_rules! dlog {
    ($($arg:tt)*) => {{
        if cfg!(any(debug_assertions, feature = "debug-log")) || $crate::log::is_debug_enabled() {
            $crate::log::_dbg(&format!($($arg)*));
        }
    }};
}

use owo_colors::{OwoColorize, Stream, Style};
use std::sync::atomic::{AtomicBool, Ordering};

static DEBUG_ENABLED: AtomicBool = AtomicBool::new(false);

/// 运行时调试日志
pub fn set_debug_enabled(enabled: bool) {
    DEBUG_ENABLED.store(enabled, Ordering::Relaxed);
}

pub fn is_debug_enabled() -> bool {
    DEBUG_ENABLED.load(Ordering::Relaxed)
}

#[doc(hidden)]
pub fn _info(msg: &str) {
    eprintln!("{}", msg);
}

#[doc(hidden)]
pub fn _ok(msg: &str) {
    eprintln!("{}", msg.if_supports_color(Stream::Stderr, |s| s.green()));
}

#[doc(hidden)]
pub fn _warn(msg: &str) {
    eprintln!("{} {}", "[WARN]".if_supports_color(Stream::Stderr, |s| s.yellow()), msg);
    #[cfg(feature = "openwrt")]
    crate::luci::set("error", msg);
}

#[doc(hidden)]
pub fn _error(msg: &str) {
    eprintln!("{} {}", "[ERROR]".if_supports_color(Stream::Stderr, |s| s.red()), msg);
    #[cfg(feature = "openwrt")]
    crate::luci::set("error", msg);
}

#[doc(hidden)]
pub fn _alert(msg: &str) {
    let style = Style::new().red().bold();
    eprintln!("{}", msg.if_supports_color(Stream::Stderr, |s| s.style(style)));
}

#[doc(hidden)]
pub fn _dbg(msg: &str) {
    eprintln!("[DEBUG] {}", msg);
}
