/// DNS redirect — OpenWRT DNS 重定向 / SIGHUP 重载 / 清理

#[cfg(feature = "openwrt")]
pub async fn init(domains: &[&str], target_ip: &str, config: &crate::config::Config) {
    if !config.dns_redirect { return; }

    // 写 checking 状态
    let first_actual = crate::dns::resolve_one(domains[0]).await.unwrap_or_default();
    crate::dns::write_dns_status(true, true, target_ip, &first_actual, false);

    // 配置 dnsmasq 重定向
    if let Some(_confdir) = super::dnsmasq::enable_redirect(domains, target_ip) {
        let mut all_ok = true;
        let mut last_actual = String::new();
        for domain in domains {
            let actual = crate::dns::resolve_one(domain).await.unwrap_or_default();
            if actual != target_ip { all_ok = false; }
            last_actual = actual;
        }
        crate::dns::write_dns_status(false, true, target_ip, &last_actual, all_ok);
    } else {
        crate::dns::write_dns_status(false, true, target_ip, &first_actual, false);
    }
}

/// SIGHUP 重载 DNS 配置
#[cfg(feature = "openwrt")]
pub async fn handle_sighup(local_ip: &str) {
    eprintln!("[INFO] SIGHUP received - Toggling DNS...");
    let enabled = std::fs::read_to_string("/tmp/pslinkb/dns_status")
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|v| v.get("enabled")?.as_bool())
        .map(|b| !b)
        .unwrap_or(true);

    let domains: Vec<&str> = crate::dns::REDIRECT_DOMAINS.to_vec();
    let confdir = super::dnsmasq::ensure_confdir();

    if enabled {
        let first_actual = crate::dns::resolve_one(domains[0]).await.unwrap_or_default();
        crate::dns::write_dns_status(true, true, local_ip, &first_actual, false);
        if let Some(ref _cd) = confdir {
            super::dnsmasq::enable_redirect(&domains, local_ip);
            let mut all_ok = true;
            let mut last_actual = String::new();
            for domain in &domains {
                let actual = crate::dns::resolve_one(domain).await.unwrap_or_default();
                if actual != local_ip { all_ok = false; }
                last_actual = actual;
            }
            crate::dns::write_dns_status(false, true, local_ip, &last_actual, all_ok);
        } else {
            crate::dns::write_dns_status(false, true, local_ip, "", false);
        }
    } else {
        crate::dns::write_dns_status(true, false, "", "", false);
        if let Some(ref cd) = confdir {
            super::dnsmasq::disable_redirect(cd);
        }
        crate::dns::write_dns_status(false, false, "", "", false);
    }
}

/// 关闭时清理
#[cfg(feature = "openwrt")]
pub fn cleanup(config: &crate::config::Config) {
    if !config.dns_redirect { return; }
    crate::dns::write_dns_status(true, false, "", "", false);
    if let Some(cd) = super::dnsmasq::ensure_confdir() {
        super::dnsmasq::disable_redirect(&cd);
    }
    crate::dns::write_dns_status(false, false, "", "", false);
}
