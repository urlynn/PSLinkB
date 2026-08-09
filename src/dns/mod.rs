//! DNS 重定向检测模块

#[cfg(all(windows, feature = "dns-redirect", not(feature = "windivert")))]
compile_error!(
    "On Windows, enable the `windivert` feature instead of `dns-redirect` \
     (it provides WinDivert DNS interception + the helper binary)."
);

mod check;

pub use check::{REDIRECT_DOMAINS, CHECK_DOMAINS, PROXY_DOMAINS, write_dns_status};

#[cfg(feature = "dns-redirect")]
pub use check::{CheckResult, resolve, system_resolve, check_domain, summarize};

#[cfg(all(feature = "openwrt", not(feature = "dns-redirect")))]
pub use check::resolve_one;

#[cfg(feature = "dns-redirect")]
pub mod desktop;
#[cfg(feature = "dns-redirect")]
pub use desktop::{proxy::DnsProxy, setup::auto_start};

#[cfg(feature = "dns-redirect")]
pub mod relay;

#[cfg(all(feature = "dns-redirect", windows))]
pub mod windows;

#[cfg(feature = "openwrt")]
mod openwrt;
#[cfg(feature = "openwrt")]
pub use openwrt::redirect;
