//! PSLinkB — 调度层：加载配置 -> 认证检查 -> 创建通道 -> 启动 workers -> 事件循环

use pslinkb::config::{RTMP_PORT, IRC_PORT};
use pslinkb::core::channel::create_danmu_channel;
use pslinkb::core::event::Event;
use pslinkb::core::auth::ensure_cookie;
use pslinkb::config::Config;
use pslinkb::core::state::GlobalState;
use pslinkb::system::{System, FfmpegCmd, BilibiliCmd, DanmakuCmd};
use pslinkb::core::error::AppError;
use pslinkb::{dispatch, luci, spawn, log_error};
#[cfg(not(feature = "openwrt"))]
use pslinkb::log_warn;

use tokio::sync::mpsc;

// ————————————————————————————————————————————————————————————
// 入口
// ————————————————————————————————————————————————————————————

#[tokio::main]
async fn main() -> Result<(), AppError> {
    // Ring crypto provider
    rustls::crypto::ring::default_provider().install_default()
        .expect("TLS provider init failed");

    // ── 加载配置 ──
    #[cfg(not(feature = "openwrt"))]
    let (config, config_path, cli_cookie) = load_config()?;
    #[cfg(feature = "openwrt")]
    let (config, config_path) = load_config()?;

    // ── CLI ──
    #[cfg(not(feature = "openwrt"))]
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

    // ── 认证（放行 or exec 重启）──
    let cookie_string = ensure_cookie(&config_path, &config).await?;

    // ── 重新加载配置 ──
    #[cfg(not(feature = "openwrt"))]
    let config = Config::from_file(&config_path)?;
    #[cfg(feature = "openwrt")]
    let config = Config::from_uci()?;

    eprintln!();
    eprintln!("╔══════════════════════════════════════╗");
    eprintln!("║  PSLinkB v{}                      ║", env!("CARGO_PKG_VERSION"));
    eprintln!("║  PS5 -> Bilibili Live Bridge         ║");
    eprintln!("╚══════════════════════════════════════╝");
    eprintln!();
    eprintln!("[INFO] RTMP: {} | IRC: {} | Room: {}",
        RTMP_PORT, IRC_PORT, config.room.room_id);
    eprintln!();

    luci::init();

    // ── ubus IPC（OpenWRT 模式）──
    #[cfg(feature = "ubus-ipc")]
    {
        eprintln!("[INFO] ubus IPC mode");
        std::thread::spawn(|| {
            if let Err(e) = ubus::serve() {
                log_error!("ubus: {}", AppError::crash("Server", e.to_string()));
            }
        });
    }

    // ── 创建通道 ──
    let (event_tx, mut event_rx) = mpsc::channel::<Event>(256);
    let (ffmpeg_tx, ffmpeg_rx) = mpsc::channel::<FfmpegCmd>(8);
    let (bilibili_tx, bilibili_rx) = mpsc::channel::<BilibiliCmd>(8);
    let (danmaku_cmd_tx, danmaku_cmd_rx) = mpsc::channel::<DanmakuCmd>(8);
    let (danmaku_tx, danmaku_rx) = create_danmu_channel(512);

    // ── 启动 24/7 服务 ──
    let (irc_state_tx, irc_state_rx) = tokio::sync::watch::channel(GlobalState::default());

    // ── 创建状态机 ──
    let mut system = System::new(config.clone(), irc_state_rx.clone());

    let (irc_notify_tx, irc_notify_rx) = mpsc::channel::<String>(64);
    spawn::spawn_rtmp_server(event_tx.clone());
    spawn::spawn_irc_server(irc_state_tx, irc_notify_rx, event_tx.clone());

    // IRC Client — 永恒 worker，跟随 channel_name 自动建连/断连
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
    spawn::spawn_ffmpeg_worker(ffmpeg_rx, event_tx.clone());
    spawn::spawn_bilibili_worker(bilibili_rx, event_tx.clone(), cookie_string.clone());
    spawn::spawn_danmaku_worker(danmaku_cmd_rx, danmaku_tx.clone(), cookie_string, event_tx.clone());

    // ── 主事件循环 ──
    loop {
        tokio::select! {
            event = event_rx.recv() => {
                match event {
                    Some(ev) => {
                        let effects = system.handle(ev);
                        for effect in effects {
                            dispatch::dispatch(
                                effect,
                                &ffmpeg_tx,
                                &bilibili_tx,
                                &danmaku_cmd_tx,
                                &irc_notify_tx,
                            ).await;
                        }
                    }
                    None => {
                        log_error!("System: Event channel closed, exiting");
                        break;
                    }
                }
            }
            _ = tokio::signal::ctrl_c() => {
                eprintln!("\n[INFO] Ctrl+C, Shutting down...");
                for effect in system.handle(Event::Shutdown) {
                    dispatch::dispatch(
                        effect,
                        &ffmpeg_tx,
                        &bilibili_tx,
                        &danmaku_cmd_tx,
                        &irc_notify_tx,
                    ).await;
                }
                break;
            }
        }
    }

    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    eprintln!("[INFO] Shutdown complete");
    std::process::exit(0);
}

// ————————————————————————————————————————————————————————————
// 配置加载
// ————————————————————————————————————————————————————————————

#[cfg(not(feature = "openwrt"))]
fn load_config() -> Result<(Config, std::path::PathBuf, Option<String>), AppError> {
    use clap::Parser;
    use std::path::PathBuf;

    let args = pslinkb::cli::Args::parse();
    let config_path = args.config
        .map(PathBuf::from)
        .unwrap_or_else(pslinkb::cli::default_config_path);

    let config = if config_path.exists() {
        eprintln!("[INFO] Loading config: {}", config_path.display());
        Config::from_file(&config_path)?
    } else {
        log_warn!("Config: Not found -  {}", config_path.display());
        let example = Config::default();
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        example.to_file(&config_path)?;
        eprintln!("[INFO] Created default config: {}", config_path.display());
        Config::default()
    };

    let mut config = config;
    config.apply_cli_overrides(
        args.room_id,
        args.title,
        args.area,
        args.mode.and_then(|s| s.parse().ok()),
    );

    Ok((config, config_path, args.cookie))
}

#[cfg(feature = "openwrt")]
fn load_config() -> Result<(Config, std::path::PathBuf), AppError> {
    eprintln!("[INFO] OpenWrt mode - loading /etc/config/pslinkb");
    let config = Config::from_uci()?;
    let path = std::path::PathBuf::from("/etc/pslinkb.toml");
    Ok((config, path))
}
