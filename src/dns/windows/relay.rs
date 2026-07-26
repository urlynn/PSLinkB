//! relay 引流 — 监听 8443 ,裸 TLS 经 HTTP 代理转发

#![cfg(windows)]

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use std::net::SocketAddr;

const RELAY_PORT: u16 = 8443;
const TARGET_PORT: u16 = 443;
const FALLBACK_HOST: &str = "api.twitch.tv";

/// 0.0.0.0:8443。
pub async fn start(proxy_url: String, log: fn(&str)) {
    let listener = match TcpListener::bind(("0.0.0.0", RELAY_PORT)).await {
        Ok(l) => l,
        Err(e) => {
            log(&format!("[ERROR] WinDivert helper error: {}", e));
            return;
        }
    };

    log(&format!("[INFO] Relay listening 0.0.0.0:{} (SNI) via {}", RELAY_PORT, proxy_url));

    loop {
        match listener.accept().await {
            Ok((client, addr)) => {
                let purl = proxy_url.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_client(client, addr, &purl).await {
                        crate::log!(error, "WinDivert helper error: {}", e);
                    }
                });
            }
            Err(_) => {}
        }
    }
}

/// 代理连通性检查
pub async fn check_proxy_connectivity(proxy_url: &str, domain: &str) -> Result<(), String> {
    let proxy_addr = parse_proxy_url(proxy_url)?;
    let mut proxy = TcpStream::connect(proxy_addr).await
        .map_err(|e| format!("Failed to connect proxy {}: {}", proxy_addr, e))?;

    let req = format!("CONNECT {}:{} HTTP/1.1\r\nHost: {}:{}\r\n\r\n",
        domain, TARGET_PORT, domain, TARGET_PORT);
    proxy.write_all(req.as_bytes()).await
        .map_err(|e| format!("Failed to send CONNECT: {}", e))?;

    let mut buf = vec![0u8; 1024];
    let mut total = 0;
    loop {
        let n = proxy.read(&mut buf[total..]).await
            .map_err(|e| format!("Failed to read proxy response: {}", e))?;
        if n == 0 { return Err("Proxy closed early".into()); }
        total += n;
        if buf[..total].windows(4).any(|w| w == b"\r\n\r\n") { break; }
        if total >= buf.len() { return Err("Proxy response too long".into()); }
    }

    let resp = String::from_utf8_lossy(&buf[..total]);
    if !resp.starts_with("HTTP/1.1 200") && !resp.starts_with("HTTP/1.0 200") {
        return Err(format!("Proxy refused CONNECT: {}", resp.lines().next().unwrap_or("?")));
    }
    Ok(())
}

async fn handle_client(mut client: TcpStream, client_addr: SocketAddr, proxy_url: &str) -> Result<(), String> {
    let proxy_addr = parse_proxy_url(proxy_url)?;

    // 读 TLS Client Hello
    let mut hello_buf = vec![0u8; 4096];
    let n = client.read(&mut hello_buf).await
        .map_err(|e| format!("Failed to read Client Hello: {}", e))?;
    let hello = &hello_buf[..n];

    // 解析 SNI
    let target_host = parse_sni(hello).unwrap_or_else(|| {
        FALLBACK_HOST.to_string()
    });
    // 连接代理
    let mut proxy = TcpStream::connect(proxy_addr).await
        .map_err(|e| format!("Failed to connect proxy: {}", e))?;

    // 发 CONNECT <sni>:443
    let connect_req = format!(
        "CONNECT {}:{} HTTP/1.1\r\nHost: {}:{}\r\n\r\n",
        target_host, TARGET_PORT, target_host, TARGET_PORT
    );
    proxy.write_all(connect_req.as_bytes()).await
        .map_err(|e| format!("Failed to send CONNECT: {}", e))?;

    // 读代理响应
    let mut buf = vec![0u8; 1024];
    let mut total = 0;
    loop {
        let n = proxy.read(&mut buf[total..]).await
            .map_err(|e| format!("Failed to read proxy response: {}", e))?;
        if n == 0 { return Err("Proxy closed early".into()); }
        total += n;
        if buf[..total].windows(4).any(|w| w == b"\r\n\r\n") { break; }
        if total >= buf.len() { return Err("Proxy response too long".into()); }
    }

    // 检查状态码 200
    let resp = String::from_utf8_lossy(&buf[..total]);
    let status_line = resp.lines().next().unwrap_or("?");
    if !resp.starts_with("HTTP/1.1 200") && !resp.starts_with("HTTP/1.0 200") {
        // PROXY 失败日志
        crate::dns::windows::windivert::pipe_send(format!(
            "\x1b[31m[WinDivert] {} -> {} ✗ PROXY -> {}\x1b[0m",
            client_addr.ip(), target_host, proxy_url
        ));
        return Err(format!("Proxy refused CONNECT: {}", status_line));
    }

    // PROXY 成功日志
    crate::dns::windows::windivert::pipe_send(format!(
        "\x1b[32m[WinDivert] {} -> {} ✓ PROXY -> {}\x1b[0m",
        client_addr.ip(), target_host, proxy_url
    ));

    proxy.write_all(hello).await
        .map_err(|e| format!("转发 Client Hello 失败: {}", e))?;

    let (mut cr, mut cw) = client.split();
    let (mut pr, mut pw) = proxy.split();

    let c2p = tokio::io::copy(&mut cr, &mut pw);
    let p2c = tokio::io::copy(&mut pr, &mut cw);

    match tokio::try_join!(c2p, p2c) {
        Ok(_) => Ok(()),
        Err(e) => Err(format!("pipe 失败: {}", e)),
    }
}

