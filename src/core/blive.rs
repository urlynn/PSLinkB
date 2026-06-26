//! BLiveManager

use serde::{Deserialize, Serialize};
use crate::core::biliapi::{self, LiveClient, StartLiveResult};
use crate::core::error::AppError;
use crate::log;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum LiveMode {
    #[serde(rename = "auto")]
    #[default]
    Auto,
    #[serde(rename = "manual")] Manual,
}

impl std::str::FromStr for LiveMode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "auto" => Ok(Self::Auto),
            "manual" => Ok(Self::Manual),
            _ => Err(format!("Invalid live_mode: '{}'", s)),
        }
    }
}

/// startLive 结果
pub enum StartOutcome {
    Started { rtmp_url: String, stream_key: String, client: LiveClient },
    AuthRequired { face_auth_url: Option<String> },
    Failed { code: i64, message: String },
}

/// startLive - 按结果分类返回
pub async fn try_start_live(cookie: &str, csrf: &str, room_id: u64, area_v2: &str, title: Option<&str>) -> StartOutcome {
    let client = reqwest::Client::new();
    let uid = get_uid(cookie);

    // electron 优先
    let mut chosen = LiveClient::Electron;
    let mut result = biliapi::start_live_electron(&client, cookie, csrf, uid.as_deref(), room_id, area_v2, title).await;

    match &result {
        Err(e) => {
            log!(error, "Bili:Live: Electron - {} - 开播失败", e);
            return StartOutcome::Failed { code: -1, message: e.to_string() };
        }
        Ok(StartLiveResult::Success { .. }) => {}
        Ok(StartLiveResult::Failed { code: 60024 | 60043, .. }) => {}
        Ok(StartLiveResult::Failed { code, message, .. }) => {
            log!(warn, "Bili:Live: {} - Electron 失败 - Fallback Livehime", AppError::bili_api("StartLive", *code as i64, message.clone()));
            chosen = LiveClient::Livehime;
            result = biliapi::start_live_livehime(&client, cookie, csrf, uid.as_deref(), room_id, area_v2, title).await;
        }
    }

    match result {
        Ok(StartLiveResult::Success { rtmp_url, stream_key }) =>
            StartOutcome::Started { rtmp_url, stream_key, client: chosen },
        Ok(StartLiveResult::Failed { code, message: _, face_auth_url }) if code == 60043 || code == 60024 => {
            print_auth_info(code, &face_auth_url);
            StartOutcome::AuthRequired { face_auth_url }
        }
        Ok(StartLiveResult::Failed { code, message, face_auth_url: _ }) => {
            log!(error, "Bili:Live: {}", AppError::bili_api("StartLive", code as i64, message.clone()));
            StartOutcome::Failed { code: code as i64, message }
        }
        Err(e) => {
            log!(error, "Bili:Live: StartLive error - {}", e);
            let (code, message) = e.code_and_message();
            StartOutcome::Failed { code, message }
        }
    }
}

/// 关播
pub async fn try_stop_live(cookie: &str, csrf: &str, room_id: u64, client_id: LiveClient) -> Result<(), (i64, String)> {
    let client = reqwest::Client::new();
    let result = match client_id {
        LiveClient::Electron => biliapi::stop_live_electron(&client, cookie, csrf, room_id).await,
        LiveClient::Livehime => biliapi::stop_live_livehime(&client, cookie, csrf, room_id).await,
    };
    match result {
        Ok(_) => Ok(()),
            Err(e) => {
                log!(error, "Bili:Live: StopLive error - {}", e);
                Err(e.code_and_message())
            }
    }
}

// ── 基础设施 ──

fn get_uid(cookie: &str) -> Option<String> {
    cookie.split(';').filter_map(|p| p.trim().split_once('='))
        .find(|(k, _)| k.trim() == "DedeUserID")
        .map(|(_, v)| v.trim().to_string())
}

fn print_auth_info(code: i32, face_auth_url: &Option<String>) {
    if let Some(url) = face_auth_url {
        crate::luci::set("qr_url", url);
        log!(warn, "Bili:Live: {}", AppError::bili_api("StartLive", code as i64, crate::core::error::start_live_error(code as i64, "需要验证")));
        eprintln!("  人脸验证链接: {}", url);
        eprintln!("  验证完成后自动开播");
        #[cfg(feature = "cli")]
        { print_face_auth_qrcode(url); }
    }
}

/// 打印人脸验证二维码到终端
#[cfg(feature = "cli")]
fn print_face_auth_qrcode(url: &str) {
    use qrcode::QrCode;
    match QrCode::new(url) {
        Ok(code) => {
            let image = code.render::<qrcode::render::unicode::Dense1x2>()
                .dark_color(qrcode::render::unicode::Dense1x2::Light)
                .light_color(qrcode::render::unicode::Dense1x2::Dark).build();
            println!("\n人脸验证二维码:\n{}", image);
        }
        Err(e) => { log!(warn, "Bili:Live: 二维码生成失败: {}", e); }
    }
}
