//! 扫码登录流程

use crate::config::CookieEntry;
use crate::core::error::AppError;
use crate::core::biliapi;
use std::path::Path;
use std::time::Duration;

// —— Cookie 提取 ——

fn extract_cookies(url: &str) -> Vec<CookieEntry> {
    let query = url.split('?').nth(1).unwrap_or("");
    query.split('&').filter_map(|pair| {
        let (k, v) = pair.split_once('=')?;
        if matches!(k, "SESSDATA" | "bili_jct" | "buvid3" | "DedeUserID" | "DedeUserID__ckMd5") {
            Some(CookieEntry { name: k.into(), value: v.into() })
        } else { None }
    }).collect()
}

// —— 共用：获取 QR + 轮询扫码 ——

enum QrStatus { Confirmed(String), Scanned, Expired, Waiting }

async fn poll_qr(
    show_qr: impl FnOnce(&str),
    mut on_scanned: impl FnMut(),
    mut on_progress: impl FnMut(u32),
) -> Result<Vec<CookieEntry>, AppError> {
    let client = reqwest::Client::new();

    let (qr_url, key) = biliapi::generate_qr(&client).await?;
    show_qr(&qr_url);

    let mut attempts = 0u32;
    loop {
        if attempts >= 300 { return Err("二维码登录超时 (5分钟)".into()); }
        tokio::time::sleep(Duration::from_secs(1)).await;
        attempts += 1;

        let s = biliapi::poll_qr_status(&client, &key).await?;

        let status = match s.code {
            0     => QrStatus::Confirmed(s.url),
            86038 => QrStatus::Expired,
            86090 => QrStatus::Scanned,
            _     => QrStatus::Waiting,
        };

        match status {
            QrStatus::Confirmed(url) => {
                let cookies = extract_cookies(&url);
                return if cookies.is_empty() { Err("Failed to extract cookies".into()) }
                       else { Ok(cookies) };
            }
            QrStatus::Expired => return Err("二维码已过期".into()),
            QrStatus::Scanned => on_scanned(),
            QrStatus::Waiting => on_progress(attempts),
        }
    }
}

// —— 桌面模式 ——

#[cfg(feature = "cli")]
pub async fn scan_qr_blocking(
    _config_path: &Path, _config: &crate::config::Config,
) -> Result<Vec<CookieEntry>, AppError> {
    eprintln!();
    eprintln!("正在获取二维码...");
    let mut scanned = false;
    poll_qr(
        |url| {
            print_qrcode_ascii(url);
            eprintln!("等待扫码...");
        },
        || {
            if !scanned { eprintln!("已扫描，请在手机上确认..."); scanned = true; }
        },
        |secs| {
            if secs % 30 == 0 { eprintln!("等待扫码... ({}秒)", secs); }
        },
    ).await.inspect(|_| {
        eprintln!("扫码成功！");
        eprintln!();
    })
}

#[cfg(feature = "cli")]
fn print_qrcode_ascii(url: &str) {
    use qrcode::QrCode;
    eprintln!("请使用 B站客户端扫描下方二维码:");
    let code = QrCode::new(url).unwrap();
    let image = code.render::<qrcode::render::unicode::Dense1x2>()
        .dark_color(qrcode::render::unicode::Dense1x2::Light)
        .light_color(qrcode::render::unicode::Dense1x2::Dark).build();
    eprintln!("{}", image);
}

// —— OpenWRT 模式 ——

#[cfg(feature = "openwrt")]
pub async fn scan_qr_blocking(
    _config_path: &Path, _config: &crate::config::Config,
) -> Result<Vec<CookieEntry>, AppError> {
    crate::luci::set("qr", r#"{"url":"","status":"generating"}"#);
    let mut scanned = false;
    let qr_url = std::cell::Cell::new(None::<String>);
    let result = poll_qr(
        |url| {
            qr_url.set(Some(url.to_string()));
            crate::luci::set("qr", &format!(r#"{{"url":"{}","status":"waiting"}}"#, url));
        },
        || {
            if !scanned {
                if let Some(url) = qr_url.take() {
                    crate::luci::set("qr", &format!(r#"{{"url":"{}","status":"scanned"}}"#, url));
                }
                scanned = true;
            }
        },
        |_| {},
    ).await;

    match &result {
        Ok(_) => {
            crate::luci::set("qr", r#"{"url":"","status":"done"}"#);
        }
        Err(e) => {
            let msg = e.to_string();
            let status = if msg.contains("超时") || msg.contains("过期") {
                "expired"
            } else {
                return Ok(vec![]); 
            };
            crate::luci::set("qr", &format!(r#"{{"url":"","status":"{}"}}"#, status));
        }
    }
    result
}