/// 从 TLS Client Hello 解析 SNI hostname
fn parse_sni(buf: &[u8]) -> Option<String> {
    // TLS Record Header
    if buf.len() < 5 || buf[0] != 0x16 { return None; } 

    // Handshake Header
    let mut pos = 5;
    if buf.len() < pos + 4 { return None; }
    if buf[pos] != 0x01 { return None; }
    pos += 4;

    // Protocol version
    pos += 2;

    // Random
    pos += 32;

    // Session ID
    if buf.len() < pos + 1 { return None; }
    let session_id_len = buf[pos] as usize;
    pos += 1 + session_id_len;

    // Cipher suites
    if buf.len() < pos + 2 { return None; }
    let cipher_len = u16::from_be_bytes([buf[pos], buf[pos + 1]]) as usize;
    pos += 2 + cipher_len;

    // Compression methods
    if buf.len() < pos + 1 { return None; }
    let comp_len = buf[pos] as usize;
    pos += 1 + comp_len;

    // Extensions
    if buf.len() < pos + 2 { return None; }
    let ext_len = u16::from_be_bytes([buf[pos], buf[pos + 1]]) as usize;
    pos += 2;

    let ext_end = pos + ext_len;
    if buf.len() < ext_end { return None; }

    // 遍历 extensions 找 SNI
    while pos + 4 <= ext_end {
        let ext_type = u16::from_be_bytes([buf[pos], buf[pos + 1]]);
        let ext_data_len = u16::from_be_bytes([buf[pos + 2], buf[pos + 3]]) as usize;
        pos += 4;

        if ext_type == 0x0000 {
            // SNI extension
            let sni_end = pos + ext_data_len;
            if buf.len() < sni_end { return None; }

            // Server name list length
            if pos + 2 > sni_end { return None; }
            let mut sni_pos = pos + 2;

            while sni_pos + 3 <= sni_end {
                let name_type = buf[sni_pos]; 
                let name_len = u16::from_be_bytes([buf[sni_pos + 1], buf[sni_pos + 2]]) as usize;
                sni_pos += 3;

                if name_type == 0x00 && sni_pos + name_len <= sni_end {
                    if let Ok(name) = std::str::from_utf8(&buf[sni_pos..sni_pos + name_len]) {
                        return Some(name.to_string());
                    }
                }
                sni_pos += name_len;
            }
        }

        pos += ext_data_len;
    }

    None
}

fn parse_proxy_url(url: &str) -> Result<SocketAddr, String> {
    let url = url.strip_prefix("http://").ok_or("proxy_url 必须 http:// 开头")?;
    url.parse::<SocketAddr>().map_err(|e| format!("Failed to parse proxy address: {}", e))
}
