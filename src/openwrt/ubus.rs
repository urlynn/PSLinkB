/// ubus IPC — experimental - Todo 
/// ubus 当前有 bug!! 开发中.. 等哪天有空再实现 ..有问题别找我

use std::io::Read;
use std::os::unix::net::UnixStream;
use std::path::Path;

use crate::core::error::AppError;

const UBUS_MSG_HELLO: u8 = 0;
const _UBUS_MSG_STATUS: u8 = 1;
const UBUS_MSG_DATA: u8 = 2;
const _UBUS_MSG_LOOKUP: u8 = 4;
const UBUS_MSG_INVOKE: u8 = 5;
const UBUS_MSG_ADD_OBJECT: u8 = 6;

fn hdr(msg_type: u8, seq: u16, peer: u32) -> [u8; 8] {
    let mut b = [0u8; 8];
    b[1] = msg_type;
    b[2..4].copy_from_slice(&seq.to_be_bytes());
    b[4..8].copy_from_slice(&peer.to_be_bytes());
    b
}

fn blob_attr(id: u32, data: &[u8]) -> Vec<u8> {
    let total = (4 + data.len()) as u32;
    let id_len = (id << 24) | total;
    let mut v = id_len.to_be_bytes().to_vec();
    v.extend_from_slice(data);
    v
}

fn read_frame(sock: &mut UnixStream) -> Result<(u8, u16, u32, Vec<u8>), std::io::Error> {
    let mut hdr_buf = [0u8; 8];
    sock.read_exact(&mut hdr_buf)?;
    let ty = hdr_buf[1];
    let seq = u16::from_be_bytes([hdr_buf[2], hdr_buf[3]]);
    let peer = u32::from_be_bytes([hdr_buf[4], hdr_buf[5], hdr_buf[6], hdr_buf[7]]);

    let mut tag_buf = [0u8; 4];
    sock.read_exact(&mut tag_buf)?;
    let tag = u32::from_be_bytes(tag_buf);
    let dlen = (tag & 0x00FF_FFFF) as usize;
    let mut body = vec![0u8; dlen.saturating_sub(4)];
    if body.len() > 0 {
        sock.read_exact(&mut body)?;
    }
    // Debug: append frame info
    use std::io::Write as _;
    let _ = std::fs::OpenOptions::new().append(true).create(true).open("/tmp/pslinkb/.dump")
        .and_then(|mut f| writeln!(f, "ty={} seq={} peer={:x} tag={:08x} dlen={}", ty, seq, peer, tag, dlen));
    Ok((ty, seq, peer, body))
}

fn write_frame(sock: &mut UnixStream, ty: u8, seq: u16, peer: u32, body: &[u8]) -> std::io::Result<()> {
    use std::os::unix::io::AsRawFd;
    let h = hdr(ty, seq, peer);

    // 使用 sendmsg + iovec 原子发送 header+body（匹配 libubus 标准做法）
    let iov = [
        libc::iovec { iov_base: h.as_ptr() as *mut libc::c_void, iov_len: h.len() },
        libc::iovec { iov_base: body.as_ptr() as *mut libc::c_void, iov_len: body.len() },
    ];
    let mut msg: libc::msghdr = unsafe { std::mem::zeroed() };
    msg.msg_iov = iov.as_ptr() as *mut libc::iovec;
    msg.msg_iovlen = iov.len() as libc::c_int;

    let fd = sock.as_raw_fd();
    let ret = unsafe { libc::sendmsg(fd, &msg, 0) };
    if ret < 0 {
        return Err(std::io::Error::last_os_error());
    }
    let total = h.len() + body.len();
    if ret as usize != total {
        return Err(std::io::Error::new(std::io::ErrorKind::WriteZero, "short sendmsg"));
    }
    Ok(())
}

/// 构造 ADD_OBJECT body — status 方法无参数
fn add_object_body() -> Vec<u8> {
    // Method table: name="status" with NO parameters (empty inner table)
    let inner = [
        &[0x00, 0x06][..],  // name_len=6
        b"status",          // "status"
        &[0, 0, 0, 0][..],  // pad
    ].concat();
    let tag = 0x8000_0000u32 | (2 << 24) | (4 + inner.len() as u32);
    let method = [&tag.to_be_bytes()[..], &inner].concat();

    let objpath = blob_attr(2, b"pslinkb\0");
    let sig = blob_attr(6, &method);
    let data = [objpath.as_slice(), sig.as_slice()].concat();
    blob_attr(0, &data)
}

