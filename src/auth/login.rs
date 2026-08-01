//! 扫码登录流程

use crate::config::CookieEntry;
use crate::core::error::AppError;
use crate::core::biliapi;
use reqwest::cookie::{CookieStore, Jar};
use std::path::Path;
use std::time::Duration;

// —— Cookie 提取 ——

fn is_auth_cookie(name: &str) -> bool {
    matches!(name, "SESSDATA" | "bili_jct" | "buvid3" | "DedeUserID" | "DedeUserID__ckMd5")
}

fn extract_url_cookies(url: &reqwest::Url) -> Vec<CookieEntry> {
    url.query_pairs().filter_map(|(name, value)| {
        is_auth_cookie(&name).then(|| CookieEntry {
            name: name.into_owned(), value: value.into_owned(),
        })
    }).collect()
}

fn extract_cookie_header(header: &str) -> Vec<CookieEntry> {
    header.split(';').filter_map(|pair| {
        let (name, value) = pair.trim().split_once('=')?;
        is_auth_cookie(name).then(|| CookieEntry { name: name.into(), value: value.into() })
    }).collect()
}

fn merge_cookies(cookies: &mut Vec<CookieEntry>, incoming: impl IntoIterator<Item = CookieEntry>) {
    for cookie in incoming {
        if let Some(existing) = cookies.iter_mut().find(|item| item.name == cookie.name) {
            existing.value = cookie.value;
        } else {
            cookies.push(cookie);
        }
    }
}

fn has_login_cookies(cookies: &[CookieEntry]) -> bool {
    ["SESSDATA", "bili_jct"].iter()
        .all(|name| cookies.iter().any(|cookie| cookie.name == *name))
}

fn extract_jar_cookies(jar: &Jar, urls: &[&reqwest::Url]) -> Vec<CookieEntry> {
    let mut cookies = Vec::new();
    let www_url = reqwest::Url::parse("https://www.bilibili.com/").expect("valid Bilibili URL");
    for url in urls.iter().copied().chain([&www_url]) {
        if let Some(header) = jar.cookies(url).and_then(|value| value.to_str().ok().map(str::to_owned)) {
            merge_cookies(&mut cookies, extract_cookie_header(&header));
        }
    }
    cookies
}

// —— 共用：获取 QR + 轮询扫码 ——

enum QrStatus { Confirmed(String), Scanned, Expired, Waiting }

async fn poll_qr(
    show_qr: impl FnOnce(&str),
    mut on_scanned: impl FnMut(),
    mut on_progress: impl FnMut(u32),
) -> Result<Vec<CookieEntry>, AppError> {
    let (client, jar) = biliapi::qr_login_client()?;
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
                let redirect_url = reqwest::Url::parse(&url)
                    .map_err(|_| AppError::from("Invalid QR login redirect URL"))?;

                let mut cookies = extract_url_cookies(&redirect_url);
                match client.get(redirect_url.clone()).send().await {
                    Ok(response) => {
                        let final_url = response.url().clone();
                        merge_cookies(
                            &mut cookies,
                            extract_jar_cookies(&jar, &[&redirect_url, &final_url]),
                        );
                    }
                    Err(_) if has_login_cookies(&cookies) => return Ok(cookies),
                    Err(e) => return Err(e.into()),
                }
                return if !has_login_cookies(&cookies) { Err("Failed to extract login cookies".into()) }
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
