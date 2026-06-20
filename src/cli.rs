//! CLI 参数定义（桌面模式）
//!
//! 仅在非 openwrt 模式下编译。

#![cfg(feature = "cli")]

use clap::Parser;

/// PSLinkB - PS5 to Bilibili Live Streaming Bridge
#[derive(Parser, Debug)]
#[command(name = "pslinkb")]
#[command(about = "PS5 to Bilibili Live Streaming Bridge")]
pub struct Args {
    /// 配置文件
    #[arg(short = 'C', long)]
    pub config: Option<String>,

    /// Cookie 字符串
    #[arg(short = 'c', long)]
    pub cookie: Option<String>,

    /// 直播间 ID
    #[arg(short = 'r', long)]
    pub room_id: Option<u64>,

    /// 直播标题
    #[arg(short = 't', long)]
    pub title: Option<String>,

    /// 直播分区 ID（默认 "237" - 单机游戏 - 主机游戏）
    #[arg(short = 'a', long)]
    pub area: Option<String>,

    /// 运行模式: auto (Default) or manual
    #[arg(short = 'm', long)]
    pub mode: Option<String>,
}

pub fn default_config_path() -> std::path::PathBuf {
    #[cfg(target_os = "windows")]
    {
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("pslinkb.toml")
    }
    #[cfg(not(target_os = "windows"))]
    {
        let home = std::env::var("HOME").unwrap_or_else(|_| "~".to_string());
        std::path::PathBuf::from(home).join(".config").join("pslinkb.toml")
    }
}
