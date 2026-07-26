//! 主事件循环 — 信号处理 + 事件分发 + 关闭流程

use crate::core::event::Event;
use crate::core::error::AppError;
use crate::dispatch;
use crate::log;
use crate::system::{System, FfmpegCmd, BilibiliCmd, DanmakuCmd};
use tokio::sync::mpsc;

#[cfg(unix)]
#[cfg(feature = "openwrt")]
use tokio::signal::unix::{signal, SignalKind};

pub struct Channels {
    pub ffmpeg: mpsc::Sender<FfmpegCmd>,
    pub bilibili: mpsc::Sender<BilibiliCmd>,
    pub danmaku: mpsc::Sender<DanmakuCmd>,
    pub irc_notify: mpsc::Sender<String>,
}

async fn dispatch_effects(system: &mut System, event: Event, ch: &Channels) {
    for effect in system.handle(event) {
        dispatch::dispatch(effect, &ch.ffmpeg, &ch.bilibili, &ch.danmaku, &ch.irc_notify).await;
    }
}

async fn do_shutdown(system: &mut System, ch: &Channels) {
    #[cfg(feature = "openwrt")]
    crate::dns::redirect::cleanup(&system.config);
    #[cfg(feature = "openwrt")]
    crate::luci::clear("user");
    dispatch_effects(system, Event::Shutdown, ch).await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    #[cfg(feature = "openwrt")]
    crate::luci::set("running", false);
}

pub async fn run_loop(
    mut system: System,
    mut event_rx: mpsc::Receiver<Event>,
    ch: Channels,
    #[allow(unused_variables)] local_ip: &str,
    #[allow(unused_variables)] base_config: crate::config::Config,
    #[cfg(unix)] sigterm: &mut tokio::signal::unix::Signal,
) -> Result<(), AppError> {
    #[cfg(feature = "openwrt")]
    let mut sighup = signal(SignalKind::hangup()).expect("无法注册 SIGHUP");

    // SIGHUP 独立 task
    #[cfg(feature = "openwrt")]
    let (hup_tx, mut hup_rx) = tokio::sync::mpsc::unbounded_channel();
    #[cfg(feature = "openwrt")]
    tokio::spawn(async move {
        loop {
            sighup.recv().await;
            let _ = hup_tx.send(());
        }
    });

    // SIGUSR1：uci 改动 -> 热更新类
    #[cfg(feature = "openwrt")]
    {
        let mut sigusr1 = signal(SignalKind::user_defined1()).expect("无法注册 SIGUSR1");
        let bili_tx = ch.bilibili.clone();
        let mut base = base_config;
        tokio::spawn(async move {
            loop {
                sigusr1.recv().await;
                // 等 LuCI 写盘落地
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                let Ok(cfg) = crate::config::Config::from_uci() else { continue };
                if cfg.auth.cookies != base.auth.cookies
                    || cfg.live.room_id != base.live.room_id
                    || cfg.live.live_mode != base.live.live_mode
                {
                    log!(info, "[INFO] 配置变更，重启中...");
                    std::process::exit(0);
                }
                if cfg.live.title != base.live.title || cfg.live.area_v2 != base.live.area_v2 {
                    let title = if cfg.live.title.is_empty() { None } else { Some(cfg.live.title.clone()) };
                    let _ = bili_tx.send(BilibiliCmd::UpdateRoom {
                        room_id: cfg.live.room_id,
                        title,
                        area: Some(cfg.live.area_v2.clone()),
                    }).await;
                }
                base = cfg;
            }
        });
    }

    loop {
        #[cfg(unix)]
        tokio::select! {
            _ = async {
                #[cfg(feature = "openwrt")]
                hup_rx.recv().await;
                #[cfg(not(feature = "openwrt"))]
                std::future::pending::<()>().await;
            } => {
                #[cfg(feature = "openwrt")]
                {
                    crate::dns::redirect::handle_sighup(local_ip).await;
                    continue;
                }
            }
            event = event_rx.recv() => {
                match event {
                    Some(ev) => dispatch_effects(&mut system, ev, &ch).await,
                    None => {
                        log!(error, "System: Event channel closed, exiting");
                        break;
                    }
                }
            }
            _ = tokio::signal::ctrl_c() => {
                eprintln!("\n[INFO] Ctrl+C, Shutting down...");
                do_shutdown(&mut system, &ch).await;
                break;
            }
            _ = sigterm.recv() => {
                eprintln!("[INFO] SIGTERM, Shutting down...");
                do_shutdown(&mut system, &ch).await;
                break;
            }
        }

        #[cfg(windows)]
        tokio::select! {
            event = event_rx.recv() => {
                match event {
                    Some(ev) => dispatch_effects(&mut system, ev, &ch).await,
                    None => {
                        log!(error, "System: Event channel closed, exiting");
                        break;
                    }
                }
            }
            _ = tokio::signal::ctrl_c() => {
                eprintln!("\n[INFO] Ctrl+C, Shutting down...");
                do_shutdown(&mut system, &ch).await;
                break;
            }
        }
    }

    eprintln!("[INFO] Shutdown complete");
    Ok(())
}
