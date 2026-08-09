//! pslinkb-windivert

#[cfg(windows)]
mod imp {
    use std::net::Ipv4Addr;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::windows::named_pipe::NamedPipeServer;
    use tokio::sync::mpsc;

    pub fn real_main() {
        let args: Vec<String> = std::env::args().collect();

        if args.iter().any(|a| a == "-v" || a == "--version") {
            match pslinkb::dns::windows::windivert::check_dll() {
                Ok(()) => {
                    println!("WinDivert.dll Load 成功");
                    std::process::exit(0);
                }
                Err(e) => {
                    eprintln!("[ERROR] {}", e);
                    std::process::exit(1);
                }
            }
        }

        init_log_file();

        let local_ip = args.get(1)
            .and_then(|s| s.parse::<Ipv4Addr>().ok())
            .unwrap_or_else(|| {
                log_impl("[ERROR] WinDivert helper error: missing local_ip argument");
                std::process::exit(1);
            });

        pslinkb::dns::windows::windivert::set_logger(log_impl);
        pslinkb::log::set_debug_enabled(true);
        pslinkb::log::set_override(log_impl);

        log_impl(&format!("[INFO] Start: local_ip={}", local_ip));

        let rt = match tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(e) => {
                log_impl(&format!("[ERROR] WinDivert helper error: {}", e));
                std::process::exit(1);
            }
        };

