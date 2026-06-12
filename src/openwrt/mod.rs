/// OpenWrt 集成：Luci 文件 IPC + ubus 服务注册（experimental）

pub mod luci;
#[cfg(feature = "openwrt")]
pub mod ubus;
