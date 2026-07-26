//! PSLinkB — 调度层：加载配置 -> 认证检查 -> 创建通道 -> 启动 workers -> 事件循环

use pslinkb::config::{RTMP_PORT, IRC_PORT};
use pslinkb::core::channel::create_danmu_channel;
use pslinkb::core::event::Event;
use pslinkb::auth::ensure_cookie;
use pslinkb::config::Config;
use pslinkb::core::state::GlobalState;
use pslinkb::system::{System, FfmpegCmd, BilibiliCmd, DanmakuCmd};
use pslinkb::core::error::AppError;
use pslinkb::run::{Channels, run_loop};
use pslinkb::{luci, spawn, log};
#[cfg(unix)]
use tokio::signal::unix::{signal, SignalKind};

use tokio::sync::mpsc;

// ————————————————————————————————————————————————————————————
// 入口
// ————————————————————————————————————————————————————————————

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("[ERROR] {}", e);
        #[cfg(windows)]
        let _ = std::process::Command::new("cmd").args(["/C", "pause"]).status();
        luci::set("running", false);
        std::process::exit(1);
    }
}

async fn run() -> Result<(), AppError> {
    // --version - openwrt 专用 
    #[cfg(feature = "openwrt")]
    if std::env::args().any(|a| a == "--version" || a == "-v") {
        println!("{}", env!("CARGO_PKG_VERSION"));
        std::process::exit(0);
    }

    // ── 解析 CLI 参数 ──
    #[cfg(feature = "cli")]
    let args = {
        use clap::Parser;
        pslinkb::cli::Args::parse()
    };

    // ── 运行时调试日志开关 ──
    #[cfg(feature = "cli")]
    if args.debug {
        pslinkb::log::set_debug_enabled(true);
    }

    // Ring crypto provider
    rustls::crypto::ring::default_provider().install_default()
        .expect("TLS provider init failed");

    // ── OpenWRT 关色 ──
    #[cfg(feature = "openwrt")]
    owo_colors::set_override(false);

    // ── 加载配置 ──
    #[cfg(feature = "cli")]
    let (config, config_path, cli_cookie, created) = load_config(&args)?;
    #[cfg(feature = "openwrt")]
    let (config, config_path) = load_config()?;

    // ── CLI ──
    #[cfg(feature = "cli")]
    if let Some(ref cookie) = cli_cookie {
        use pslinkb::config::CookieEntry;
        let entries: Vec<CookieEntry> = cookie
            .split(';')
            .filter_map(|pair| {
                let mut kv = pair.trim().splitn(2, '=');
                let name = kv.next()?.trim();
                let value = kv.next()?.trim();
                if name.is_empty() { return None; }
                Some(CookieEntry { name: name.to_string(), value: value.to_string() })
            })
            .collect();
        if !entries.is_empty() {
            Config::save_auth_cookies(&config_path, &entries)?;
        }
    }

    // ── IPC 目录 + 清理 ──
    luci::init();

    #[cfg(feature = "openwrt")]
    luci::set("running", true);

    // ── 认证（放行 or exec 重启）──
    let (cookie_string, csrf) = ensure_cookie(&config_path, &config).await?;

    // ── DNS 重定向检测 ──
    let local_ip = pslinkb::utils::ip::local_ip();

    #[cfg(feature = "dns-redirect")]
    let deferred;
    #[cfg(feature = "dns-redirect")]
    {
        let dns_override = parse_dns_override(
            #[cfg(feature = "cli")]
            args.dns.as_deref(),
        );
        deferred = pslinkb::dns::auto_start(
            config.dns_proxy,
            &local_ip,
            dns_override,
        ).await;
    }

    #[cfg(unix)]
    let mut sigterm = signal(SignalKind::terminate()).expect("无法注册 SIGTERM");

    #[cfg(feature = "openwrt")]
    {
        pslinkb::dns::redirect::init(pslinkb::dns::REDIRECT_DOMAINS, &local_ip, &config).await;
    }

    // ── 重新加载配置 ──
    #[cfg(feature = "cli")]
    let mut config = Config::from_file(&config_path)?;
    #[cfg(feature = "openwrt")]
    let mut config = Config::from_uci()?;

    #[cfg(feature = "cli")]
    config.apply_cli_overrides(
        args.room_id,
        args.title.clone(),
        args.area.clone(),
        args.mode.as_ref().and_then(|s| s.parse().ok()),
        args.ffmpeg.clone(),
    );

    #[cfg(feature = "cli")]
    if args.area.is_none() {
        use pslinkb::core::biliapi;
        match biliapi::get_area_list().await {
            Ok(list) => {
                if let Some(resolved) = biliapi::resolve_area_id(&list, &config.live.area_name) {
                    let resolved_str = resolved.to_string();
                    if created {
                        if let Err(_) = Config::save_area_v2(&config_path, &resolved_str) {
                            log!(warn, "分区设置失败 - 请在 pslinkb.toml 设置分区 ID");
                        } else {
                            config.live.area_v2 = resolved_str.clone();
                        }
                    } else if config.live.area_v2 != resolved_str {
                        log!(warn, "分区 {} 的 ID 已变为 {} - 如需修改 请在 pslinkb.toml 设置 area_v2 = \"{}\"",
                            config.live.area_name, resolved_str, resolved_str);
                    }
                }
            }
            Err(_) => {
                log!(warn, "分区设置失败 - 请在 pslinkb.toml 设置分区 ID");
            }
        }
    }

    #[cfg(feature = "openwrt")]
    {
        use pslinkb::core::biliapi;
        if let Ok(list) = biliapi::get_area_list().await {
            if let Some(resolved) = biliapi::resolve_area_id(&list, &config.live.area_name) {
                let resolved_str = resolved.to_string();
                if Config::save_area_v2(&resolved_str).is_ok() {
                    config.live.area_v2 = resolved_str;
                }
            }
        }
    }

    #[cfg(feature = "cli")]
    {
        eprintln!();
        eprintln!("╔══════════════════════════════════════╗");
        eprintln!("║  PSLinkB v{}                      ║", env!("CARGO_PKG_VERSION"));
        eprintln!("║  PS5 -> Bilibili Live Bridge         ║");
        eprintln!("╚══════════════════════════════════════╝");
        eprintln!();
    }
    #[cfg(feature = "openwrt")]
    eprintln!("[INFO] PSLinkB v{}", env!("CARGO_PKG_VERSION"));
    eprintln!("[INFO] RTMP: {} | IRC: {} | Room: {}",
        RTMP_PORT, IRC_PORT, config.live.room_id);
    eprintln!();

    // ── 创建通道 ──
    let (event_tx, event_rx) = mpsc::channel::<Event>(256);
    let (ffmpeg_tx, ffmpeg_rx) = mpsc::channel::<FfmpegCmd>(8);
    let (bilibili_tx, bilibili_rx) = mpsc::channel::<BilibiliCmd>(8);
    let (danmaku_cmd_tx, danmaku_cmd_rx) = mpsc::channel::<DanmakuCmd>(8);
    let (danmaku_tx, danmaku_rx) = create_danmu_channel(512);

    // ── 启动 24/7 服务 ──
    let (irc_state_tx, irc_state_rx) = tokio::sync::watch::channel(GlobalState::default());

    // ── 创建状态机 ──
    let system = System::new(config.clone(), irc_state_rx.clone());

    let (irc_notify_tx, irc_notify_rx) = mpsc::channel::<String>(64);
    spawn::spawn_rtmp_server(event_tx.clone());
    let irc_ready = spawn::spawn_irc_server(irc_state_tx, irc_notify_rx, event_tx.clone());

    // IRC Client
    #[cfg(feature = "channel-mpsc")]
    spawn::spawn_irc_client_worker(danmaku_rx, irc_state_rx.clone());

    #[cfg(feature = "channel-broadcast")]
    {
        let rx = danmaku_rx.resubscribe();
        let fmt_rx = danmaku_rx.resubscribe();
        drop(danmaku_rx);
        spawn::spawn_irc_client_worker(rx, irc_state_rx.clone());
        spawn::spawn_danmaku_formatter(fmt_rx);
    }

    // ── 启动按需 workers ──
    #[cfg(feature = "cli")]
    let ffmpeg_path = config.ffmpeg.clone();
    #[cfg(not(feature = "cli"))]
    let ffmpeg_path = None;
    spawn::spawn_ffmpeg_worker(ffmpeg_rx, event_tx.clone(), ffmpeg_path);
    spawn::spawn_bililive_cmds(bilibili_rx, event_tx.clone(), cookie_string.clone(), csrf.clone());
    spawn::spawn_danmaku_worker(danmaku_cmd_rx, danmaku_tx.clone(), cookie_string, event_tx.clone());

    let _ = irc_ready.await;

    // ── 初始化完成 ──
    #[cfg(feature = "dns-redirect")]
    if !deferred {
        log!(ok, "{}", pslinkb::INIT_COMPLETE_MSG);
    }
    #[cfg(feature = "openwrt")]
    log!(ok, "{}", pslinkb::INIT_COMPLETE_MSG);


    #[cfg(all(feature = "cli", windows))]
    {
        log!(alert, "[WARN] 请将 PS5 的首选 DNS 设为本机 IP: {} - 备用 DNS 为 0.0.0.0", local_ip);
        log!(alert, "[WARN] 若设置完成后此处未打印 PS5 连接日志 - 请在 https://urlynn.xyz 根据 #常见问题 自主排查");
    }

    // ── 主事件循环 ──
    let ch = Channels { ffmpeg: ffmpeg_tx, bilibili: bilibili_tx, danmaku: danmaku_cmd_tx, irc_notify: irc_notify_tx };
    #[cfg(unix)]
    let result = run_loop(system, event_rx, ch, &local_ip, config, &mut sigterm).await;
    #[cfg(not(unix))]
    let result = run_loop(system, event_rx, ch, &local_ip, config).await;


    result
}

