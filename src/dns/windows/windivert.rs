//! WinDivert 网络层拦截

#![cfg(windows)]

use libloading::{Library, Symbol};
use std::ffi::CString;
use std::net::Ipv4Addr;
use std::os::raw::{c_char, c_int, c_void};
use std::sync::{Mutex, OnceLock};
use std::thread::{self, JoinHandle};
use tokio::sync::mpsc;

// ── 常量 ──

const LAYER_NETWORK: c_int = 0;
const SHUTDOWN_RECV: u64 = 0x1;

/// 命名管道名
pub const PIPE_NAME: &str = r"\\.\pipe\pslinkb-windivert";

// ── log 回调 ──

static LOG_FN: OnceLock<fn(&str)> = OnceLock::new();

pub fn set_logger(f: fn(&str)) {
    let _ = LOG_FN.set(f);
}

// ── 命名管道裸传发送 ────
static PIPE_TX: OnceLock<mpsc::UnboundedSender<String>> = OnceLock::new();

/// 设置管道发送端
pub fn set_pipe_tx(tx: mpsc::UnboundedSender<String>) {
    let _ = PIPE_TX.set(tx);
}

/// 日志裸传
pub fn pipe_send(line: impl Into<String>) {
    if let Some(tx) = PIPE_TX.get() {
        let _ = tx.send(line.into());
    }
}

fn log_err(msg: &str) {
    if let Some(f) = LOG_FN.get() {
        f(msg);
    } else {
        eprintln!("{}", msg);
    }
}

// ── FFI 类型 ──

type FnOpen = unsafe extern "C" fn(*const c_char, c_int, i16, u64) -> *mut c_void;
type FnRecv = unsafe extern "C" fn(*mut c_void, *mut u8, u32, *mut u32, *mut WindivertAddr) -> c_int;
type FnSend = unsafe extern "C" fn(*mut c_void, *const u8, u32, *mut u32, *const WindivertAddr) -> c_int;
type FnShutdown = unsafe extern "C" fn(*mut c_void, u64) -> c_int;
type FnClose = unsafe extern "C" fn(*mut c_void) -> c_int;
type FnChecksums = unsafe extern "C" fn(*mut u8, u32, *const WindivertAddr, u64) -> u64;

/// WINDIVERT_ADDRESS
#[repr(C)]
struct WindivertAddr {
    timestamp: i64,
    flags: u32,
    reserved2: u32,
    data: [u8; 64],
}

impl WindivertAddr {
    /// bit 17 = Outbound
    fn outbound(&self) -> bool {
        (self.flags >> 17) & 1 == 1
    }
}

// ── DLL 加载 ──

static DLL: OnceLock<Library> = OnceLock::new();

fn dll() -> &'static Library {
    DLL.get_or_init(|| unsafe {
        Library::new("WinDivert.dll").expect("WinDivert.dll 未找到")
    })
}

/// 验证
pub fn check_dll() -> Result<(), String> {
    unsafe { Library::new("WinDivert.dll") }.map_err(|e| format!("WinDivert.dll 加载失败: {}", e))?;
    Ok(())
}

// ── 底层 API ──

unsafe fn wd_open(filter: &str) -> Result<*mut c_void, String> {
    let c_filter = CString::new(filter).map_err(|e| e.to_string())?;
    let func: Symbol<FnOpen> = unsafe { dll().get(b"WinDivertOpen\0") }.map_err(|e| e.to_string())?;
    let handle = unsafe { func(c_filter.as_ptr(), LAYER_NETWORK, 0, 0) };
    if handle.is_null() {
        return Err("WinDivertOpen 失败(需管理员权限?)".into());
    }
    Ok(handle)
}

unsafe fn wd_recv(handle: *mut c_void, buf: &mut [u8]) -> Result<(usize, WindivertAddr), String> {
    let func: Symbol<FnRecv> = unsafe { dll().get(b"WinDivertRecv\0") }.map_err(|e| e.to_string())?;
    let mut addr = WindivertAddr { timestamp: 0, flags: 0, reserved2: 0, data: [0; 64] };
    let mut recv_len: u32 = 0;
    let ok = unsafe { func(handle, buf.as_mut_ptr(), buf.len() as u32, &mut recv_len, &mut addr) };
    if ok == 0 {
        return Err("WinDivertRecv 失败".into());
    }
    Ok((recv_len as usize, addr))
}

