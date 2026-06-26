//! 认证初始化：cookie 验证 - IPC 状态写入 - QR 登录调度 - Room ID 获取

use std::path::Path;

use crate::config::Config;
use crate::core::biliapi::{self, UserInfo};
use crate::core::error::AppError;
use crate::log;

/// 有效 cookie 返回 (cookie_string, csrf)，无效 exec 重启
pub async fn ensure_cookie(
    config_path: &Path,
    config: &Config,
) -> Result<(String, String), AppError> {
    let cookie = load_cookie_string(config_path, config);

    if !cookie.is_empty() {
        let csrf = config.auth.csrf();
        match verify_cookie_str(&cookie).await {
            Ok(Some(info)) => {
                if config.live.room_id == 0 {
                    discover_and_save_room(config_path, info.uid).await;
                }
                return Ok((cookie, csrf));
            }
            Ok(None) => eprintln!("[WARN] Cookie 已过期"),
            Err(_) => {
                eprintln!("[WARN] Cookie 验证失败 - 网络波动?");
                return Ok((cookie, csrf));
            }
        }
    } else {
        eprintln!("[INFO] 未找到 Cookie");
    }

    // ── QR 扫码登录 ──
    eprintln!("[INFO] 启动扫码登录...");
    let cookies = super::login::scan_qr_blocking(config_path, config).await?;

    save_cookies(config_path, &cookies)?;

    let cookie_str: String = cookies
        .iter()
        .map(|c| format!("{}={}", c.name, c.value))
        .collect::<Vec<_>>()
        .join("; ");
    if let Ok(Some(info)) = verify_cookie_str(&cookie_str).await
        && config.live.room_id == 0
    {
        discover_and_save_room(config_path, info.uid).await;
    }

    // ── exec 重启 ──
    eprintln!("[INFO] 登录完成，重启中...");

    #[cfg(all(feature = "cli", unix))]
    {
        use std::os::unix::process::CommandExt;
        let exe = std::env::current_exe()
            .unwrap_or_else(|_| Path::new("/proc/self/exe").to_path_buf());
        let args: Vec<_> = std::env::args().skip(1).collect();
        let err = std::process::Command::new(&exe).args(&args).exec();
        panic!("exec failed: {err}");
    }
    #[cfg(windows)]
    {
        let exe = std::env::current_exe()
            .unwrap_or_else(|_| Path::new("pslinkb.exe").to_path_buf());
        let args: Vec<_> = std::env::args().skip(1).collect();
        std::process::Command::new(&exe).args(&args).spawn().ok();
        std::process::exit(0);
    }
    #[cfg(feature = "openwrt")]
    {
        std::process::exit(0);
    }
}

// ── 验证 cookie ──

pub async fn verify_cookie_str(cookie_str: &str) -> Result<Option<UserInfo>, AppError> {
    let result = biliapi::get_user_info(cookie_str).await?;
    match &result {
        Some(info) => {
            use owo_colors::{OwoColorize, Stream, colors::css::HotPink};

            let check = "✓ 已登录".if_supports_color(Stream::Stderr, |s| s.green());
            let uname = info.uname.if_supports_color(Stream::Stderr, |s| s.fg::<HotPink>());
            eprintln!("[Auth] {} - {}:{}", check, uname, info.uid);
            crate::luci::set("user", &info.uname);
        }
        None => {
            crate::luci::set("user", "");
        }
    }
    Ok(result)
}

// ── 内部辅助 ──

#[cfg(feature = "cli")]
fn load_cookie_string(config_path: &Path, _config: &Config) -> String {
    Config::load_cookie_string(config_path).unwrap_or_default()
}

#[cfg(feature = "openwrt")]
fn load_cookie_string(_config_path: &Path, config: &Config) -> String {
    if config.auth.cookies.is_empty() {
        return String::new();
    }
    config.auth.cookies.iter()
        .map(|c| format!("{}={}", c.name, c.value))
        .collect::<Vec<_>>()
        .join("; ")
}

#[cfg(feature = "cli")]
fn save_cookies(config_path: &Path, cookies: &[crate::config::CookieEntry]) -> Result<(), AppError> {
    Config::save_auth_cookies(config_path, cookies)
}

#[cfg(feature = "openwrt")]
fn save_cookies(_config_path: &Path, cookies: &[crate::config::CookieEntry]) -> Result<(), AppError> {
    use std::process::Command;
    let cookie_str: String = cookies.iter()
        .map(|c| format!("{}={}", c.name, c.value))
        .collect::<Vec<_>>()
        .join("; ");
    if !Command::new("uci")
        .args(["set", &format!("pslinkb.@auth[0].cookie={}", cookie_str)])
        .output().map(|o| o.status.success()).unwrap_or(false)
    {
        return Err("uci set cookie failed".into());
    }
    Command::new("uci").args(["commit", "pslinkb"]).output()
        .map_err(|e| format!("uci commit: {}", e))?;
    Ok(())
}

async fn discover_and_save_room(
    #[cfg_attr(feature = "openwrt", allow(unused))] config_path: &Path,
    uid: i64,
) -> Option<u64> {
    match biliapi::get_room_id(uid).await {
        Ok(room_id) => {
            log!(ok, "[Auth] ✓ 从 API 获取 - 直播间 ID: {}", room_id);
            #[cfg(feature = "cli")]
            if let Err(e) = Config::save_room_id(config_path, room_id) {
                log!(warn, "保存房间号失败 - {}", e);
            }
            #[cfg(feature = "openwrt")]
            if let Err(e) = Config::save_room_id(room_id) {
                log!(warn, "保存房间号失败 - {}", e);
            }
            Some(room_id)
        }
        Err(e) => { log!(warn, "获取房间号失败 - {}", e); None }
    }
}
