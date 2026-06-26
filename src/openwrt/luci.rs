/// 文件 IPC：/tmp/pslinkb/ 与 LuCI 通信

#[cfg(feature = "openwrt")]
mod imp {
    const TMP_DIR: &str = "/tmp/pslinkb";

    pub fn init() {
        let _ = std::fs::create_dir_all(TMP_DIR);
        reset();
    }

    pub fn set(key: &str, value: &str) {
        let tmp = format!("{}/.{}.tmp", TMP_DIR, key);
        let dst = format!("{}/{}", TMP_DIR, key);
        if std::fs::write(&tmp, value).is_ok() {
            let _ = std::fs::rename(&tmp, &dst);
        }
    }

    pub fn read(key: &str) -> Option<String> {
        std::fs::read_to_string(format!("{}/{}", TMP_DIR, key)).ok()
    }

    pub fn clear(key: &str) {
        let _ = std::fs::remove_file(format!("{}/{}", TMP_DIR, key));
    }

    /// LuCI 写入 "1" 触发扫码，rust 处理完后 clear
    pub fn has_command(key: &str) -> bool {
        if read(key).as_deref() == Some("1") { clear(key); true } else { false }
    }

    /// 关播清理
    /// qr_status 不清 - 登录完成后 Rust 退出 respawn, 保留 done 让 LuCI 读到跳转
    pub fn reset() {
        for key in &["user", "rtmp", "stream", "qr_url", "error"] {
            clear(key);
        }
    }
}

#[cfg(feature = "openwrt")]
pub use imp::*;

#[cfg(feature = "cli")]
mod stub {
    pub fn init() {}
    pub fn set(_key: &str, _value: &str) {}
    pub fn read(_key: &str) -> Option<String> { None }
    pub fn clear(_key: &str) {}
    pub fn has_command(_key: &str) -> bool { false }
    pub fn reset() {}
}

#[cfg(feature = "cli")]
pub use stub::*;