pub fn serve() -> Result<(), AppError> {
    use std::io::Write as _;
    let _ = std::fs::write("/tmp/pslinkb/.ubus", ""); // clear
    let mut sock = UnixStream::connect(Path::new("/var/run/ubus/ubus.sock"))?;
    {
        let _ = std::fs::OpenOptions::new().append(true).open("/tmp/pslinkb/.ubus")
            .and_then(|mut f| writeln!(f, "connect"));
    }

    // ── HELLO ──
    let (ty, _seq, peer, _body) = read_frame(&mut sock)?;
    {
        let _ = std::fs::OpenOptions::new().append(true).open("/tmp/pslinkb/.ubus")
            .and_then(|mut f| writeln!(f, "hello_ty={}", ty));
    }

    // HELLO reply (blob_attr: id=0, len=4)
    write_frame(&mut sock, UBUS_MSG_HELLO, 1, peer, &[0u8, 0, 0, 4])?;
    {
        let _ = std::fs::OpenOptions::new().append(true).open("/tmp/pslinkb/.ubus")
            .and_then(|mut f| writeln!(f, "hello_replied"));
    }

    // ── 读 HELLO ACK ──
    let (ack_ty, _seq, our_peer, _body) = read_frame(&mut sock)?;
    {
        let _ = std::fs::OpenOptions::new().append(true).open("/tmp/pslinkb/.ubus")
            .and_then(|mut f| writeln!(f, "ack_ty={}", ack_ty));
    }

    // ── ADD_OBJECT ──
    let body = add_object_body();
    write_frame(&mut sock, UBUS_MSG_ADD_OBJECT, 1, our_peer, &body)?;
    {
        let _ = std::fs::OpenOptions::new().append(true).open("/tmp/pslinkb/.ubus")
            .and_then(|mut f| writeln!(f, "addobj"));
    }

    // ── 事件循环 ──
    let mut n = 0u32;
    loop {
        n += 1;
        let (ty, seq, _peer, body) = match read_frame(&mut sock) {
            Ok(f) => f,
            Err(e) => {
                use std::io::Write as _;
                let _ = std::fs::OpenOptions::new().append(true).open("/tmp/pslinkb/.ubus")
                    .and_then(|mut f| writeln!(f, "exit_{}:{}", n, e));
                break;
            }
        };
        {
            use std::io::Write as _;
            let _ = std::fs::OpenOptions::new().append(true).open("/tmp/pslinkb/.ubus")
                .and_then(|mut f| writeln!(f, "{}:ty={}", n, ty));
        }

        if ty == UBUS_MSG_INVOKE {
            // Parse OBJID
            let mut oid = 0u32;
            let mut pos = 0;
            while pos + 8 <= body.len() {
                let tag = u32::from_be_bytes([body[pos], body[pos+1], body[pos+2], body[pos+3]]);
                let aid = (tag >> 24) & 0x7F;
                let alen = (tag & 0x00FF_FFFF) as usize;
                if aid == 3 && alen == 8 {
                    oid = u32::from_be_bytes([body[pos+4], body[pos+5], body[pos+6], body[pos+7]]);
                    break;
                }
                pos += alen;
                if pos > body.len() { break; }
            }

            // Build blobmsg TABLE (NO extra type byte — matching system board format)
            let state_val = crate::luci::read("state").unwrap_or_default();
            let user_val = crate::luci::read("user").unwrap_or_default();
            let rtmp_val = crate::luci::read("rtmp").unwrap_or_default();
            let error_val = crate::luci::read("error").unwrap_or_default();
            let qr_val = crate::luci::read("qr_url").unwrap_or_default();

            fn field(name: &str, val: &str) -> Vec<u8> {
                let nb = name.as_bytes();
                let vb = val.as_bytes();
                let np = ((nb.len() + 3) / 4) * 4;    // name padded to 4
                let vp = ((vb.len() + 1 + 3) / 4) * 4; // null-term value padded to 4
                let total = 4 + 2 + np + vp;           // tag + name_len + name_pad + value_pad
                let tag = 0x8000_0000u32 | (3u32 << 24) | (total as u32);
                let mut v = tag.to_be_bytes().to_vec();
                v.extend_from_slice(&(nb.len() as u16).to_be_bytes());
                v.extend_from_slice(nb);
                v.resize(4 + 2 + np, 0);  // pad name
                v.extend_from_slice(vb);   // value
                v.push(0);                 // null terminate
                v.resize(total, 0);        // pad to total
                v
            }

            let mut tbl = Vec::new();
            for (k, v) in &[("state", &state_val), ("user", &user_val), ("rtmp", &rtmp_val), ("error", &error_val), ("qr_url", &qr_val)] {
                tbl.extend_from_slice(&field(k, v));
            }

            let body_content = [
                blob_attr(3, &oid.to_be_bytes()).as_slice(),
                blob_attr(7, &tbl).as_slice(),
            ].concat();
            // Wrap in top-level blob_attr so ubusd computes correct blob_raw_len()
            let resp_body = blob_attr(0, &body_content);

            write_frame(&mut sock, UBUS_MSG_DATA, seq, oid, &resp_body)?;
            // ALSO: 延迟 500ms 后再发一次（测试内核缓冲竞态理论）
            std::thread::sleep(std::time::Duration::from_millis(500));
            write_frame(&mut sock, UBUS_MSG_DATA, seq, oid, &resp_body)?;
            // Dump exact response bytes for comparison
            {
                use std::io::Write as _;
                let full = [&hdr(UBUS_MSG_DATA, seq, oid)[..], &resp_body].concat();
                let hex: String = full.iter().map(|b| format!("{:02x}", b)).collect::<Vec<_>>().join(" ");
                let _ = std::fs::OpenOptions::new().append(true).create(true).open("/tmp/pslinkb/.resp_hex")
                    .and_then(|mut f| writeln!(f, "{}:{}", n, hex));
                let _ = std::fs::OpenOptions::new().append(true).open("/tmp/pslinkb/.ubus")
                    .and_then(|mut f| writeln!(f, "{}:resp_{}b_objid={:x}", n, full.len(), oid));
            }
        }
    }

    Ok(())
}
