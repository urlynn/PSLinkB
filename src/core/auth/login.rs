/// 登录管理模块 — 扫码登录流程
#[cfg(feature = "openwrt")]
use crate::core::error::AppError;

// —— 桌面模式 ——
#[cfg(not(feature = "openwrt"))]
mod qrcode_login {
    use crate::core::auth::cookie::CookieManager;
    use crate::core::error::AppError;
    use crate::config::CookieEntry;
    use serde::Deserialize;

    /// B站 API 响应
    #[derive(Debug, Deserialize)]
    struct BilibiliResponse<T> {
        code: i64,
        message: String,
        data: Option<T>,
    }

    /// 二维码生成响应
    #[derive(Debug, Deserialize)]
    struct QrCodeData {
        url: String,
        #[serde(alias = "oauth_key")]
        qrcode_key: String,
    }

    /// 扫码状态响应
    #[derive(Debug, Deserialize)]
    #[allow(dead_code)]
    struct ScanStatusData {
        url: String,
        refresh_token: String,
        timestamp: i64,
        code: i64,
        message: String,
        #[serde(default)]
        oauth_key: Option<String>,
    }

    /// 生成终端二维码
    fn print_qrcode_ascii(url: &str) {
        use qrcode::QrCode;

        println!("请使用 B站客户端扫描下方二维码:");

        let code = QrCode::new(url).unwrap();
        let image = code
            .render::<qrcode::render::unicode::Dense1x2>()
            .dark_color(qrcode::render::unicode::Dense1x2::Light)
            .light_color(qrcode::render::unicode::Dense1x2::Dark)
            .build();

        println!("{}", image);
        println!("等待扫码...");
    }

    /// 从 URL 中提取 Cookie
    fn extract_cookies_from_url(url: &str) -> Vec<CookieEntry> {
        use url::Url;

        if let Ok(parsed_url) = Url::parse(url) {
            let mut cookies = Vec::new();

            for (key, value) in parsed_url.query_pairs() {
                if matches!(
                    key.as_ref(),
                    "SESSDATA" | "bili_jct" | "buvid3" | "DedeUserID" | "DedeUserID__ckMd5"
                ) {
                    cookies.push(CookieEntry {
                        name: key.to_string(),
                        value: value.to_string(),
                    });
                }
            }

            return cookies;
        }

        Vec::new()
    }

    /// 扫码登录流程
    pub async fn scan_login(
        cookie_manager: &mut CookieManager,
    ) -> Result<crate::core::biliapi::UserInfo, AppError> {
        let client = reqwest::Client::new();

        //获取二维码 URL
        println!();
        println!("正在获取二维码...");
        let qr_response = client
            .get("https://passport.bilibili.com/x/passport-login/web/qrcode/generate")
            .send()
            .await?;

        let qr_data: BilibiliResponse<QrCodeData> = qr_response.json().await?;

        if qr_data.code != 0 {
            return Err(format!("Failed to get QR code: {}", qr_data.message).into());
        }

        let qr_info = qr_data.data.ok_or("No QR code data")?;

        //显示二维码
        print_qrcode_ascii(&qr_info.url);

        //轮询检查扫码状态
        let qrcode_key = qr_info.qrcode_key.clone();
        let mut attempts = 0;
        let max_attempts = 300; // 约 5 分钟
        let mut scanned = false;

        loop {
            if attempts >= max_attempts {
                return Err("QR code login timeout (5 minutes)".into());
            }

            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            attempts += 1;

            let status_url = format!(
                "https://passport.bilibili.com/x/passport-login/web/qrcode/poll?qrcode_key={}",
                qrcode_key
            );

            let status_response = client.get(&status_url).send().await?;
            let status_data: BilibiliResponse<ScanStatusData> = status_response.json().await?;

            if status_data.code != 0 {
                continue;
            }

            let status = status_data.data.ok_or("No status data")?;

            match status.code {
                0 => {
                    // 扫码成功
                    println!("扫码成功！");
                    println!();

                    let cookie_entries = extract_cookies_from_url(&status.url);

                    if cookie_entries.is_empty() {
                        return Err("Failed to extract cookies from response".into());
                    }

                    // 通过 CookieManager -> Config 写入配置文件
                    cookie_manager.save_cookies(&cookie_entries)?;

                    // 验证并显示用户信息
                    let user_info = cookie_manager.verify_cookie().await?
                        .ok_or_else(|| AppError::General("Cookie验证失败".into()))?;

                    return Ok(user_info);
                }
                86038 => {
                    return Err("二维码已过期，请重试".into());
                }
                86090 => {
                    // 86090 = 已扫描，等待手机确认
                    if !scanned {
                        println!("已扫描，请在手机上确认...");
                        scanned = true;
                    }
                }
                86101 => {
                    // 86101 = 未扫描，等待中
                    if attempts % 30 == 0 {
                        println!("等待扫码... ({}秒)", attempts);
                    }
                }
                _ => {
                    if attempts % 30 == 0 {
                        println!("等待扫码... ({}秒, code={})", attempts, status.code);
                    }
                }
            }
        }
    }
}

