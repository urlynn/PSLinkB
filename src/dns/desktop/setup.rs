//! 桌面端 DNS 自动检测 / 代理启动

use crate::dns::{CHECK_DOMAINS, REDIRECT_DOMAINS, check_domain, resolve, summarize, system_resolve};
use crate::log;
use std::net::Ipv4Addr;

pub async fn auto_start(config_dns_proxy: bool, local_ip: &str) {
    if !config_dns_proxy {
        return;
    }

    let results = check_domain(CHECK_DOMAINS, local_ip, system_resolve).await;
    if results.iter().all(|r| r.success) {
        summarize(&[]);
        return;
    }

    eprintln!("\r\x1b[K[System] 域名重定向未配置 - 启用内置 DNS 代理");
    log!(alert, "[WARN] 请确保 PS5 的首选 DNS 设为本机 IP: {}", local_ip);
    eprintln!("[System] 如需禁用 DNS 代理 - 请在 pslinkb.toml 设置 dns_proxy = false");

    let proxy = crate::dns::DnsProxy::new(
        REDIRECT_DOMAINS,
        local_ip.parse().unwrap_or(Ipv4Addr::new(127, 0, 0, 1)),
    ).await;

    match proxy {
        Ok(p) => {
            tokio::spawn(async move {
                if let Err(e) = p.serve().await {
                    log!(error, "DNS Proxy: {}", crate::core::error::AppError::crash("DNS", e.to_string()));
                }
            });
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            let results = check_domain(CHECK_DOMAINS, local_ip, resolve).await;
            summarize(&results);
        }
        Err(e) => {
            log!(error, "DNS Proxy: 端口 53 不可用 - {}", e);
            std::process::exit(1);
        }
    }
}
