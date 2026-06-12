/// BLive Manager Actor

use std::sync::{Arc, atomic::{AtomicBool, Ordering}};

use tokio::sync::{broadcast, mpsc};
use serde::{Deserialize, Serialize};
use crate::core::biliapi::{self, StartLiveMode, StartLiveResult};
use crate::core::error::AppError;
use crate::{log_warn, log_error};

// ————————————————————————————————————————————————————————————
// Actor 类型
// ————————————————————————————————————————————————————————————

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LiveMode {
    #[serde(rename = "auto")] Auto,
    #[serde(rename = "manual")] Manual,
}

impl Default for LiveMode { fn default() -> Self { Self::Auto } }

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

#[derive(Debug, Clone, PartialEq)]
pub enum LiveState { Idle, Streaming { rtmp_url: String, stream_key: String } }

#[derive(Debug)]
pub enum LiveCommand {
    Start { room_id: u64, area_v2: String, title: Option<String> },
    Stop { room_id: u64 },
}

#[derive(Debug, Clone)]
pub enum LiveEvent {
    Started { rtmp_url: String, stream_key: String },
    Stopped,
    AuthRequired { face_auth_url: Option<String> },
    StartFailed { error_code: i64, message: String },
    StopFailed { error_code: i64, message: String },
}

// ————————————————————————————————————————————————————————————
// Actor
// ————————————————————————————————————————————————————————————

pub struct BLiveManager {
    cmd_rx: mpsc::Receiver<LiveCommand>,
    event_tx: mpsc::Sender<LiveEvent>,
    cookie_string: String,
    state: LiveState,
    cancel: Arc<AtomicBool>,
}

impl BLiveManager {
    pub fn new(cmd_rx: mpsc::Receiver<LiveCommand>, event_tx: mpsc::Sender<LiveEvent>, cookie_string: String, cancel: Arc<AtomicBool>) -> Self {
        Self { cmd_rx, event_tx, cookie_string, state: LiveState::Idle, cancel }
    }

