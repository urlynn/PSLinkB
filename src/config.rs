//! PSLinkB Configuration Management

use serde::{Deserialize, Serialize};

use crate::core::error::AppError;

pub const RTMP_PORT: u16 = 1935;
pub const IRC_PORT: u16 = 6667;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub live: LiveConfig,
    #[serde(default)]
    pub auth: AuthConfig,

    #[cfg(feature = "openwrt")]
    #[serde(default = "default_true")]
    pub dns_redirect: bool,

    #[cfg(feature = "cli")]
    #[serde(default)]
    pub args: Option<String>,

    #[cfg(feature = "cli")]
    #[serde(default)]
    pub ffmpeg: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveConfig {
    #[serde(default = "default_room_id")]
    pub room_id: u64,

    #[serde(default)]
    pub title: String,

    #[serde(default)]
    pub area_v2: String,

    #[serde(default = "default_area_name")]
    pub area_name: String,

    #[serde(default)]
    pub live_mode: crate::core::blive::LiveMode,
}

/// 扫码登录成功后 init::ensure_cookie() 调用 Config::save_auth_cookies() 写回此段。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AuthConfig {
    #[serde(default)]
    pub cookies: Vec<CookieEntry>,
}

impl AuthConfig {
    pub fn csrf(&self) -> String {
        self.cookies.iter().find(|c| c.name == "bili_jct").map(|c| c.value.clone()).unwrap_or_default()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CookieEntry {
    pub name: String,
    pub value: String,
}

// 默认值
fn default_true() -> bool { true }
fn default_room_id() -> u64 { 0 }

fn default_area_name() -> String {
    "主机游戏".to_string()
}
/// 构造 RTMP URL
pub fn rtmp_url(host: &str, app: &str, key: &str) -> String {
    format!("rtmp://{}:{}/{}/{}", host, RTMP_PORT, app, key)
}

impl Default for Config {
    fn default() -> Self {
        Self {
            live: LiveConfig::default(),
            auth: AuthConfig::default(),
            #[cfg(feature = "openwrt")]
            dns_redirect: true,
            #[cfg(feature = "cli")]
            args: None,
            #[cfg(feature = "cli")]
            ffmpeg: None,
        }
    }
}

impl Default for LiveConfig {
    fn default() -> Self {
        Self {
            room_id: default_room_id(),
            title: String::new(),
            area_v2: String::new(),
            area_name: default_area_name(),
            live_mode: crate::core::blive::LiveMode::default(),
        }
    }
}

// Config 方法 — TOML 读写

impl Config {
    /// 从文件加载配置
    #[cfg(feature = "cli")]
    pub fn from_file(path: &std::path::Path) -> Result<Self, AppError> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    /// 保存配置到文件
    #[cfg(feature = "cli")]
    pub fn to_file(&self, path: &std::path::Path) -> Result<(), AppError> {
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// ensure_cookie() 调用此方法写入扫码登录获取的 cookie。
    #[cfg(feature = "cli")]
    pub fn save_auth_cookies(
        path: &std::path::Path,
        cookies: &[CookieEntry],
    ) -> Result<(), AppError> {
        // 读取现有配置
        let mut config = if path.exists() {
            Self::from_file(path).unwrap_or_default()
        } else {
            // 文件不存在时创建默认配置
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            Self::default()
        };

        config.auth.cookies = cookies.to_vec();
        config.to_file(path)?;
        Ok(())
    }

    /// 从配置文件的 [auth.cookies] 段加载 cookie 字符串
    #[cfg(feature = "cli")]
    pub fn load_cookie_string(path: &std::path::Path) -> Result<String, AppError> {
        let config = Self::from_file(path)?;
        if config.auth.cookies.is_empty() {
            return Err("No cookies found in config file".into());
        }
        Ok(config
            .auth
            .cookies
            .iter()
            .map(|c| format!("{}={}", c.name, c.value))
            .collect::<Vec<_>>()
            .join("; "))
    }

    /// 用 CLI 参数覆盖配置值
    #[cfg(feature = "cli")]
    pub fn apply_cli_overrides(
        &mut self,
        room_id: Option<u64>,
        title: Option<String>,
        area: Option<String>,
        mode: Option<crate::core::blive::LiveMode>,
        ffmpeg: Option<String>,
    ) {
        if let Some(id) = room_id {
            self.live.room_id = id;
        }
        if let Some(t) = title {
            self.live.title = t;
        }
        if let Some(a) = area {
            self.live.area_v2 = a;
        }
        if let Some(m) = mode {
            self.live.live_mode = m;
        }
        if let Some(f) = ffmpeg {
            self.ffmpeg = Some(f);
        }
    }

    #[cfg(feature = "cli")]
    pub fn save_room_id(path: &std::path::Path, room_id: u64) -> Result<(), AppError> {
        let mut config = Self::from_file(path).unwrap_or_default();
        config.live.room_id = room_id;
        config.to_file(path)
    }

    #[cfg(feature = "cli")]
    pub fn save_area_v2(path: &std::path::Path, area_v2: &str) -> Result<(), AppError> {
        let mut config = Self::from_file(path).unwrap_or_default();
        config.live.area_v2 = area_v2.to_string();
        config.to_file(path)
    }

    #[cfg(feature = "cli")]
    pub fn save_args(path: &std::path::Path, args: &str) -> Result<(), AppError> {
        let mut config = Self::from_file(path).unwrap_or_default();
        config.args = Some(args.to_string());
        config.to_file(path)
    }

    #[cfg(feature = "openwrt")]
    pub fn save_room_id(room_id: u64) -> Result<(), AppError> {
        use std::process::Command;
        let set = format!("pslinkb.@live[0].room_id={}", room_id);
        let out = Command::new("uci").args(["set", &set]).output()
            .map_err(|e| AppError::General(format!("uci set: {}", e)))?;
        if !out.status.success() {
            return Err("uci set failed".into());
        }
        Command::new("uci").args(["commit", "pslinkb"]).output()
            .map_err(|e| AppError::General(format!("uci commit: {}", e)))?;
        Ok(())
    }

    #[cfg(feature = "openwrt")]
    pub fn save_area_v2(area_v2: &str) -> Result<(), AppError> {
        use std::process::Command;
        let set = format!("pslinkb.@live[0].area_v2={}", area_v2);
        let out = Command::new("uci").args(["set", &set]).output()
            .map_err(|e| AppError::General(format!("uci set: {}", e)))?;
        if !out.status.success() {
            return Err("uci set failed".into());
        }
        Command::new("uci").args(["commit", "pslinkb"]).output()
            .map_err(|e| AppError::General(format!("uci commit: {}", e)))?;
        Ok(())
    }

    /// OpenWRT: 从 /etc/config/pslinkb 读取配置
    #[cfg(feature = "openwrt")]
    pub fn from_uci() -> Result<Self, AppError> {
        use std::io::BufRead;
        use std::str::FromStr;
        let mut config = Self::default();
        let f = std::fs::File::open("/etc/config/pslinkb")?;
        let mut section = String::new();
        for line in std::io::BufReader::new(f).lines() {
            let line = line?;
            let line = line.trim();
            if line.starts_with("config ") {
                let parts: Vec<&str> = line.splitn(3, ' ').collect();
                if parts.len() >= 2 { section = parts[1].to_string(); }
            } else if line.starts_with("option cookie") && section == "auth" {
                // cookie 改为 uci get 读取
                continue;
            } else if line.starts_with("option ") && !section.is_empty() {
                if let Some(rest) = line.strip_prefix("option ") {
                    if let Some((key, val)) = rest.split_once(' ') {
                        let val = val.trim_matches('\'').trim_matches('"');
                        match (section.as_str(), key) {
                            ("live", "room_id") => { if let Ok(id) = val.parse::<u64>() { config.live.room_id = id; } }
                            ("live", "area_v2") => { config.live.area_v2 = val.to_string(); }
                            ("live", "title")   => { config.live.title = val.to_string(); }
                            ("live", "live_mode") => {
                                if let Ok(m) = crate::core::blive::LiveMode::from_str(val) {
                                    config.live.live_mode = m;
                                }
                            }
                            ("service", "dns_redirect") => {
                                config.dns_redirect = val != "0";
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        // Cookie: uci get 避免转义出错
        if let Ok(out) = std::process::Command::new("uci")
            .args(["get", "pslinkb.auth.cookie"])
            .output()
        {
            if out.status.success() {
                let raw = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !raw.is_empty() {
                    config.auth.cookies.clear();
                    for kv in raw.split("; ") {
                        if let Some((name, value)) = kv.split_once('=') {
                            config.auth.cookies.push(CookieEntry {
                                name: name.to_string(),
                                value: value.to_string(),
                            });
                        }
                    }
                }
            }
        }
        Ok(config)
    }
}