        rt.block_on(async move {
            run(local_ip).await;
        });
    }

    fn create_pipe_server(
        pipe_name: &str,
    ) -> std::io::Result<NamedPipeServer> {
        use std::ffi::c_void;
        use std::os::windows::ffi::OsStrExt;
        use std::os::windows::io::RawHandle;
        use std::ptr::null_mut;

        type HANDLE = *mut c_void;
        type BOOL = i32;
        type DWORD = u32;
        type LPCWSTR = *const u16;
        #[allow(non_camel_case_types)]
        type PSECURITY_DESCRIPTOR = *mut c_void;
        type PVOID = *mut c_void;

        const INVALID_HANDLE_VALUE: HANDLE = usize::MAX as *mut c_void;
        const PIPE_ACCESS_DUPLEX: DWORD = 0x0000_0003;
        const FILE_FLAG_OVERLAPPED: DWORD = 0x4000_0000;
        const PIPE_TYPE_BYTE: DWORD = 0x0000_0000;
        const PIPE_READMODE_BYTE: DWORD = 0x0000_0000;
        const PIPE_WAIT: DWORD = 0x0000_0000;
        const SDDL_REVISION_1: DWORD = 1;

        #[repr(C)]
        struct SECURITY_ATTRIBUTES {
            n_length: DWORD,
            lp_security_descriptor: PSECURITY_DESCRIPTOR,
            b_inherit_handle: BOOL,
        }

        #[link(name = "advapi32")]
        unsafe extern "system" {
            fn ConvertStringSecurityDescriptorToSecurityDescriptorW(
                string_security_descriptor: LPCWSTR,
                string_sd_revision: DWORD,
                security_descriptor: *mut PSECURITY_DESCRIPTOR,
                security_descriptor_size: *mut DWORD,
            ) -> BOOL;
            fn LocalFree(h_mem: PVOID) -> PVOID;
        }

        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn CreateNamedPipeW(
                lp_name: LPCWSTR,
                dw_open_mode: DWORD,
                dw_pipe_mode: DWORD,
                n_max_instances: DWORD,
                n_out_buffer_size: DWORD,
                n_in_buffer_size: DWORD,
                n_default_time_out: DWORD,
                lp_security_attributes: *mut SECURITY_ATTRIBUTES,
            ) -> HANDLE;
        }

        fn to_wide(s: &str) -> Vec<u16> {
            std::ffi::OsStr::new(s)
                .encode_wide()
                .chain(std::iter::once(0))
                .collect()
        }

        let sddl = "D:(A;;FA;;;WD)S:(ML;;NW;;;LW)";
        let sddl_w = to_wide(sddl);
        let mut sd: PSECURITY_DESCRIPTOR = null_mut();
        let ok = unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl_w.as_ptr(),
                SDDL_REVISION_1,
                &mut sd,
                null_mut(),
            )
        };
        if ok == 0 {
            return Err(std::io::Error::last_os_error());
        }
        let mut sa = SECURITY_ATTRIBUTES {
            n_length: std::mem::size_of::<SECURITY_ATTRIBUTES>() as DWORD,
            lp_security_descriptor: sd,
            b_inherit_handle: 0,
        };
        let name_w = to_wide(pipe_name);
        let h = unsafe {
            CreateNamedPipeW(
                name_w.as_ptr(),
                PIPE_ACCESS_DUPLEX | FILE_FLAG_OVERLAPPED,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
                1,
                4096,
                4096,
                0,
                &mut sa,
            )
        };
        unsafe {
            LocalFree(sd as PVOID);
        }
        if h == INVALID_HANDLE_VALUE {
            return Err(std::io::Error::last_os_error());
        }
        unsafe { NamedPipeServer::from_raw_handle(h as RawHandle) }
    }

    async fn run(
        local_ip: Ipv4Addr,
    ) {

        let pipe_name = pslinkb::dns::windows::windivert::PIPE_NAME;
        let pipe_server = match create_pipe_server(pipe_name) {
            Ok(s) => s,
            Err(e) => {
                log_impl(&format!("[ERROR] WinDivert helper error: {}", e));
                std::process::exit(1);
            }
        };

        if let Err(e) = pipe_server.connect().await {
            log_impl(&format!("[ERROR] WinDivert helper error: {}", e));
            std::process::exit(1);
        }
        log_impl("[INFO] Named pipe connected (main attached)");

        let (mut pipe_rd, mut pipe_wr) = tokio::io::split(pipe_server);

        let (tx, mut pipe_rx) = mpsc::unbounded_channel::<String>();
        pslinkb::dns::windows::windivert::set_pipe_tx(tx);

        tokio::spawn(async move {
            while let Some(line) = pipe_rx.recv().await {
                if pipe_wr.write_all(line.as_bytes()).await.is_err() {
                    break;
                }
                if pipe_wr.write_all(b"\n").await.is_err() {
                    break;
                }
            }
        });

        let pipe_reader = tokio::spawn(async move {
            let mut buf = [0u8; 16];
            loop {
                match pipe_rd.read(&mut buf).await {
                    Ok(0) => {
                        log_impl("[INFO] 管道 EOF,主程序已退出");
                        break;
                    }
                    Ok(_) => continue, 
                    Err(e) => {
                        log_impl(&format!("[ERROR] WinDivert helper error: {}", e));
                        break;
                    }
                }
            }
            pslinkb::dns::windows::windivert::shutdown_1935_intercept();
            pslinkb::dns::windows::windivert::shutdown_6667_intercept();
            std::process::exit(0);
        });

        if let Err(e) = pslinkb::dns::windows::windivert::start_1935_intercept(local_ip) {
            log_impl(&format!("[ERROR] WinDivert helper error: {}", e));
            std::process::exit(1);
        }
        log_impl(&format!("[INFO] WinDivert 1935 redirect -> {}", local_ip));

        if let Err(e) = pslinkb::dns::windows::windivert::start_6667_intercept(local_ip) {
            log_impl(&format!("[ERROR] WinDivert helper error: {}", e));
            std::process::exit(1);
        }
        log_impl(&format!("[INFO] WinDivert 6667 redirect -> {}", local_ip));

        pslinkb::dns::windows::windivert::pipe_send("[[INIT_DONE]]");

        let _ = pipe_reader.await;
    }

    fn log_file_path() -> Option<std::path::PathBuf> {
        let exe = std::env::current_exe().ok()?;
        let dir = exe.parent()?;
        Some(dir.join("pslinkb-windivert.log"))
    }

    fn init_log_file() {
        if let Some(path) = log_file_path() {
            let _ = std::fs::File::create(&path);
        }
    }

    fn log_impl(msg: &str) {
        let line = format!("{}\n", msg);
        eprint!("[pslinkb-windivert] {}", line);
        if let Some(path) = log_file_path() {
            use std::io::Write;
            if let Ok(mut f) = std::fs::OpenOptions::new().append(true).create(true).open(&path) {
                let _ = f.write_all(line.as_bytes());
            }
        }
    }
}

fn main() {
    #[cfg(windows)]
    imp::real_main();
    #[cfg(not(windows))]
    {
        eprintln!("pslinkb-windivert is only supported on Windows targets (enable the `windivert` feature for Windows builds).");
        std::process::exit(1);
    }
}