// ————————————————————————————————————————————————————————————
// 配置加载
// ————————————————————————————————————————————————————————————

#[cfg(feature = "cli")]
fn load_config(args: &pslinkb::cli::Args) -> Result<(Config, std::path::PathBuf, Option<String>, bool), AppError> {
    use std::path::PathBuf;

    let config_path = args.config.clone()
        .map(PathBuf::from)
        .unwrap_or_else(pslinkb::cli::default_config_path);

    let (config, created) = if config_path.exists() {
        eprintln!("[INFO] Loading config: {}", config_path.display());
        (Config::from_file(&config_path)?, false)
    } else {
        log!(warn, "Config: Not found -  {}", config_path.display());
        let example = Config::default();
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        example.to_file(&config_path)?;
        eprintln!("[INFO] Created default config: {}", config_path.display());
        (Config::default(), true)
    };

    let mut config = config;
    config.apply_cli_overrides(
        args.room_id,
        args.title.clone(),
        args.area.clone(),
        args.mode.as_ref().and_then(|s| s.parse().ok()),
        args.ffmpeg.clone(),
    );

    Ok((config, config_path, args.cookie.clone(), created))
}

#[cfg(feature = "openwrt")]
fn load_config() -> Result<(Config, std::path::PathBuf), AppError> {
    eprintln!("[INFO] OpenWrt mode - loading /etc/config/pslinkb");
    let config = Config::from_uci()?;
    let path = std::path::PathBuf::new();
    Ok((config, path))
}

// ────────────────────────────────────────────────────────────
// DNS override 解析
// ────────────────────────────────────────────────────────────

#[cfg(feature = "dns-redirect")]
fn parse_dns_override(dns: Option<&str>) -> Option<std::net::SocketAddr> {
    use std::net::{Ipv4Addr, SocketAddr, IpAddr};
    let input = dns?;
    if let Some((ip_part, port_part)) = input.split_once(':') {
        let ip: Ipv4Addr = ip_part.parse().ok()?;
        let port: u16 = port_part.parse().ok()?;
        Some(SocketAddr::new(IpAddr::V4(ip), port))
    } else {
        let ip: Ipv4Addr = input.parse().ok()?;
        Some(SocketAddr::new(IpAddr::V4(ip), 53))
    }
}