#[cfg(not(feature = "openwrt"))]
pub use qrcode_login::scan_login;

// —— OpenWRT 模式（文件 IPC + LuCI）——
#[cfg(feature = "openwrt")]
pub async fn scan_login(
    cookie_manager: &mut crate::core::auth::CookieManager,
) -> Result<crate::core::biliapi::UserInfo, AppError> {
    use crate::config::CookieEntry;
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    struct BilibiliResponse<T> {
        code: i64,
        message: String,
        data: Option<T>,
    }

    #[derive(Debug, Deserialize)]
    struct QrCodeData {
        url: String,
        #[serde(alias = "oauth_key")]
        qrcode_key: String,
    }

    #[derive(Debug, Deserialize)]
    #[allow(dead_code)]
    struct ScanStatusData {
        url: String,
        code: i64,
        message: String,
    }

    let client = reqwest::Client::new();
    crate::luci::set("qr_status", "generating");

    // 1. 获取二维码
    let qr_response = client
        .get("https://passport.bilibili.com/x/passport-login/web/qrcode/generate")
        .send().await?;
    let qr_data: BilibiliResponse<QrCodeData> = qr_response.json().await?;
    if qr_data.code != 0 {
        crate::luci::set("qr_status", &format!("error:{}", qr_data.message));
        return Err(format!("Failed to get QR code: {}", qr_data.message).into());
    }
    let qr_info = qr_data.data.ok_or("No QR code data")?;

    crate::luci::set("qr_url", &qr_info.url);
    crate::luci::set("qr_status", "waiting");

    // 2. 轮询
    let qrcode_key = qr_info.qrcode_key;
    let mut attempts = 0u32;
    let mut scanned = false;

    loop {
        if attempts >= 300 {
            crate::luci::set("qr_status", "expired");
            crate::luci::clear("qr_url");
            return Err("QR code login timeout (5 minutes)".into());
        }
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        attempts += 1;

        let status_url = format!(
            "https://passport.bilibili.com/x/passport-login/web/qrcode/poll?qrcode_key={}",
            qrcode_key
        );

        let status_response = client.get(&status_url).send().await?;
        let status_data: BilibiliResponse<ScanStatusData> = status_response.json().await?;
        if status_data.code != 0 { continue; }
        let status = status_data.data.ok_or("No status data")?;

        match status.code {
            0 => {
                crate::luci::set("qr_status", "confirmed");
                crate::luci::clear("qr_url");

                // Parse cookies from URL query string
                let cookies: Vec<CookieEntry> = status.url
                    .split('?').nth(1).unwrap_or("")
                    .split('&')
                    .filter_map(|pair| {
                        let mut kv = pair.splitn(2, '=');
                        let key = kv.next()?;
                        let val = kv.next()?;
                        if matches!(key, "SESSDATA"|"bili_jct"|"buvid3"|"DedeUserID"|"DedeUserID__ckMd5") {
                            Some(CookieEntry { name: key.to_string(), value: val.to_string() })
                        } else { None }
                    })
                    .collect();

                if cookies.is_empty() {
                    crate::luci::set("qr_status", "error:no_cookies");
                    return Err("Failed to extract cookies".into());
                }

                cookie_manager.save_cookies(&cookies)?;
                let user_info = cookie_manager.verify_cookie().await?
                    .ok_or_else(|| AppError::General("Cookie验证失败".into()))?;
                crate::luci::set("qr_status", "done");
                return Ok(user_info);
            }
            86038 => {
                crate::luci::set("qr_status", "expired");
                crate::luci::clear("qr_url");
                return Err("QR code expired".into());
            }
            86090 => {
                if !scanned { crate::luci::set("qr_status", "scanned"); scanned = true; }
            }
            _ => {}
        }
    }
}