unsafe fn wd_send(handle: *mut c_void, packet: &[u8], addr: &WindivertAddr) -> Result<(), String> {
    let func: Symbol<FnSend> = unsafe { dll().get(b"WinDivertSend\0") }.map_err(|e| e.to_string())?;
    let mut send_len: u32 = 0;
    let ok = unsafe { func(handle, packet.as_ptr(), packet.len() as u32, &mut send_len, addr) };
    if ok == 0 {
        return Err("WinDivertSend 失败".into());
    }
    Ok(())
}

unsafe fn wd_shutdown(handle: *mut c_void) {
    let func: Symbol<FnShutdown> = unsafe { dll().get(b"WinDivertShutdown\0") }.unwrap();
    unsafe { func(handle, SHUTDOWN_RECV) };
}

unsafe fn wd_close(handle: *mut c_void) {
    let func: Symbol<FnClose> = unsafe { dll().get(b"WinDivertClose\0") }.unwrap();
    unsafe { func(handle) };
}

unsafe fn wd_checksums(packet: &mut [u8], addr: &WindivertAddr) {
    let func: Symbol<FnChecksums> = unsafe { dll().get(b"WinDivertHelperCalcChecksums\0") }.unwrap();
    unsafe { func(packet.as_mut_ptr(), packet.len() as u32, addr, 0) };
}

// ── DNS 53 ──
fn rewrite_udp_dst_port(packet: &mut [u8], new_port: u16) -> bool {
    if packet.len() < 28 {
        return false;
    }
    let ihl = (packet[0] & 0x0F) as usize * 4;
    if packet.len() < ihl + 8 || packet[9] != 17 {
        return false;
    }
    packet[ihl + 2] = (new_port >> 8) as u8;
    packet[ihl + 3] = (new_port & 0xFF) as u8;
    true
}

/// 修改 UDP SrcPort
fn rewrite_udp_src_port(packet: &mut [u8], new_port: u16) -> bool {
    if packet.len() < 28 {
        return false;
    }
    let ihl = (packet[0] & 0x0F) as usize * 4;
    if packet.len() < ihl + 8 || packet[9] != 17 {
        return false;
    }
    packet[ihl] = (new_port >> 8) as u8;
    packet[ihl + 1] = (new_port & 0xFF) as u8;
    true
}

// ── 443 引流 ──
fn rewrite_tcp_dst_port(packet: &mut [u8], new_port: u16) -> bool {
    if packet.len() < 40 {
        return false;
    }
    let ihl = (packet[0] & 0x0F) as usize * 4;
    if packet.len() < ihl + 4 || packet[9] != 6 {
        return false;
    }
    packet[ihl + 2] = (new_port >> 8) as u8;
    packet[ihl + 3] = (new_port & 0xFF) as u8;
    true
}

/// 修改 TCP SrcPort
fn rewrite_tcp_src_port(packet: &mut [u8], new_port: u16) -> bool {
    if packet.len() < 40 {
        return false;
    }
    let ihl = (packet[0] & 0x0F) as usize * 4;
    if packet.len() < ihl + 4 || packet[9] != 6 {
        return false;
    }
    packet[ihl] = (new_port >> 8) as u8;
    packet[ihl + 1] = (new_port & 0xFF) as u8;
    true
}

// ── DNS 53 拦截 ──

static HANDLE_53: OnceLock<usize> = OnceLock::new();
static THREAD_53: Mutex<Option<JoinHandle<()>>> = Mutex::new(None);

/// 启动 DNS 53 拦截
pub fn start_intercept(dns_port: u16) -> Result<(), String> {
    let filter = format!(
        "(inbound and ip and udp.DstPort == 53) or (outbound and ip and udp.SrcPort == {})",
        dns_port
    );
    let handle = unsafe { wd_open(&filter)? };
    HANDLE_53.set(handle as usize).ok();

    let handle_usize = handle as usize;
    let th = thread::spawn(move || {
        let handle = handle_usize as *mut c_void;
        let mut buf = [0u8; 65535];
        loop {
            match unsafe { wd_recv(handle, &mut buf) } {
                Ok((len, addr)) => {
                    let packet = &mut buf[..len];
                    if addr.outbound() {
                        if rewrite_udp_src_port(packet, 53) {
                            unsafe { wd_checksums(packet, &addr) };
                        }
                    } else {
                        if rewrite_udp_dst_port(packet, dns_port) {
                            unsafe { wd_checksums(packet, &addr) };
                        }
                    }
                    if let Err(e) = unsafe { wd_send(handle, packet, &addr) } {
                        log_err(&format!("[ERROR] WinDivert helper error: {}", e));
                    }
                }
                Err(_) => break,
            }
        }
    });
    *THREAD_53.lock().unwrap() = Some(th);

    Ok(())
}

