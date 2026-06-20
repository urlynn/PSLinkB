//! OpenWRT dnsmasq control

use std::process::Command;

pub fn ensure_confdir() -> Option<String> {
    let output = Command::new("uci")
        .args(["get", "dhcp.@dnsmasq[0].confdir"])
        .output()
        .ok()?;

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !stdout.is_empty() {
        return Some(stdout);
    }

    Command::new("uci")
        .args(["set", "dhcp.@dnsmasq[0].confdir=/etc/dnsmasq.d"])
        .output()
        .ok()?;
    Command::new("uci")
        .args(["commit", "dhcp"])
        .output()
        .ok()?;

    Some("/etc/dnsmasq.d".to_string())
}

fn write_conf(confdir: &str, domains: &[&str], target_ip: &str) -> bool {
    let conf_path = format!("{}/pslinkb.conf", confdir);
    let mut content = String::new();
    for domain in domains {
        content.push_str(&format!("address=/{}/{}\n", domain, target_ip));
    }
    std::fs::write(&conf_path, &content).is_ok()
}

fn restart_dnsmasq() -> bool {
    Command::new("/etc/init.d/dnsmasq")
        .arg("restart")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn enable_redirect(domains: &[&str], target_ip: &str) -> Option<String> {
    let confdir = ensure_confdir()?;
    if !write_conf(&confdir, domains, target_ip) { return None; }
    if !restart_dnsmasq() { return None; }
    Some(confdir)
}

pub fn disable_redirect(confdir: &str) {
    let conf_path = format!("{}/pslinkb.conf", confdir);
    let _ = std::fs::remove_file(&conf_path);
    restart_dnsmasq();
}
