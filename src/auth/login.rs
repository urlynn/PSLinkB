//! 扫码登录流程

use crate::config::CookieEntry;
use crate::core::error::AppError;
use crate::core::biliapi;
use reqwest::cookie::{CookieStore, Jar};
use std::path::Path;
use std::sync::Arc;
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

fn is_trusted_bilibili_url(url: &reqwest::Url) -> bool {
    url.scheme() == "https" && url.host_str().is_some_and(|host| {
        matches!(host, "bilibili.com" | "biligame.com")
            || host.ends_with(".bilibili.com")
            || host.ends_with(".biligame.com")
    })
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
    let jar = Arc::new(Jar::default());
    let client = reqwest::Client::builder()
        .cookie_provider(Arc::clone(&jar))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() < 10 && is_trusted_bilibili_url(attempt.url()) {
                attempt.follow()
            } else {
                attempt.stop()
            }
        }))
        .build()?;

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
                if !is_trusted_bilibili_url(&redirect_url) {
                    return Err("Refusing untrusted QR login redirect URL".into());
                }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_legacy_cookies_from_redirect_url() {
        let url = reqwest::Url::parse(
            "https://passport.bilibili.com/login?SESSDATA=session%3Dvalue&bili_jct=csrf&ignored=value",
        ).unwrap();

        assert_eq!(extract_url_cookies(&url), vec![
            CookieEntry { name: "SESSDATA".into(), value: "session=value".into() },
            CookieEntry { name: "bili_jct".into(), value: "csrf".into() },
        ]);
    }

    #[test]
    fn extracts_only_auth_cookies_from_cookie_header() {
        assert_eq!(
            extract_cookie_header("SESSDATA=session==; ignored=value; bili_jct=csrf"),
            vec![
                CookieEntry { name: "SESSDATA".into(), value: "session==".into() },
                CookieEntry { name: "bili_jct".into(), value: "csrf".into() },
            ],
        );
    }

    #[test]
    fn reads_auth_cookies_from_jar() {
        let jar = Jar::default();
        let url = reqwest::Url::parse("https://passport.bilibili.com/").unwrap();
        jar.add_cookie_str("SESSDATA=session; Domain=.bilibili.com; Path=/", &url);
        jar.add_cookie_str("bili_jct=csrf; Domain=.bilibili.com; Path=/", &url);
        jar.add_cookie_str("ignored=value; Domain=.bilibili.com; Path=/", &url);

        let cookies = extract_jar_cookies(&jar, &[&url]);
        assert_eq!(cookies.len(), 2);
        assert!(cookies.contains(&CookieEntry { name: "SESSDATA".into(), value: "session".into() }));
        assert!(cookies.contains(&CookieEntry { name: "bili_jct".into(), value: "csrf".into() }));
    }

    #[test]
    fn merge_replaces_duplicate_cookie_values() {
        let mut cookies = vec![
            CookieEntry { name: "SESSDATA".into(), value: "legacy".into() },
        ];

        merge_cookies(&mut cookies, vec![
            CookieEntry { name: "SESSDATA".into(), value: "redirect".into() },
            CookieEntry { name: "bili_jct".into(), value: "csrf".into() },
        ]);

        assert_eq!(cookies, vec![
            CookieEntry { name: "SESSDATA".into(), value: "redirect".into() },
            CookieEntry { name: "bili_jct".into(), value: "csrf".into() },
        ]);
    }

    #[test]
    fn requires_session_and_csrf_cookies() {
        let mut cookies = vec![
            CookieEntry { name: "SESSDATA".into(), value: "session".into() },
            CookieEntry { name: "buvid3".into(), value: "visitor".into() },
        ];
        assert!(!has_login_cookies(&cookies));

        cookies.push(CookieEntry { name: "bili_jct".into(), value: "csrf".into() });
        assert!(has_login_cookies(&cookies));
    }

    #[test]
    fn trusts_only_https_bilibili_hosts() {
        for url in [
            "https://bilibili.com/",
            "https://passport.bilibili.com/",
            "https://passport.biligame.com/",
        ] {
            assert!(is_trusted_bilibili_url(&reqwest::Url::parse(url).unwrap()));
        }
        for url in [
            "http://passport.bilibili.com/",
            "https://bilibili.com.evil.example/",
            "https://biligame.com.evil.example/",
            "https://evil-bilibili.com/",
            "https://example.com/",
        ] {
            assert!(!is_trusted_bilibili_url(&reqwest::Url::parse(url).unwrap()));
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