/// 卸载 53 过滤器
pub fn shutdown_intercept() {
    let handle = HANDLE_53.get().copied().unwrap_or(0) as *mut c_void;
    if handle.is_null() {
        return;
    }
    unsafe { wd_shutdown(handle) };
    drop(THREAD_53.lock().unwrap().take());
    unsafe { wd_close(handle) };
}

// ── 443 引流拦截 ──

static HANDLE_443: Mutex<Option<usize>> = Mutex::new(None);
static THREAD_443: Mutex<Option<JoinHandle<()>>> = Mutex::new(None);

/// 启动 443 引流拦截
pub fn start_443_intercept(ps5_ip: Ipv4Addr, relay_port: u16) -> Result<(), String> {
    let filter = format!(
        "(inbound and ip and ip.SrcAddr == {} and tcp.DstPort == 443) or (outbound and ip and ip.DstAddr == {} and tcp.SrcPort == {})",
        ps5_ip, ps5_ip, relay_port
    );
    let handle = unsafe { wd_open(&filter)? };

    let mut h = HANDLE_443.lock().unwrap();
    if h.is_some() {
        return Err("443 拦截已在运行".into());
    }
    *h = Some(handle as usize);

    let handle_usize = handle as usize;
    let th = thread::spawn(move || {
        let handle = handle_usize as *mut c_void;
        let mut buf = [0u8; 65535];
        loop {
            match unsafe { wd_recv(handle, &mut buf) } {
                Ok((len, addr)) => {
                    let packet = &mut buf[..len];
                    if addr.outbound() {
                        if rewrite_tcp_src_port(packet, 443) {
                            unsafe { wd_checksums(packet, &addr) };
                        }
                    } else {
                        if rewrite_tcp_dst_port(packet, relay_port) {
                            unsafe { wd_checksums(packet, &addr) };
                        }
                    }
                    if let Err(e) = unsafe { wd_send(handle, packet, &addr) } {
                        log_err(&format!("[ERROR] WinDivert helper error: {}", e));
                    }
                }
                Err(_) => break,
            }
        }
    });
    *THREAD_443.lock().unwrap() = Some(th);

    Ok(())
}

/// 卸载 443 过滤器
pub fn shutdown_443_intercept() {
    let handle_opt = HANDLE_443.lock().unwrap().take();
    if let Some(handle_usize) = handle_opt {
        let handle = handle_usize as *mut c_void;
        unsafe { wd_shutdown(handle) };
        drop(THREAD_443.lock().unwrap().take());
        unsafe { wd_close(handle) };
    }
}

// ── 主程序侧 ──
pub fn start(
    proxy_url: Option<&str>,
    upstream: Option<&str>,
) -> Result<(), String> {
    let exe = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .parent()
        .ok_or("Cannot get exe directory")?
        .join("pslinkb-windivert.exe");
    if !exe.exists() {
        return Err(format!("pslinkb-windivert.exe not found: {}", exe.display()));
    }

    let exe_str = exe.to_str().ok_or("Exe path is not valid UTF-8")?;
    let proxy_url_str = proxy_url.unwrap_or("none");
    let upstream_str = upstream.unwrap_or("none");
    let params = format!(
        "{} {}",
        proxy_url_str, upstream_str
    );
    eprintln!("[INFO] 正在启动 pslinkb-windivert.exe - 请在 UAC 弹窗同意管理员请求");
    shell_execute_runas(exe_str, &params)?;

    Ok(())
}

/// 主动终止辅助进程
pub fn shutdown() {
    let _ = std::process::Command::new("taskkill")
        .args(["/IM", "pslinkb-windivert.exe", "/F"])
        .output();
}

fn shell_execute_runas(exe: &str, params: &str) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;

    unsafe extern "system" {
        fn ShellExecuteW(
            hwnd: *mut u8,
            op: *const u16,
            file: *const u16,
            params: *const u16,
            dir: *const u16,
            show: i32,
        ) -> isize;
    }

    fn to_wide(s: &str) -> Vec<u16> {
        std::ffi::OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
    }

    let op = to_wide("runas");
    let file = to_wide(exe);
    let params = to_wide(params);
    let dir = to_wide("");

    unsafe {
        let result = ShellExecuteW(
            std::ptr::null_mut(),
            op.as_ptr(),
            file.as_ptr(),
            params.as_ptr(),
            dir.as_ptr(),
            0, // SW_HIDE
        );
        if result <= 32 {
            return Err("ShellExecute 失败(用户拒绝 UAC?)".into());
        }
    }
    Ok(())
}
