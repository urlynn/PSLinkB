//! 桌面端 DNS 自动检测 / 代理启动

use crate::dns::{CHECK_DOMAINS, check_domain, summarize, system_resolve};
#[cfg(not(windows))]
use crate::dns::{REDIRECT_DOMAINS, resolve};
use crate::log;
use std::net::SocketAddr;

#[cfg(windows)]
fn pause_exit() {
    let _ = std::process::Command::new("cmd").args(["/C", "pause"]).status();
}

pub async fn auto_start(
    config_dns_proxy: bool,
    local_ip: &str,
    upstream: Option<SocketAddr>,
    #[cfg(windows)]
    proxy_url: Option<&str>,
) -> bool {
    if !config_dns_proxy {
        return false;
    }

    let results = check_domain(CHECK_DOMAINS, local_ip, system_resolve).await;
    if results.iter().all(|r| r.success) {
        summarize(&[]);
        return false;
    }

    eprintln!("\r\x1b[K[System] 域名重定向未配置 - 启用内置 DNS 代理");
    eprintln!("[System] 如需禁用 DNS 代理 - 请在 pslinkb.toml 设置 dns_proxy = false");

    #[cfg(windows)]
    {
        use tokio::io::{AsyncBufReadExt, BufReader};
        use tokio::net::windows::named_pipe::ClientOptions;

        let upstream_str = upstream.map(|s| s.to_string());

        if let Err(e) = crate::dns::windows::windivert::start(
            proxy_url,
            upstream_str.as_deref(),
        ) {
            log!(error, "WinDivert 启动失败: {}", e);
            pause_exit();
            std::process::exit(1);
        }
        eprintln!("[INFO] WinDivert DNS 拦截已启动 - 日志路径: pslinkb-windivert.log");

        let pipe_name = crate::dns::windows::windivert::PIPE_NAME;
        let pipe_client = {
            let mut client = None;
            let mut last_err: Option<std::io::Error> = None;
            for _ in 0..20 {
                match ClientOptions::new().open(pipe_name) {
                    Ok(c) => { client = Some(c); break; }
                    Err(e) => {
                        let starting = e.raw_os_error() == Some(2); 
                        last_err = Some(e);
                        if !starting { break; } 
                        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                    }
                }
            }
            match client {
                Some(c) => c,
                None => {
                    match last_err {
                        Some(e) => log!(error, "连接 WinDivert 命名管道失败: {}", e),
                        None => log!(error, "连接 WinDivert 命名管道失败"),
                    }
                    pause_exit();
                    std::process::exit(1);
                }
            }
        };
        eprintln!("[INFO] 已连接 WinDivert 命名管道");

        let deferred = tokio::spawn(async move {
            let mut reader = BufReader::new(pipe_client);
            let mut line = String::new();
            loop {
                line.clear();
                match reader.read_line(&mut line).await {
                    Ok(0) => break,
                    Ok(_) => {
                        let trimmed = line.trim_end();
                        if trimmed == "[[INIT_DONE]]" {

                            log!(ok, "{}", crate::INIT_COMPLETE_MSG);
                        } else {
                            eprint!("{}", line);
                        }
                    }
                    Err(_) => break,
                }
            }
        });
        let _ = deferred; 

        return true;
    }

    #[cfg(not(windows))]
    {
        use std::net::Ipv4Addr;
        log!(alert, "[WARN] 请确保 PS5 的首选 DNS 设为本机 IP: {}", local_ip);
        let domains: Vec<&str> = REDIRECT_DOMAINS.to_vec();
        let proxy = crate::dns::DnsProxy::new(
            &domains,
            local_ip.parse().unwrap_or(Ipv4Addr::new(127, 0, 0, 1)),
            upstream,
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

        return false;
    }
}
