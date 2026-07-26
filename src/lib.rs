//! PSLinkB — PS5 to Bilibili Live Streaming Bridge

pub mod actors;
pub mod auth;
#[cfg(feature = "cli")]
pub mod cli;
pub mod config;
pub mod core;
pub mod dispatch;
pub mod dns;
#[cfg(feature = "ffi-ffmpeg")]
pub mod ffmpeg;
pub mod openwrt;
pub use openwrt::luci;
pub mod run;
pub mod spawn;
pub mod system;
pub mod utils;

#[path = "utils/log.rs"]
pub mod log;

pub const INIT_COMPLETE_MSG: &str = "[INFO] ✓ 初始化完成 - PS5 按下直播键即可开播";
