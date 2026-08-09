//! CLI 参数定义（桌面模式）

#![cfg(feature = "cli")]

use clap::Parser;

/// PSLinkB - PS5 to Bilibili Live Streaming Bridge
#[derive(Parser, Debug)]
#[command(name = "pslinkb")]
#[command(about = "PS5 to Bilibili Live Streaming Bridge")]
#[command(version)]
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

    /// 直播分区 ID（默认 单机游戏 - 主机游戏）
    #[arg(short = 'a', long)]
    pub area: Option<String>,

    /// 运行模式: auto (Default) or manual
    #[arg(short = 'm', long = "live-mode", alias = "mode")]
    pub live_mode: Option<String>,

    /// 使用系统 FFmpeg (路径)
    #[arg(long)]
    pub ffmpeg: Option<String>,

    /// HTTP 代理模式 (格式 http://host:port)
    #[arg(long)]
    pub proxy: Option<String>,

    /// 加速器兼容模式
    #[arg(long)]
    pub windivert: bool,

    /// DNS 53 劫持模式
    #[arg(long)]
    pub dns: bool,

    /// 开启调试日志输出
    #[arg(long)]
    pub debug: bool,
}

/// TOML 启动时附加参数
#[derive(Debug, Clone, Default)]
pub struct LaunchArgs {
    pub proxy: Option<String>,
    pub windivert: bool,
    pub dns: bool,
}

pub fn parse_args_string(s: &str) -> LaunchArgs {
    let mut args = LaunchArgs::default();
    for tok in s.split_whitespace() {
        if let Some(v) = tok.strip_prefix("--proxy=") {
            args.proxy = Some(v.to_string());
        } else if tok == "--windivert" {
            args.windivert = true;
        } else if tok == "--dns" {
            args.dns = true;
        }
    }
    args
}

impl LaunchArgs {
    pub fn merge_cli(&mut self, proxy: Option<String>, windivert: bool, dns: bool) {
        if proxy.is_some() { self.proxy = proxy; }
        if windivert { self.windivert = true; }
        if dns { self.dns = true; }
    }
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
    #[cfg(unix)]
    {
        let home = std::env::var("HOME").unwrap_or_else(|_| "~".to_string());
        std::path::PathBuf::from(home).join(".config").join("pslinkb.toml")
    }
}

/// 交互式运行模式选择
pub fn select_mode() -> Option<String> {
    use std::io::{self, Write};

    loop {
        eprintln!("未设置运行参数 - 请选择运行模式:");
        eprintln!("    1. 默认模式             我会自己处理 DNS 重定向");
        eprintln!("    2. DNS 劫持模式         内置 DNS 代理 - 需要 PS5 的 DNS 指向本机");
        eprintln!("    3. HTTP 代理模式        PS5 无需设置代理 - 仅需将 DNS 指向本机");
        #[cfg(windows)]
        eprintln!("    4. 加速器兼容模式       仅在使用 PC 端加速器的情况下选择此项");
        #[cfg(windows)]
        eprint!("  输入 1-4: ");
        #[cfg(unix)]
        eprint!("  输入 1-3: ");
        io::stdout().flush().ok();

        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_err() {
            return None;
        }
        let choice = input.trim();

        let args = match choice {
            "1" => Some("--default".to_string()),
            "2" => Some("--dns".to_string()),
            "3" => {
                eprintln!("HTTP 代理格式为 http://host:port - 如 http://127.0.0.1:7890");
                let mut url = String::new();
                loop {
                    eprint!("请输入 HTTP 代理地址: ");
                    io::stdout().flush().ok();
                    url.clear();
                    if io::stdin().read_line(&mut url).is_err() {
                        return None;
                    }
                    let url = url.trim();
                    if url.starts_with("http://") && url[7..].contains(':') {
                        return Some(format!("--proxy={}", url));
                    }
                    eprintln!("参数无效 - 请重新输入:");
                }
            }
            #[cfg(windows)]
            "4" => Some("--windivert".to_string()),
            _ => {
                eprintln!("参数无效 - 请重新输入:");
                continue;
            }
        };

        return args;
    }
}

pub fn confirm_save() -> bool {
    use std::io::{self, Write};

    loop {
        eprint!("是否将当前模式保存到 pslinkb.toml? [Y/n]: ");
        io::stdout().flush().ok();

        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_err() {
            return false;
        }
        let ans = input.trim().to_lowercase();
        match ans.as_str() {
            "" | "y" | "yes" => return true,
            "n" | "no" => return false,
            _ => eprintln!("参数无效 - 请重新输入:"),
        }
    }
}
