//! 本机 IP 检测

#[cfg(feature = "openwrt")]
pub fn local_ip() -> String {
    use std::net::Ipv4Addr;
    use std::process::Command;
    if let Ok(output) = Command::new("uci").args(["get", "network.lan.ipaddr"]).output() {
        if output.status.success() {
            let ip = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !ip.is_empty() && ip.parse::<Ipv4Addr>().is_ok() {
                return ip;
            }
        }
    }
    fallback_ip()
}

#[cfg(feature = "cli")]
pub fn local_ip() -> String {
    fallback_ip()
}

fn fallback_ip() -> String {
    std::net::UdpSocket::bind("0.0.0.0:0")
        .and_then(|s| { s.connect("8.8.8.8:80")?; s.local_addr().map(|a| a.ip().to_string()) })
        .unwrap_or_else(|_| "127.0.0.1".to_string())
}
