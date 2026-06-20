/// DNS check

#[cfg(feature = "dns-redirect")]
mod dns_redirect_imports {
    pub(crate) use std::net::{Ipv4Addr, SocketAddr};
    pub(crate) use hickory_proto::op::{Message, MessageType, OpCode, Query};
    pub(crate) use hickory_proto::rr::{Name, RecordType};
    pub(crate) use hickory_proto::serialize::binary::{BinDecodable, BinEncodable};
}

#[cfg(feature = "dns-redirect")]
use std::time::Duration;
#[cfg(feature = "dns-redirect")]
use owo_colors::{OwoColorize, Stream};

pub const REDIRECT_DOMAINS: &[&str] = &[
    "global-contribute.live-video.net",
    "irc.twitch.tv",
    "live.twitch.tv",
    "contribute.live-video.net",
    "tmi.twitch.tv",
];

pub const CHECK_DOMAINS: &[&str] = &[
    "irc.twitch.tv",
    "ingest.global-contribute.live-video.net",
];

const DNS_STATUS_PATH: &str = "/tmp/pslinkb/dns_status";

#[derive(serde::Serialize)]
struct DnsStatus {
    checking: bool,
    enabled: bool,
    target: String,
    actual: String,
    ok: bool,
}

pub fn write_dns_status(checking: bool, enabled: bool, target: &str, actual: &str, ok: bool) {
    let status = DnsStatus {
        checking,
        enabled,
        target: target.to_string(),
        actual: actual.to_string(),
        ok,
    };
    if let Ok(json) = serde_json::to_string(&status) {
        let _ = std::fs::write(DNS_STATUS_PATH, json);
    }
}

// ── DNS 查询 ──

#[cfg(feature = "dns-redirect")]
#[derive(Debug)]
pub struct CheckResult {
    pub domain: String,
    pub expected: String,
    pub actual: Option<String>,
    pub success: bool,
}

/// 本地查询
#[cfg(feature = "dns-redirect")]
pub async fn resolve(domain: String) -> Option<String> {
    use dns_redirect_imports::*;
    let socket = tokio::net::UdpSocket::bind("0.0.0.0:0").await.ok()?;
    socket.connect(SocketAddr::new(
        std::net::IpAddr::V4(Ipv4Addr::LOCALHOST), 53
    )).await.ok()?;

    let name = Name::from_utf8(domain).ok()?;
    let mut query = Message::new(0x42, MessageType::Query, OpCode::Query);
    query.add_query(Query::query(name, RecordType::A));

    let query_bytes = query.to_bytes().ok()?;
    socket.send(&query_bytes).await.ok()?;

    let mut buf = [0u8; 512];
    let (len, _) = tokio::time::timeout(Duration::from_secs(2), socket.recv_from(&mut buf))
        .await.ok()?
        .ok()?;

    let response = Message::from_bytes(&buf[..len]).ok()?;
    for answer in &response.answers {
        if let hickory_proto::rr::RData::A(a) = &answer.data {
            return Some(a.0.to_string());
        }
    }
    None
}

/// 系统解析器查询
#[cfg(feature = "dns-redirect")]
pub async fn system_resolve(domain: String) -> Option<String> {
    use std::net::ToSocketAddrs;
    let addr_str = format!("{}:0", domain);
    tokio::task::spawn_blocking(move || {
        addr_str.to_socket_addrs().ok()
            .and_then(|addrs| {
                addrs.filter_map(|a| match a.ip() {
                    std::net::IpAddr::V4(v4) => Some(v4.to_string()),
                    _ => None,
                }).next()
            })
    }).await.ok().flatten()
}

// ── OpenWRT 模式 ──
#[cfg(feature = "openwrt")]
pub async fn resolve_one(domain: &str) -> Option<String> {
    use std::net::ToSocketAddrs;
    let addr_str = format!("{}:0", domain);
    tokio::task::spawn_blocking(move || {
        addr_str.to_socket_addrs().ok()
            .and_then(|addrs| {
                addrs.filter_map(|a| match a.ip() {
                    std::net::IpAddr::V4(v4) => Some(v4.to_string()),
                    _ => None,
                }).next()
            })
    }).await.ok().flatten()
}

/// 逐域名检测
#[cfg(feature = "dns-redirect")]
pub async fn check_domain<F, Fut>(
    domains: &[&str],
    expected_ip: &str,
    resolve_fn: F,
) -> Vec<CheckResult>
where
    F: Fn(String) -> Fut,
    Fut: std::future::Future<Output = Option<String>>,
{
    let mut results = Vec::new();

    for domain in domains {
        let ip = resolve_fn(domain.to_string()).await.unwrap_or_else(|| "无解析".into());

        if ip == expected_ip {
            let ip_colored = ip.if_supports_color(Stream::Stderr, |s| s.green());
            let check = "✓".if_supports_color(Stream::Stderr, |s| s.green());
            eprintln!("[System] DNS Check - {} -> {} {}", domain, ip_colored, check);
            results.push(CheckResult { domain: domain.to_string(), expected: expected_ip.to_string(), actual: Some(ip), success: true });
        } else {
            let ip_colored = ip.if_supports_color(Stream::Stderr, |s| s.red());
            let cross = "✗".if_supports_color(Stream::Stderr, |s| s.red());
            eprintln!("[System] DNS Check - {} -> {} {}", domain, ip_colored, cross);
            results.push(CheckResult { domain: domain.to_string(), expected: expected_ip.to_string(), actual: Some(ip), success: false });
        }
    }

    results
}

/// 输出结果
#[cfg(feature = "dns-redirect")]
pub fn summarize(results: &[CheckResult]) -> bool {
    let all_ok = results.iter().all(|r| r.success);

    if all_ok {
        crate::log!(ok, "[System] DNS Check - ✓ 重定向正常");
        return true;
    }

    crate::log!(error, "[System] DNS Check - ✗ 重定向失败，请检查端口 53 是否被占用");
    false
}