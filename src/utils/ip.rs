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
    enum_ip()
}

#[cfg(feature = "cli")]
fn enum_ip() -> String {
    use std::net::Ipv4Addr;

    const LOOPBACK: Ipv4Addr = Ipv4Addr::new(127, 0, 0, 1);

    let mut pick: [Option<Ipv4Addr>; 4] = [None; 4];
    let ifaces = if_addrs::get_if_addrs().unwrap_or_default();
    for iface in ifaces {
        if let if_addrs::IfAddr::V4(v4) = iface.addr {
            let ip = v4.ip;
            if ip.is_loopback() { continue; }
            let o = ip.octets();
            let rank = if o[0] == 192 && o[1] == 168 { 0 }
                else if o[0] == 10 { 1 }
                else if !ip.is_private()
                    && !ip.is_link_local()
                    && !ip.is_multicast()
                    && !ip.is_broadcast()
                    && !ip.is_unspecified()
                    && !(o[0] == 198 && (18..=19).contains(&o[1])) { 2 }
                else { 3 };
            if pick[rank].is_none() { pick[rank] = Some(ip); }
        }
    }
    pick.iter().flatten().next().unwrap_or(&LOOPBACK).to_string()
}

#[cfg(feature = "openwrt")]
fn fallback_ip() -> String {
    std::net::UdpSocket::bind("0.0.0.0:0")
        .and_then(|s| { s.connect("8.8.8.8:80")?; s.local_addr().map(|a| a.ip().to_string()) })
        .unwrap_or_else(|_| "127.0.0.1".to_string())
}
