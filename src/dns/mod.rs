//! DNS 重定向检测模块

mod check;

pub use check::{REDIRECT_DOMAINS, CHECK_DOMAINS, write_dns_status};

#[cfg(feature = "dns-redirect")]
pub use check::{CheckResult, resolve, system_resolve, check_domain, summarize};

#[cfg(all(feature = "openwrt", not(feature = "dns-redirect")))]
pub use check::resolve_one;

#[cfg(feature = "dns-redirect")]
mod desktop;
#[cfg(feature = "dns-redirect")]
pub use desktop::{proxy::DnsProxy, setup::auto_start};

#[cfg(feature = "openwrt")]
mod openwrt;
#[cfg(feature = "openwrt")]
pub use openwrt::redirect;
