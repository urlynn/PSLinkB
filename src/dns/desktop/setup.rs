//! 桌面端 DNS 代理 / Windivert 附加

use crate::dns::{CHECK_DOMAINS, REDIRECT_DOMAINS, PROXY_DOMAINS, check_domain, resolve, system_resolve};
use crate::log;
use std::net::Ipv4Addr;
use tokio::sync::watch;

#[cfg(windows)]
fn pause_exit() {
    let _ = std::process::Command::new("cmd").args(["/C", "pause"]).status();
}

#[cfg(windows)]
pub async fn start_windivert(local_ip: std::net::Ipv4Addr) -> tokio::sync::oneshot::Receiver<()> {
    use tokio::io::{AsyncBufReadExt, BufReader};
    use tokio::net::windows::named_pipe::ClientOptions;

    let (done_tx, done_rx) = tokio::sync::oneshot::channel::<()>();

    if let Err(e) = crate::dns::windows::windivert::start(local_ip) {
        log!(error, "WinDivert 启动失败: {}", e);
        pause_exit();
        std::process::exit(1);
    }
    eprintln!("[INFO] WinDivert 端口重定向已启动 - 日志路径: pslinkb-windivert.log");

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

    tokio::spawn(async move {
        let mut reader = BufReader::new(pipe_client);
        let mut line = String::new();
        let mut done_tx = Some(done_tx);
        loop {
            line.clear();
            match reader.read_line(&mut line).await {
                Ok(0) => break,
                Ok(_) => {
                    let trimmed = line.trim_end();
                    if trimmed == "[[INIT_DONE]]" {
                        if let Some(tx) = done_tx.take() {
                            let _ = tx.send(());
                        }
                    } else {
                        eprint!("{}", line);
                    }
                }
                Err(_) => break,
            }
        }
    });

    done_rx
}

#[cfg(not(windows))]
pub async fn start_windivert(_local_ip: std::net::Ipv4Addr) -> tokio::sync::oneshot::Receiver<()> {
    let (_, rx) = tokio::sync::oneshot::channel::<()>();
    rx
}

pub async fn auto_start(
    local_ip: &str,
    proxy_url: Option<&str>,
    dns: bool,
) -> bool {
    if !dns && proxy_url.is_none() {
        let results = check_domain(CHECK_DOMAINS, local_ip, system_resolve).await;
        if !results.iter().all(|r| r.success) {
            return true;
        }
        log!(ok, "[INFO] DNS Check - ✓ 域名已指向本机");
        return false;
    }

    let mut domains: Vec<&str> = REDIRECT_DOMAINS.to_vec();
    if proxy_url.is_some() {
        domains.extend_from_slice(PROXY_DOMAINS);
    }

    if dns && proxy_url.is_none() {
        eprintln!("[INFO] 启用内置 DNS 代理 - 0.0.0.0:53");
    }

    let mut upstream = crate::dns::desktop::proxy::detect_upstream();
    if dns && proxy_url.is_none()
        && !crate::dns::desktop::proxy::DnsProxy::probe_upstream(upstream).await
    {
        log!(warn, "上游 DNS {} 不可达 - 回退 223.5.5.5:53", upstream);
        upstream = "223.5.5.5:53".parse().unwrap();
    }

    let (ps5_tx, mut ps5_rx) = watch::channel::<Option<Ipv4Addr>>(None);

    let domains_owned: Vec<String> = domains.iter().map(|s| s.to_string()).collect();
    let local_ip_v4 = local_ip.parse().expect("Local IP must be a valid IPv4");

    tokio::spawn(async move {
        loop {
            let proxy = crate::dns::DnsProxy::new(
                &domains_owned.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
                local_ip_v4,
                Some(upstream),
                Some(ps5_tx.clone()),
            ).await;
            match proxy {
                Ok(p) => {
                    if let Err(e) = p.serve().await {
                        log!(warn, "DNS Proxy: {}", e);
                    }
                }
                Err(e) => {
                    log!(warn, "DNS Proxy: 端口 53 不可用 - {}", e);
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
    });

    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    if dns && proxy_url.is_none() {
        let results = check_domain(CHECK_DOMAINS, local_ip, resolve).await;
        if !results.iter().all(|r| r.success) {
            return true;
        }
        log!(ok, "[INFO] DNS Check - ✓ 重定向正常");
    }

    log!(warn, "请将 PS5 的首选 DNS 设为本机 IP: {} - 备用 DNS 为 0.0.0.0", local_ip);
    log!(warn, "若设置完成后此处未打印 PS5 连接日志 - 请根据 https://urlynn.xyz/post/2/#常见问题 自主排查");

    let local_ip_c = local_ip.to_string();
    tokio::spawn(async move {
        if ps5_rx.changed().await.is_ok()
            && let Some(ps5_ip) = *ps5_rx.borrow()
        {
            log!(ok, "[INFO] PS5: {} -> PSLinkB: {} - ✓ PS5已连接", ps5_ip, local_ip_c);
            log!(ok, "{}", crate::INIT_COMPLETE_MSG);
        }
    });

    true
}
