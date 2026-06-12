/// Cookie 管理模块

use std::path::PathBuf;

use crate::core::error::AppError;

pub struct CookieManager {
    #[cfg_attr(feature = "openwrt", allow(dead_code))]
    config_path: PathBuf,
    cached_cookie_string: Option<String>,
}

impl CookieManager {
    pub fn new(config_path: Option<PathBuf>) -> Self {
        let config_path = config_path.unwrap_or_else(|| Self::get_default_config_path());
        Self { config_path, cached_cookie_string: None }
    }

    fn get_default_config_path() -> PathBuf {
        #[cfg(target_os = "windows")]
        {
            std::env::current_exe().ok().and_then(|p| p.parent().map(|p| p.to_path_buf()))
                .unwrap_or_else(|| PathBuf::from(".")).join("pslinkb.toml")
        }
        #[cfg(not(target_os = "windows"))]
        {
            let home = std::env::var("HOME").unwrap_or_else(|_| "~".to_string());
            PathBuf::from(home).join(".config").join("pslinkb.toml")
        }
    }

    pub fn get_cookie_string(&mut self) -> Result<String, AppError> {
        if let Some(ref cookie) = self.cached_cookie_string { return Ok(cookie.clone()); }

        // 桌面模式: TOML
        #[cfg(not(feature = "openwrt"))]
        {
            let cookie = crate::config::Config::load_cookie_string(&self.config_path)?;
            self.cached_cookie_string = Some(cookie.clone());
            return Ok(cookie);
        }

        #[cfg(feature = "openwrt")]
        {
            if let Ok(content) = std::fs::read_to_string("/etc/config/pslinkb") {
                for line in content.lines() {
                    let line = line.trim();
                    if line.starts_with("option cookie") {
                        if let Some(val) = line.strip_prefix("option cookie") {
                            let cookie = val.trim().trim_matches('\'').trim_matches('"').to_string();
                            if !cookie.is_empty() {
                                self.cached_cookie_string = Some(cookie.clone());
                                return Ok(cookie);
                            }
                        }
                    }
                }
            }
            return Err("No cookie found in UCI".into());
        }
    }

    // ── 桌面模式 ──

    #[cfg(not(feature = "openwrt"))]
    pub fn save_cookies(&mut self, cookies: &[crate::config::CookieEntry])
        -> Result<(), AppError>
    {
        let cookie_str: String = cookies.iter()
            .map(|c| format!("{}={}", c.name, c.value))
            .collect::<Vec<_>>()
            .join("; ");
        self.cached_cookie_string = Some(cookie_str);
        let cookies: Vec<_> = cookies.iter()
            .map(|c| crate::config::CookieEntry { name: c.name.clone(), value: c.value.clone() })
            .collect();
        crate::config::Config::save_auth_cookies(&self.config_path, &cookies)?;
        eprintln!("[Auth] 已保存 {} 条 cookie 到 {:?}", cookies.len(), self.config_path);
        Ok(())
    }

    // ── OpenWRT 模式 ──

    #[cfg(feature = "openwrt")]
    pub fn save_cookies(&mut self, cookies: &[crate::config::CookieEntry])
        -> Result<(), AppError>
    {
        use std::process::Command;
        let cookie_str: String = cookies.iter()
            .map(|c| format!("{}={}", c.name, c.value))
            .collect::<Vec<_>>()
            .join("; ");
        self.cached_cookie_string = Some(cookie_str.clone());
        // 写入 UCI option cookie（字符串）
        if !Command::new("uci").args(["set", &format!("pslinkb.@auth[0].cookie={}", cookie_str)]).output()
            .map(|o| o.status.success()).unwrap_or(false)
        {
            return Err("uci set cookie failed".into());
        }
        Command::new("uci").args(["commit", "pslinkb"]).output()
            .map_err(|e| format!("uci commit: {}", e))?;
        Ok(())
    }

    // ── 通用 ──

    pub async fn verify_cookie(&mut self) -> Result<Option<crate::core::biliapi::UserInfo>, AppError> {
        verify_cookie_str(&self.get_cookie_string()?).await
    }

    pub fn has_cookie(&mut self) -> bool { self.get_cookie_string().is_ok() }
    pub fn clear_cache(&mut self) { self.cached_cookie_string = None; }
    pub fn set_cookie(&mut self, cookie: String) { self.cached_cookie_string = Some(cookie); }
    /// TODO: 移除 in auth refactor — 绕过缓存强制重读 TOML，避免写竞态
    pub fn reload_cookie(&mut self) -> Result<String, AppError> {
        self.cached_cookie_string = None;
        self.get_cookie_string()
    }
}

// 自由函数：验证 cookie 字符串 - 不依赖 CookieManager
pub async fn verify_cookie_str(cookie_str: &str) -> Result<Option<crate::core::biliapi::UserInfo>, AppError> {
    let result = crate::core::biliapi::get_user_info(cookie_str).await?;
    match &result {
        Some(info) => {
            eprintln!("[Auth] 已登录 - {}: {}", info.uname, info.uid);
            crate::luci::set("user", &info.uname);
        }
        None => {
            eprintln!("[Auth] 验证失败");
            crate::luci::set("user", "");
        }
    }
    Ok(result)
}
