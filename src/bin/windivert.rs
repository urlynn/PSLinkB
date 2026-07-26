//! pslinkb-windivert

#[cfg(windows)]
mod imp {
    use std::net::{Ipv4Addr, SocketAddr};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::windows::named_pipe::NamedPipeServer;
    use tokio::sync::mpsc;
    use tokio::sync::watch;

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

        let proxy_url: Option<String> = args.get(1)
            .filter(|s| !s.is_empty() && s.as_str() != "none")
            .map(|s| s.to_string());

        let upstream: Option<SocketAddr> = args.get(2)
            .filter(|s| s.as_str() != "none")
            .and_then(|s| s.parse().ok());

        let local_ip = windows_local_ip();

        pslinkb::dns::windows::windivert::set_logger(log_impl);
        pslinkb::log::set_debug_enabled(true);
        pslinkb::log::set_override(log_impl);

        log_impl(&format!(
            "[INFO] Start: proxy_url={} - upstream={}",
            proxy_url.as_deref().unwrap_or("none"),
            upstream.map(|s| s.to_string()).unwrap_or_else(|| "auto".into()),
        ));

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
            run(proxy_url, local_ip, upstream).await;
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
        proxy_url: Option<String>,
        local_ip: Ipv4Addr,
        upstream: Option<SocketAddr>,
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
                        log_impl("[DEBUG] 管道 EOF(主程序已退出),准备退出");
                        break;
                    }
                    Ok(_) => continue, 
                    Err(e) => {
                        log_impl(&format!("[ERROR] WinDivert helper error: {}", e));
                        break;
                    }
                }
            }
            pslinkb::dns::windows::windivert::shutdown_443_intercept();
            pslinkb::dns::windows::windivert::shutdown_intercept();
            std::process::exit(0);
        });

        let mut domains: Vec<&str> = pslinkb::dns::REDIRECT_DOMAINS.to_vec();
        if proxy_url.is_some() {
            domains.extend_from_slice(pslinkb::dns::PROXY_DOMAINS);
        }

        let (ps5_tx, ps5_rx) = watch::channel::<Option<Ipv4Addr>>(None);

        {
            let ps5_rx_clone = ps5_rx.clone();
            let proxy_enabled = proxy_url.is_some();
            tokio::spawn(async move {
                let mut rx = ps5_rx_clone;
                if rx.changed().await.is_ok() {
                    if let Some(ps5_ip) = *rx.borrow() {
                        log_impl(&format!("[INFO] PS5 detected: {}", ps5_ip));
                        let ps5_line = format!(
                            "\x1b[32m[WinDivert] PS5: {} -> Windows: {} - ✓ DNS 重定向正常\x1b[0m",
                            ps5_ip, local_ip
                        );
                        pslinkb::dns::windows::windivert::pipe_send(ps5_line);
                        if !proxy_enabled {
                            pslinkb::dns::windows::windivert::pipe_send("[[INIT_DONE]]");
                        }
                    }
                }
            });
        }

        if let Some(purl) = proxy_url.clone() {
            let ps5_rx_clone = ps5_rx.clone();
            tokio::spawn(async move {
                let mut rx = ps5_rx_clone;
                if rx.changed().await.is_ok() {
                    let ps5_ip = *rx.borrow();
                    if let Some(ps5_ip) = ps5_ip {
                        let mut proxy_ok = true;
                        for domain in pslinkb::dns::PROXY_DOMAINS {
                            match pslinkb::dns::windows::relay::check_proxy_connectivity(&purl, domain).await {
                                Ok(()) => {
                                    pslinkb::dns::windows::windivert::pipe_send(format!(
                                        "\x1b[32m[WinDivert] Proxy - {} - {} ✓ 代理连通性检查通过\x1b[0m",
                                        purl, domain
                                    ));
                                }
                                Err(e) => {
                                    pslinkb::dns::windows::windivert::pipe_send(format!(
                                        "\x1b[31m[WinDivert] Proxy - {} - ✗ {} 代理失败 {}\x1b[0m",
                                        purl, domain, e
                                    ));
                                    proxy_ok = false;
                                    break;
                                }
                            }
                        }
                        if proxy_ok {
                            pslinkb::dns::windows::windivert::pipe_send("[[INIT_DONE]]");
                        }

                        // 启动 relay
                        let purl_for_relay = purl.clone();
                        tokio::spawn(async move {
                            pslinkb::dns::windows::relay::start(purl_for_relay, log_impl).await;
                        });
                        log_impl("[INFO] Relay started (8443)");

                        // open 443 handle
                        match pslinkb::dns::windows::windivert::start_443_intercept(ps5_ip, 8443) {
                            Ok(()) => log_impl(&format!("[INFO] WinDivert 443 redirect: {} -> 8443", ps5_ip)),
                            Err(e) => log_impl(&format!("[ERROR] WinDivert helper error: {}", e)),
                        }
                    }
                }
            });
        }

        let proxy = match pslinkb::dns::DnsProxy::with_ps5(
            &domains,
            local_ip,
            upstream,
            Some(ps5_tx),
        ).await {
            Ok(p) => p,
            Err(e) => {
                log_impl(&format!("[ERROR] WinDivert helper error: {}", e));
                std::process::exit(1);
            }
        };

        let dns_port = proxy.listen_port();
        if let Err(e) = pslinkb::dns::windows::windivert::start_intercept(dns_port) {
            log_impl(&format!("[ERROR] WinDivert helper error: {}", e));
            std::process::exit(1);
        }
        log_impl(&format!("[INFO] WinDivert DNS intercept: 53 -> {}", dns_port));

        tokio::spawn(async move {
            if let Err(e) = proxy.serve().await {
                log_impl(&format!("[ERROR] WinDivert helper error: {}", e));
            }
        });

        let _ = pipe_reader.await;
    }

    fn windows_local_ip() -> Ipv4Addr {
        use std::ptr;

        #[repr(C)]
        struct IpAddrRow {
            dw_addr: u32,
            dw_index: u32,
            dw_mask: u32,
            dw_bcast_addr: u32,
            dw_reasm_size: u32,
            unused1: u16,
            unused2: u16,
        }
        #[repr(C)]
        struct IpAddrTable {
            dw_num_entries: u32,
            table: [IpAddrRow; 1],
        }

        #[link(name = "iphlpapi")]
        unsafe extern "system" {
            fn GetIpAddrTable(
                p_ip_addr_table: *mut IpAddrTable,
                pdw_size: *mut u32,
                b_order: u32,
            ) -> u32;
            fn GetBestInterface(dw_dest_addr: u32, pdw_best_if_index: *mut u32) -> u32;
        }

        const LOOPBACK: Ipv4Addr = Ipv4Addr::new(127, 0, 0, 1);

        unsafe {
            let mut best_if: u32 = 0;
            let have_best = GetBestInterface(0x08080808, &mut best_if) == 0;

            let mut size: u32 = 0;
            GetIpAddrTable(ptr::null_mut(), &mut size, 0);
            if size == 0 {
                size = 1024;
            }
            let mut buf: Vec<u8> = vec![0u8; size as usize];
            let table_ptr = buf.as_mut_ptr() as *mut IpAddrTable;
            if GetIpAddrTable(table_ptr, &mut size, 0) != 0 {
                return LOOPBACK;
            }
            let num = (*table_ptr).dw_num_entries as usize;
            let rows = std::slice::from_raw_parts((*table_ptr).table.as_ptr(), num);

            if have_best {
                for r in rows {
                    if r.dw_index == best_if {
                        let ip = Ipv4Addr::from(u32::from_be(r.dw_addr));
                        if ip != LOOPBACK {
                            return ip;
                        }
                    }
                }
            }

            for r in rows {
                let ip = Ipv4Addr::from(u32::from_be(r.dw_addr));
                if ip != LOOPBACK {
                    return ip;
                }
            }
            LOOPBACK
        }
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
