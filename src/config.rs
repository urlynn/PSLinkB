/// PSLinkB Configuration Management

use serde::{Deserialize, Serialize};

use crate::core::error::AppError;

pub const RTMP_PORT: u16 = 1935;
pub const IRC_PORT: u16 = 6667;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub room: RoomConfig,
    #[serde(default)]
    pub auth: AuthConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomConfig {
    #[serde(default = "default_room_id")]
    pub room_id: u64,

    #[serde(default)]
    pub title: String,

    #[serde(default = "default_area_v2")]
    pub area_v2: String,

    #[serde(default)]
    pub live_mode: crate::actors::blive::LiveMode,
}

/// 扫码登录成功后，CookieManager 调用 Config::save_auth_cookies() 写回此段。
/// DedeUserID__ckMd5 是 DedeUserID 的 MD5 校验值，用于避免重复计算
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    #[serde(default)]
    pub cookies: Vec<CookieEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CookieEntry {
    pub name: String,
    pub value: String,
}

// 默认值
fn default_room_id() -> u64 { 0 }

fn default_area_v2() -> String {
    "237".to_string()
}
/// 构造 RTMP URL
pub fn rtmp_url(host: &str, app: &str, key: &str) -> String {
    format!("rtmp://{}:{}/{}/{}", host, RTMP_PORT, app, key)
}

impl Default for Config {
    fn default() -> Self {
        Self {
            room: RoomConfig::default(),
            auth: AuthConfig::default(),
        }
    }
}

impl Default for RoomConfig {
    fn default() -> Self {
        Self {
            room_id: default_room_id(),
            title: String::new(),
            area_v2: default_area_v2(),
            live_mode: crate::actors::blive::LiveMode::default(),
        }
    }
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            cookies: Vec::new(),
        }
    }
}

// Config 方法 — TOML 读写

impl Config {
    /// 从文件加载配置
    #[cfg(not(feature = "openwrt"))]
    pub fn from_file(path: &std::path::Path) -> Result<Self, AppError> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    /// 保存配置到文件
    #[cfg(not(feature = "openwrt"))]
    pub fn to_file(&self, path: &std::path::Path) -> Result<(), AppError> {
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// CookieManager 调用此方法写入扫码登录获取的 cookie。
    #[cfg(not(feature = "openwrt"))]
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
    #[cfg(not(feature = "openwrt"))]
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
    #[cfg(not(feature = "openwrt"))]
    pub fn apply_cli_overrides(
        &mut self,
        room_id: Option<u64>,
        title: Option<String>,
        area: Option<String>,
        mode: Option<crate::actors::blive::LiveMode>,
    ) {
        if let Some(id) = room_id {
            self.room.room_id = id;
        }
        if let Some(t) = title {
            self.room.title = t;
        }
        if let Some(a) = area {
            self.room.area_v2 = a;
        }
        if let Some(m) = mode {
            self.room.live_mode = m;
        }
    }

    /// 保存 room_id 到配置文件（桌面 TOML）
    #[cfg(not(feature = "openwrt"))]
    pub fn save_room_id(path: &std::path::Path, room_id: u64) -> Result<(), AppError> {
        let mut config = Self::from_file(path).unwrap_or_default();
        config.room.room_id = room_id;
        config.to_file(path)
    }

    /// 保存 room_id 到 UCI（OpenWRT）
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

    /// OpenWRT: 从 /etc/config/pslinkb (UCI 格式) 读取配置
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
                if let Some(val) = line.strip_prefix("option cookie") {
                    let val = val.trim().trim_matches('\'').trim_matches('"');
                    for kv in val.split("; ") {
                        if let Some((name, value)) = kv.split_once('=') {
                            config.auth.cookies.push(CookieEntry {
                                name: name.to_string(),
                                value: value.to_string(),
                            });
                        }
                    }
                }
            } else if line.starts_with("option ") && !section.is_empty() {
                if let Some(rest) = line.strip_prefix("option ") {
                    if let Some((key, val)) = rest.split_once(' ') {
                        let val = val.trim_matches('\'').trim_matches('"');
                        match (section.as_str(), key) {
                            ("live", "room_id") => { if let Ok(id) = val.parse::<u64>() { config.room.room_id = id; } }
                            ("live", "area_v2") => { config.room.area_v2 = val.to_string(); }
                            ("live", "title")   => { config.room.title = val.to_string(); }
                            ("live", "live_mode") => {
                                if let Ok(m) = crate::actors::blive::LiveMode::from_str(val) {
                                    config.room.live_mode = m;
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        Ok(config)
    }
}
