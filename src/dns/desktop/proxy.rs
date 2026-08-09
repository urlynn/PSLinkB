//! DNS 代理

use std::collections::HashSet;
use std::net::{Ipv4Addr, SocketAddr};
use tokio::net::UdpSocket;
use tokio::sync::watch;

use hickory_proto::op::{Message, MessageType, OpCode, Query, ResponseCode};
use hickory_proto::rr::{Name, RData, Record, RecordType};
use hickory_proto::rr::rdata::A;
use hickory_proto::serialize::binary::{BinDecodable, BinEncodable};

pub struct DnsProxy {
    socket: UdpSocket,
    redirect_domains: HashSet<String>,
    target_ip: Ipv4Addr,
    upstream: SocketAddr,
    ps5_tx: Option<watch::Sender<Option<Ipv4Addr>>>,
}

pub fn detect_upstream() -> SocketAddr {    #[cfg(target_os = "windows")]
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
                if line.starts_with("nameserver")
                    && let Some(ip) = line.split_whitespace().nth(1)
                    && let Ok(addr) = ip.parse::<Ipv4Addr>()
                {
                    return SocketAddr::new(std::net::IpAddr::V4(addr), 53);
                }
            }
        }
    }
    SocketAddr::new(std::net::IpAddr::V4(Ipv4Addr::new(114, 114, 114, 114)), 53)
}

impl DnsProxy {
    pub async fn probe_upstream(upstream: SocketAddr) -> bool {
        let socket = match UdpSocket::bind("0.0.0.0:0").await {
            Ok(s) => s,
            Err(_) => return false,
        };
        let name = match Name::from_utf8("jd.com") {
            Ok(n) => n,
            Err(_) => return false,
        };
        let mut query = Message::new(0x1234, MessageType::Query, OpCode::Query);
        query.add_query(Query::query(name, RecordType::A));
        let Ok(bytes) = query.to_bytes() else { return false };
        if socket.send_to(&bytes, upstream).await.is_err() {
            return false;
        }
        let mut buf = [0u8; 512];
        tokio::time::timeout(
            std::time::Duration::from_secs(3),
            socket.recv_from(&mut buf),
        ).await.map(|r| r.is_ok()).unwrap_or(false)
    }

    async fn bind(
        domains: &[&str],
        upstream: Option<SocketAddr>,
    ) -> Result<(UdpSocket, HashSet<String>, SocketAddr), std::io::Error> {
        let bind_addr = "0.0.0.0:53";
        let socket = UdpSocket::bind(bind_addr).await?;
        let redirect_domains: HashSet<String> = domains.iter().map(|d| d.to_string()).collect();
        let upstream = upstream.unwrap_or_else(detect_upstream);
        Ok((socket, redirect_domains, upstream))
    }

    pub async fn new(
        domains: &[&str],
        target_ip: Ipv4Addr,
        upstream: Option<SocketAddr>,
        ps5_tx: Option<watch::Sender<Option<Ipv4Addr>>>,
    ) -> Result<Self, std::io::Error> {
        let (socket, redirect_domains, upstream) = Self::bind(domains, upstream).await?;
        Ok(Self {
            socket,
            redirect_domains,
            target_ip,
            upstream,
            ps5_tx,
        })
    }

    pub async fn serve(self) -> Result<(), std::io::Error> {
        let mut buf = [0u8; 512];
        loop {
            let (len, src) = match self.socket.recv_from(&mut buf).await {
                Ok(r) => r,
                Err(e) if e.kind() == std::io::ErrorKind::ConnectionReset => continue,
                Err(e) => return Err(e),
            };

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
            let qtype = query.query_type();

            if let Some(tx) = &self.ps5_tx {
                if name == "playstation.net" || name.ends_with(".playstation.net")
                    || name == "playstation.com" || name.ends_with(".playstation.com")
                {
                    if let std::net::IpAddr::V4(v4) = src.ip() {
                        let _ = tx.send(Some(v4));
                    }
                }
            }

            let should_redirect = self.redirect_domains.iter()
                .any(|d| name == d || name.ends_with(&format!(".{}", d)));

            if should_redirect && qtype == RecordType::A {
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
                let upstream_sock = match UdpSocket::bind("0.0.0.0:0").await {
                    Ok(s) => s,
                    Err(_) => continue,
                };
                if upstream_sock.send_to(&buf[..len], self.upstream).await.is_err() {
                    continue;
                }
                let mut resp_buf = [0u8; 512];
                if let Ok(Ok((rlen, _))) = tokio::time::timeout(
                    std::time::Duration::from_secs(3),
                    upstream_sock.recv_from(&mut resp_buf),
                ).await {
                    let _ = self.socket.send_to(&resp_buf[..rlen], src).await;
                }
            }
        }
    }
}
