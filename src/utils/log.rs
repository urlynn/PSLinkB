//! 统一日志：log_warn! / log_error!
//!
//! 每个宏自动加 [WARN] / [ERROR] 前缀到 stderr。
//! 两个宏都写入 /tmp/pslinkb/error（OpenWrt 下）。
//! info 级别直接用 eprintln!。

/// 警告日志 — stderr [WARN] + LuCI error 文件
#[macro_export]
macro_rules! log_warn {
    ($($arg:tt)*) => {{
        let msg = format!($($arg)*);
        eprintln!("[WARN] {}", msg);
        #[cfg(feature = "openwrt")]
        crate::luci::set("error", &msg);
    }};
}

/// 错误日志 — stderr [ERROR] + LuCI error 文件
#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => {{
        let msg = format!($($arg)*);
        eprintln!("[ERROR] {}", msg);
        #[cfg(feature = "openwrt")]
        crate::luci::set("error", &msg);
    }};
}
