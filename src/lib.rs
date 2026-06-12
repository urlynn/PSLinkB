//! PSLinkB — PS5 to Bilibili Live Streaming Bridge

pub mod actors;
#[cfg(not(feature = "openwrt"))]
pub mod cli;
pub mod config;
pub mod core;
pub mod dispatch;
#[cfg(not(feature = "external-ffmpeg"))]
pub mod ffmpeg;
pub mod openwrt;
pub use openwrt::luci;
#[cfg(feature = "openwrt")]
pub use openwrt::ubus;
#[path = "utils/log.rs"]
pub mod log;
pub mod spawn;
pub mod system;
pub mod utils;
