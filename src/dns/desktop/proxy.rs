//! DNS 代理 — 重定向指定域名返回本机 IP，其余透传到上游 DNS。

use std::collections::HashSet;
use std::net::{Ipv4Addr, SocketAddr};
use tokio::net::UdpSocket;

use hickory_proto::op::{Message, MessageType, OpCode, ResponseCode};
use hickory_proto::rr::{RData, Record, RecordType};
use hickory_proto::rr::rdata::A;
use hickory_proto::serialize::binary::{BinDecodable, BinEncodable};

pub struct DnsProxy {
    socket: UdpSocket,
    redirect_domains: HashSet<String>,
    target_ip: Ipv4Addr,
    upstream: SocketAddr,
}

fn detect_upstream() -> SocketAddr {
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        if let Ok(out) = Command::new("powershell")
            .args(["-Command", "(Get-DnsClientServerAddress -AddressFamily IPv4 | Where-Object {$_.ServerAddresses.Count -gt 0}).ServerAddresses[0]"])
            .output()
        {
            if out.status.success() {
                let addr = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if let Ok(ip) = addr.parse::<Ipv4Addr>() {
                    return SocketAddr::new(std::net::IpAddr::V4(ip), 53);
                }
            }
        }
    }
    #[cfg(unix)]
    {
        if let Ok(content) = std::fs::read_to_string("/etc/resolv.conf") {
            for line in content.lines() {
                if line.starts_with("nameserver") {
                    if let Some(ip) = line.split_whitespace().nth(1) {
                        if let Ok(addr) = ip.parse::<Ipv4Addr>() {
                            return SocketAddr::new(std::net::IpAddr::V4(addr), 53);
                        }
                    }
                }
            }
        }
    }
    SocketAddr::new(std::net::IpAddr::V4(Ipv4Addr::new(114, 114, 114, 114)), 53)
}

impl DnsProxy {
    pub async fn new(
        domains: &[&str],
        target_ip: Ipv4Addr,
    ) -> Result<Self, std::io::Error> {
        let socket = UdpSocket::bind("0.0.0.0:53").await?;
        let redirect_domains: HashSet<String> = domains.iter().map(|d| d.to_string()).collect();
        let upstream = detect_upstream();
        Ok(Self { socket, redirect_domains, target_ip, upstream })
    }

    pub async fn serve(self) -> Result<(), std::io::Error> {
        let mut buf = [0u8; 512];
        loop {
            let (len, src) = self.socket.recv_from(&mut buf).await?;

            let request = match Message::from_bytes(&buf[..len]) {
                Ok(m) => m,
                Err(_) => continue,
            };

            let query = match request.queries.first() {
                Some(q) => q,
                None => continue,
            };
            let name = query.name().to_utf8();
            let name = name.trim_end_matches('.');

            let should_redirect = self.redirect_domains.iter()
                .any(|d| name == d || name.ends_with(&format!(".{}", d)));

            if should_redirect && query.query_type() == RecordType::A {
                let mut response = Message::new(request.id, MessageType::Response, OpCode::Query);
                response.queries.push(query.clone());
                response.metadata.authoritative = true;
                response.metadata.recursion_available = true;
                response.metadata.response_code = ResponseCode::NoError;
                let a_rdata = RData::A(A(self.target_ip));
                let record = Record::from_rdata(query.name().clone(), 0, a_rdata);
                response.answers.push(record);
                if let Ok(resp_bytes) = response.to_bytes() {
                    let _ = self.socket.send_to(&resp_bytes, src).await;
                }
            } else {
                let upstream_sock = match std::net::UdpSocket::bind("0.0.0.0:0") {
                    Ok(s) => {
                        s.set_read_timeout(Some(std::time::Duration::from_secs(3))).ok();
                        s
                    }
                    Err(_) => continue,
                };
                if upstream_sock.send_to(&buf[..len], self.upstream).is_err() {
                    continue;
                }
                let mut resp_buf = [0u8; 512];
                if let Ok((rlen, _)) = upstream_sock.recv_from(&mut resp_buf) {
                    let _ = self.socket.send_to(&resp_buf[..rlen], src).await;
                }
            }
        }
    }
}