    pub async fn run(mut self, mut shutdown_rx: broadcast::Receiver<()>) -> Result<(), AppError> {
        let client = self.create_http_client().await?;
        let (_, csrf) = self.parse_cookies()?;
        let uid = self.get_uid();

        'main: loop {
            tokio::select! {
                _ = shutdown_rx.recv() => break 'main,
                cmd = self.cmd_rx.recv() => match cmd {
                    Some(LiveCommand::Start { room_id, area_v2, title }) => {
                        let result = biliapi::start_live(&client, &self.cookie_string, &csrf, uid.as_deref(), room_id, &area_v2, title.as_deref(), StartLiveMode::Full).await;
                        let result = match result {
                            Ok(StartLiveResult::Success { .. }) => result,
                            Ok(StartLiveResult::Failed { code, .. }) if code == 60024 || code == 60043 => result,
                            Ok(StartLiveResult::Failed { code, message, .. }) => {
                                log_warn!("Bili:Live: {} - Try Simple", AppError::bili_api("StartLive", code as i64, message));
                                biliapi::start_live(&client, &self.cookie_string, &csrf, uid.as_deref(), room_id, &area_v2, title.as_deref(), StartLiveMode::Simple).await
                            }
                            Err(e) => {
                                log_warn!("Bili:Live: {} - Try Simple", e);
                                biliapi::start_live(&client, &self.cookie_string, &csrf, uid.as_deref(), room_id, &area_v2, title.as_deref(), StartLiveMode::Simple).await
                            }
                        };
                        match result {
                            Ok(StartLiveResult::Success { rtmp_url, stream_key }) => {
                                self.state = LiveState::Streaming { rtmp_url: rtmp_url.clone(), stream_key: stream_key.clone() };
                                let _ = self.event_tx.send(LiveEvent::Started { rtmp_url, stream_key }).await;
                            }
                            Ok(StartLiveResult::Failed { code, message: _, face_auth_url }) if code == 60043 || code == 60024 => {
                                Self::print_auth_info(code, &face_auth_url);
                                let _ = self.event_tx.send(LiveEvent::AuthRequired { face_auth_url: face_auth_url.clone() }).await;

                                let start = tokio::time::Instant::now();
                                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

                                loop {
                                    tokio::time::sleep(tokio::time::Duration::from_secs_f64(
                                        (0.5 + start.elapsed().as_secs_f64() * 0.1).min(5.0))).await;
                                    if self.cancel.load(Ordering::Relaxed) {
                                        break;
                                    }
                                    if start.elapsed().as_secs_f64() > 60.0 {
                                        crate::luci::clear("qr_url");
                                        log_warn!("Bili:Live: 人脸验证超时 (60s)");
                                        let _ = self.event_tx.send(LiveEvent::StartFailed { error_code: code as i64, message: "验证超时".into() }).await;
                                        break;
                                    }
                                    let result = biliapi::start_live(&client, &self.cookie_string, &csrf, uid.as_deref(), room_id, &area_v2, title.as_deref(), StartLiveMode::Full).await;
                        let result = match result {
                            Ok(StartLiveResult::Failed { .. }) => {
                                biliapi::start_live(&client, &self.cookie_string, &csrf, uid.as_deref(), room_id, &area_v2, title.as_deref(), StartLiveMode::Simple).await
                            }
                            other => other,
                        };
                        match result {
                                        Ok(StartLiveResult::Success { rtmp_url, stream_key }) => {
                                            crate::luci::clear("qr_url");
                                            self.state = LiveState::Streaming { rtmp_url: rtmp_url.clone(), stream_key: stream_key.clone() };
                                            let _ = self.event_tx.send(LiveEvent::Started { rtmp_url, stream_key }).await;
                                            break;
                                        }
                                        _ => {}
                                    }
                                }
                            }
                            Ok(StartLiveResult::Failed { code, message, face_auth_url: _ }) => {
                                log_error!("Bili:Live: {}", AppError::bili_api("StartLive", code as i64, message.clone()));
                                let _ = self.event_tx.send(LiveEvent::StartFailed { error_code: code as i64, message }).await;
                            }
                            Err(e) => {
                                log_error!("Bili:Live: StartLive error - {}", e);
                                let (ec, msg) = Self::parse_api_error(&e.to_string());
                                let _ = self.event_tx.send(LiveEvent::StartFailed { error_code: ec, message: msg }).await;
                            }
                        }
                    }
                    Some(LiveCommand::Stop { room_id }) => {
                        match biliapi::stop_live(&client, &csrf, room_id).await {
                            Ok(_) => { self.state = LiveState::Idle; let _ = self.event_tx.send(LiveEvent::Stopped).await; }
                            Err(e) => {
                                log_error!("Bili:Live: StopLive error - {}", e);
                                let (ec, msg) = Self::parse_api_error(&e.to_string());
                                let _ = self.event_tx.send(LiveEvent::StopFailed { error_code: ec, message: msg }).await;
                            }
                        }
                    }
                    None => break 'main,
                }
            }
        }
        Ok(())
    }

    // ── 基础设施 ──

    async fn create_http_client(&self) -> Result<reqwest::Client, AppError> {
        let (sessdata, csrf) = self.parse_cookies()?;
        let jar = reqwest::cookie::Jar::default();
        jar.add_cookie_str(&format!("SESSDATA={}; bili_jct={}", sessdata, csrf), &"https://api.live.bilibili.com".parse().unwrap());
        Ok(reqwest::Client::builder().cookie_provider(std::sync::Arc::new(jar)).build()?)
    }

    fn parse_cookies(&self) -> Result<(String, String), AppError> {
        let mut sessdata = String::new();
        let mut bili_jct = String::new();
        for pair in self.cookie_string.split(';') {
            if let Some((k, v)) = pair.trim().split_once('=') {
                match k.trim() { "SESSDATA" => sessdata = v.trim().into(), "bili_jct" => bili_jct = v.trim().into(), _ => {} }
            }
        }
        if sessdata.is_empty() || bili_jct.is_empty() { Err("Missing SESSDATA or bili_jct".into()) } else { Ok((sessdata, bili_jct)) }
    }

    fn get_uid(&self) -> Option<String> {
        for pair in self.cookie_string.split(';') {
            if let Some((k, v)) = pair.trim().split_once('=') {
                if k.trim() == "DedeUserID" { return Some(v.trim().into()); }
            }
        }
        None
    }

    fn parse_api_error(err: &str) -> (i64, String) {
        if let Some(pos) = err.find("BiliAPI (") {
            let rest = &err[pos + 8..];
            if let Some(comma) = rest.find(',') {
                if let Ok(code) = rest[..comma].parse::<i64>() {
                    let msg = rest.find(" — ").map(|p| &rest[p + 3..]).unwrap_or(rest);
                    return (code, msg.to_string());
                }
            }
        }
        (-1, err.to_string())
    }

    fn print_auth_info(code: i32, face_auth_url: &Option<String>) {
        if let Some(url) = face_auth_url {
            crate::luci::set("qr_url", url);
            log_warn!("Bili:Live: {}", AppError::bili_api("StartLive", code as i64, crate::core::error::start_live_error(code as i64, "需要验证")));
            eprintln!("  人脸验证链接: {}", url);
            eprintln!("  验证完成后自动开播");
            #[cfg(not(feature = "openwrt"))]
            { print_face_auth_qrcode(url); }
        }
    }
}

/// 打印人脸验证二维码到终端
#[cfg(not(feature = "openwrt"))]
fn print_face_auth_qrcode(url: &str) {
    use qrcode::QrCode;
    match QrCode::new(url) {
        Ok(code) => {
            let image = code.render::<qrcode::render::unicode::Dense1x2>()
                .dark_color(qrcode::render::unicode::Dense1x2::Light)
                .light_color(qrcode::render::unicode::Dense1x2::Dark).build();
            println!("\n人脸验证二维码:\n{}", image);
        }
        Err(e) => { log_warn!("Bili:Live: 二维码生成失败: {}", e); }
    }
}
