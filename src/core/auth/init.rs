/// 认证初始化：cookie 验证 - IPC 状态写入 - QR 登录调度 - Room ID 获取

use crate::config::Config;
use crate::core::biliapi;
use crate::core::error::AppError;
use crate::log_warn;

// ── Room ID 获取 ──

async fn discover_and_save_room(
    #[cfg_attr(feature = "openwrt", allow(unused))] config_path: &std::path::Path,
    uid: i64,
) -> Option<u64> {
    match biliapi::get_room_id(uid).await {
        Ok(room_id) => {
            eprintln!("[Auth] 从 API 获取 - 直播间 ID: {}", room_id);
            #[cfg(not(feature = "openwrt"))]
            if let Err(e) = Config::save_room_id(config_path, room_id) {
                log_warn!("保存房间号失败 - {}", e);
            }
            #[cfg(feature = "openwrt")]
            if let Err(e) = Config::save_room_id(room_id) {
                log_warn!("保存房间号失败 - {}", e);
            }
            Some(room_id)
        }
        Err(e) => {
            log_warn!("获取房间号失败 - {}", e);
            None
        }
    }
}

// ── 共用：后台 QR 扫码登录 ──

pub fn spawn_qr_login_background(config_path: &std::path::Path, cm: std::sync::Arc<std::sync::Mutex<crate::core::auth::CookieManager>>) {
    let p = config_path.to_path_buf();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("qr runtime");
        rt.block_on(async move {
            loop {
                match crate::core::auth::scan_login(&mut *cm.lock().unwrap()).await {
                    Ok(user_info) => {
                        discover_and_save_room(&p, user_info.uid).await;
                        break;
                    }
                    Err(_) => {
                        log_warn!("二维码登录失败，重试中");
                        cm.lock().unwrap().clear_cache();
                        tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
                    }
                }
            }
        });
    });
}

// ── 认证检查 ──

#[cfg(not(feature = "openwrt"))]
pub async fn auth_check(
    config: &mut Config,
    config_path: &std::path::Path,
    cli_cookie: Option<String>,
) -> Result<(), AppError> {
    use crate::actors::blive::LiveMode;
    use crate::core::auth::CookieManager;

    if config.room.live_mode != LiveMode::Auto {
        eprintln!("[INFO] 手动模式 - 跳过认证");
        return Ok(());
    }

    let mut cm = CookieManager::new(Some(config_path.to_path_buf()));
    if let Some(c) = cli_cookie {
        cm.set_cookie(c);
    }

    if cm.has_cookie() {
        match cm.verify_cookie().await {
            Ok(Some(info)) => {
                if config.room.room_id == 0 {
                    if let Some(room_id) = discover_and_save_room(config_path, info.uid).await {
                        config.room.room_id = room_id;
                    }
                }
            }
            Ok(None) => {
                eprintln!("[WARN] Cookie 已过期，启动扫码登录...");
                cm.clear_cache();
                spawn_qr_login_background(config_path, std::sync::Arc::new(std::sync::Mutex::new(cm)));
            }
            Err(e) => return Err(e),
        }
    } else {
        eprintln!("[WARN] 无 Cookie，启动扫码登录...");
        spawn_qr_login_background(config_path, std::sync::Arc::new(std::sync::Mutex::new(cm)));
    }

    Ok(())
}

#[cfg(feature = "openwrt")]
pub async fn auth_check(
    config: &Config,
    _config_path: &std::path::Path,
) -> Result<(), AppError> {
    use crate::actors::blive::LiveMode;
    if config.room.live_mode != LiveMode::Auto {
        return Ok(());
    }
    Ok(())
}

// ── OpenWRT 认证初始化 ──

#[cfg(feature = "openwrt")]
pub async fn auth_init(config_path: &std::path::Path, cm: std::sync::Arc<std::sync::Mutex<crate::core::auth::CookieManager>>, config: &Config) {
    let cookie = cm.lock().unwrap().get_cookie_string().ok().filter(|c| !c.is_empty());
    if let Some(cookie) = cookie {
        match crate::core::auth::verify_cookie_str(&cookie).await {
            Ok(Some(info)) => {
                if config.room.room_id == 0 {
                    discover_and_save_room(config_path, info.uid).await;
                }
            }
            Ok(None) => {
                spawn_qr_login_background(config_path, cm.clone());
            }
            Err(_) => {} // 网络波动：保留旧 user，不丢失登录态
        }
    }
}
