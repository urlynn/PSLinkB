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
use std::sync::OnceLock;

static DEBUG_ENABLED: AtomicBool = AtomicBool::new(false);

/// 运行时调试日志
pub fn set_debug_enabled(enabled: bool) {
    DEBUG_ENABLED.store(enabled, Ordering::Relaxed);
}

pub fn is_debug_enabled() -> bool {
    DEBUG_ENABLED.load(Ordering::Relaxed)
}

/// Windivert 专用
static LOG_OVERRIDE: OnceLock<fn(&str)> = OnceLock::new();

pub fn set_override(f: fn(&str)) {
    let _ = LOG_OVERRIDE.set(f);
}

#[doc(hidden)]
pub fn _info(msg: &str) {
    if let Some(f) = LOG_OVERRIDE.get() { f(msg); } else { eprintln!("{}", msg); }
}

#[doc(hidden)]
pub fn _ok(msg: &str) {
    if let Some(f) = LOG_OVERRIDE.get() {
        f(msg);
    } else {
        eprintln!("{}", msg.if_supports_color(Stream::Stderr, |s| s.green()));
    }
}

#[doc(hidden)]
pub fn _warn(msg: &str) {
    if let Some(f) = LOG_OVERRIDE.get() {
        f(&format!("[WARN] {}", msg));
    } else {
        eprintln!("{} {}", "[WARN]".if_supports_color(Stream::Stderr, |s| s.yellow()), msg);
    }
    #[cfg(feature = "openwrt")]
    crate::luci::set("error", msg);
}

#[doc(hidden)]
pub fn _error(msg: &str) {
    if let Some(f) = LOG_OVERRIDE.get() {
        f(&format!("[ERROR] {}", msg));
    } else {
        eprintln!("{} {}", "[ERROR]".if_supports_color(Stream::Stderr, |s| s.red()), msg);
    }
    #[cfg(feature = "openwrt")]
    crate::luci::set("error", msg);
}

#[doc(hidden)]
pub fn _alert(msg: &str) {
    if let Some(f) = LOG_OVERRIDE.get() {
        f(&format!("[ERROR] {}", msg));
    } else {
        let style = Style::new().red().bold();
        eprintln!("{}", msg.if_supports_color(Stream::Stderr, |s| s.style(style)));
    }
}

#[doc(hidden)]
pub fn _dbg(msg: &str) {
    if let Some(f) = LOG_OVERRIDE.get() {
        f(&format!("[DEBUG] {}", msg));
    } else {
        eprintln!("[DEBUG] {}", msg);
    }
}
