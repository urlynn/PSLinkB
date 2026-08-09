//! WinDivert 网络层拦截

#![cfg(windows)]

use libloading::{Library, Symbol};
use std::collections::HashMap;
use std::ffi::CString;
use std::net::Ipv4Addr;
use std::os::raw::{c_char, c_int, c_void};
use std::sync::{Arc, Mutex, OnceLock};
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
    flags: u64,
    data: [u8; 64],
}

impl WindivertAddr {
    fn new() -> Self {
        Self { timestamp: 0, flags: 0, data: [0; 64] }
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
    let mut addr = WindivertAddr::new();
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

// ── 1935/6667 端口重定向 ──

fn read_ipv4_src(packet: &[u8]) -> Option<Ipv4Addr> {
    if packet.len() < 20 { return None; }
    Some(Ipv4Addr::new(packet[12], packet[13], packet[14], packet[15]))
}

fn read_ipv4_dst(packet: &[u8]) -> Option<Ipv4Addr> {
    if packet.len() < 20 { return None; }
    Some(Ipv4Addr::new(packet[16], packet[17], packet[18], packet[19]))
}

fn rewrite_ipv4_src(packet: &mut [u8], new_ip: Ipv4Addr) -> bool {
    if packet.len() < 20 { return false; }
    let bytes = new_ip.octets();
    packet[12..16].copy_from_slice(&bytes);
    true
}

fn rewrite_ipv4_dst(packet: &mut [u8], new_ip: Ipv4Addr) -> bool {
    if packet.len() < 20 { return false; }
    let bytes = new_ip.octets();
    packet[16..20].copy_from_slice(&bytes);
    true
}

fn read_tcp_src_port(packet: &[u8]) -> Option<u16> {
    if packet.is_empty() { return None; }
    let ihl = (packet[0] & 0x0F) as usize * 4;
    if packet.len() < ihl + 4 { return None; }
    Some(u16::from_be_bytes([packet[ihl], packet[ihl + 1]]))
}

fn read_tcp_dst_port(packet: &[u8]) -> Option<u16> {
    if packet.is_empty() { return None; }
    let ihl = (packet[0] & 0x0F) as usize * 4;
    if packet.len() < ihl + 4 { return None; }
    Some(u16::from_be_bytes([packet[ihl + 2], packet[ihl + 3]]))
}

fn port_redirect_loop(
    handle: *mut c_void,
    local_ip: Ipv4Addr,
    port: u16,
    map: Arc<Mutex<HashMap<u16, Ipv4Addr>>>,
    learned_shared: Arc<Mutex<Option<Ipv4Addr>>>,
) {
    let mut buf = [0u8; 65535];
    let mut last_seen = std::time::Instant::now();
    loop {
        match unsafe { wd_recv(handle, &mut buf) } {
            Ok((len, addr)) => {
                let packet = &mut buf[..len];
                if last_seen.elapsed() > std::time::Duration::from_secs(60) {
                    *learned_shared.lock().unwrap() = None;
                }
                last_seen = std::time::Instant::now();

                let src_port = read_tcp_src_port(packet);
                let dst_port = read_tcp_dst_port(packet);
                let src_ip = read_ipv4_src(packet);
                let dst_ip = read_ipv4_dst(packet);

                if let (Some(dp), Some(dip)) = (dst_port, dst_ip) {
                    if dp == port {
                        let mut learned = learned_shared.lock().unwrap();
                        match *learned {
                            None => {
                                *learned = Some(dip);
                                if let Some(sp) = src_port {
                                    map.lock().unwrap().insert(sp, dip);
                                }
                                if rewrite_ipv4_dst(packet, local_ip) {
                                    unsafe { wd_checksums(packet, &addr) };
                                }
                            }
                            Some(l) if l == dip => {
                                if let Some(sp) = src_port {
                                    map.lock().unwrap().insert(sp, dip);
                                }
                                if rewrite_ipv4_dst(packet, local_ip) {
                                    unsafe { wd_checksums(packet, &addr) };
                                }
                            }
                            _ => {}
                        }
                    }
                }

                if let (Some(sp), Some(sip)) = (src_port, src_ip) {
                    if sp == port && sip == local_ip {
                        if let Some(dp) = dst_port {
                            let orig_ip = map.lock().unwrap().get(&dp).copied();
                            if let Some(ip) = orig_ip {
                                if rewrite_ipv4_src(packet, ip) {
                                    unsafe { wd_checksums(packet, &addr) };
                                }
                            }
                        }
                    }
                }

                if let Err(e) = unsafe { wd_send(handle, packet, &addr) } {
                    log_err(&format!("[ERROR] WinDivert helper error: {}", e));
                }
            }
            Err(_) => break,
        }
    }
}

// ── 1935 端口重定向 ──

static HANDLE_1935: Mutex<Option<usize>> = Mutex::new(None);
static THREAD_1935: Mutex<Option<JoinHandle<()>>> = Mutex::new(None);
static LEARNED_1935: OnceLock<Arc<Mutex<Option<Ipv4Addr>>>> = OnceLock::new();

fn learned_1935() -> Arc<Mutex<Option<Ipv4Addr>>> {
    LEARNED_1935.get_or_init(|| Arc::new(Mutex::new(None))).clone()
}

/// 启动 1935 端口重定向
pub fn start_1935_intercept(local_ip: Ipv4Addr) -> Result<(), String> {
    let filter = "tcp and (tcp.DstPort == 1935 or tcp.SrcPort == 1935)";
    let handle = unsafe { wd_open(filter)? };
    let map = Arc::new(Mutex::new(HashMap::<u16, Ipv4Addr>::new()));

    *HANDLE_1935.lock().unwrap() = Some(handle as usize);

    let handle_usize = handle as usize;
    let th = thread::spawn(move || {
        let handle = handle_usize as *mut c_void;
        port_redirect_loop(handle, local_ip, 1935, map, learned_1935());
    });
    *THREAD_1935.lock().unwrap() = Some(th);
    Ok(())
}

/// 卸载 1935 过滤器
pub fn shutdown_1935_intercept() {
    let handle_opt = HANDLE_1935.lock().unwrap().take();
    if let Some(handle_usize) = handle_opt {
        let handle = handle_usize as *mut c_void;
        unsafe { wd_shutdown(handle) };
        drop(THREAD_1935.lock().unwrap().take());
        unsafe { wd_close(handle) };
    }
}

// ── 6667 端口重定向 ──

static HANDLE_6667: Mutex<Option<usize>> = Mutex::new(None);
static THREAD_6667: Mutex<Option<JoinHandle<()>>> = Mutex::new(None);
static LEARNED_6667: OnceLock<Arc<Mutex<Option<Ipv4Addr>>>> = OnceLock::new();

fn learned_6667() -> Arc<Mutex<Option<Ipv4Addr>>> {
    LEARNED_6667.get_or_init(|| Arc::new(Mutex::new(None))).clone()
}

/// 启动 6667 端口重定向
pub fn start_6667_intercept(local_ip: Ipv4Addr) -> Result<(), String> {
    let filter = "tcp and (tcp.DstPort == 6667 or tcp.SrcPort == 6667)";
    let handle = unsafe { wd_open(filter)? };
    let map = Arc::new(Mutex::new(HashMap::<u16, Ipv4Addr>::new()));

    *HANDLE_6667.lock().unwrap() = Some(handle as usize);

    let handle_usize = handle as usize;
    let th = thread::spawn(move || {
        let handle = handle_usize as *mut c_void;
        port_redirect_loop(handle, local_ip, 6667, map, learned_6667());
    });
    *THREAD_6667.lock().unwrap() = Some(th);
    Ok(())
}

/// 卸载 6667 过滤器
pub fn shutdown_6667_intercept() {
    let handle_opt = HANDLE_6667.lock().unwrap().take();
    if let Some(handle_usize) = handle_opt {
        let handle = handle_usize as *mut c_void;
        unsafe { wd_shutdown(handle) };
        drop(THREAD_6667.lock().unwrap().take());
        unsafe { wd_close(handle) };
    }
}

// ── 主程序侧 ──
pub fn start(local_ip: Ipv4Addr) -> Result<(), String> {
    let exe = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .parent()
        .ok_or("Cannot get exe directory")?
        .join("pslinkb-windivert.exe");
    if !exe.exists() {
        return Err(format!("pslinkb-windivert.exe not found: {}", exe.display()));
    }

    let exe_str = exe.to_str().ok_or("Exe path is not valid UTF-8")?;
    eprintln!("[INFO] 正在启动 pslinkb-windivert.exe - 请在 UAC 弹窗同意管理员请求");
    shell_execute_runas(exe_str, &local_ip.to_string())?;

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
