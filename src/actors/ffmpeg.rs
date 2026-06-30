//! FFmpeg Actor

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::{broadcast, mpsc};

use crate::core::error::{AppError, FfmpegErrorKind, FfmpegExitStatus};
use crate::core::event::Event;
use crate::dlog;
use crate::log;

// ── 共享类型 ──

#[derive(Debug)]
pub enum FfmpegCommand {
    Start {
        ps5_app: String,
        ps5_stream_key: String,
        bilibili_rtmp_url: String,
        bilibili_stream_key: String,
    },
    Stop,
}

pub struct FfmpegActor {
    cmd_rx: mpsc::Receiver<FfmpegCommand>,
    event_tx: mpsc::Sender<Event>,
    streaming_flag: Arc<AtomicBool>,
}

impl FfmpegActor {
    pub fn new(cmd_rx: mpsc::Receiver<FfmpegCommand>, event_tx: mpsc::Sender<Event>) -> Self {
        Self {
            cmd_rx,
            event_tx,
            streaming_flag: Arc::new(AtomicBool::new(false)),
        }
    }

    pub async fn run(mut self, mut shutdown_rx: broadcast::Receiver<()>) -> Result<(), AppError> {
        loop {
            tokio::select! {
                _ = shutdown_rx.recv() => {
                    self.streaming_flag.store(false, Ordering::SeqCst);
                    eprintln!("[FFmpeg] Stop signal received -> Stopped");
                    break;
                }
                cmd = self.cmd_rx.recv() => {
                    match cmd {
                        Some(FfmpegCommand::Start { ps5_app, ps5_stream_key, bilibili_rtmp_url, bilibili_stream_key }) => {
                            self.streaming_flag.store(true, Ordering::SeqCst);
                            let flag = Arc::clone(&self.streaming_flag);
                            let mut inner_shutdown = shutdown_rx.resubscribe();
                            let et = self.event_tx.clone();

                            tokio::task::spawn_blocking(move || {
                                let input_url = crate::config::rtmp_url("127.0.0.1", &ps5_app, &ps5_stream_key);
                                let output_url = format!("{}{}", bilibili_rtmp_url, bilibili_stream_key);

                                match Self::do_stream(
                                    &input_url, &output_url, &flag, &mut inner_shutdown,
                                ) {
                                    Err(FfmpegExitStatus::Normal) => {
                                        dlog!("[FFmpeg] Streaming completed");
                                    }
                                    Err(status) => {
                                        log!(error, "FFmpeg: Streaming error");
                                        let _ = et.try_send(Event::FfmpegError(status));
                                    }
                                    Ok(()) => unreachable!(),
                                }
                            });
                        }
                        Some(FfmpegCommand::Stop) => {
                            self.streaming_flag.store(false, Ordering::SeqCst);
                        }
                        None => { break; }
                    }
                }
            }
        }
        Ok(())
    }

    // ── 平台分发 ──

    #[cfg(feature = "ffi-ffmpeg")]
    fn do_stream(
        input_url: &str,
        output_url: &str,
        flag: &Arc<AtomicBool>,
        shutdown_rx: &mut broadcast::Receiver<()>,
    ) -> Result<(), FfmpegExitStatus> {
        match self::ffi::do_stream(input_url, output_url, flag, shutdown_rx) {
            Ok(()) => Err(FfmpegExitStatus::Normal), // 正常完成（EOF）
            Err(e) => Err(e),
        }
    }

    #[cfg(not(feature = "ffi-ffmpeg"))]
    fn do_stream(
        input_url: &str,
        output_url: &str,
        flag: &Arc<AtomicBool>,
        shutdown_rx: &mut broadcast::Receiver<()>,
    ) -> Result<(), FfmpegExitStatus> {
        self::external::do_stream(input_url, output_url, flag, shutdown_rx)
    }
}

// ————————————————————————————————————————————————————————————
//  子进程管理 — external-ffmpeg / 系统 ffmpeg
// ————————————————————————————————————————————————————————————

#[cfg(not(feature = "ffi-ffmpeg"))]
fn run_process(
    mut child: std::process::Child,
    label: &str,
    flag: &Arc<AtomicBool>,
    shutdown_rx: &mut broadcast::Receiver<()>,
) -> Result<(), FfmpegExitStatus> {
    loop {
        std::thread::sleep(std::time::Duration::from_millis(500));

        match child.try_wait() {
            Ok(Some(status)) => {
                if status.success() {
                    return Err(FfmpegExitStatus::Normal);
                }
                let mut err = String::new();
                if let Some(mut stderr) = child.stderr {
                    let _ = std::io::Read::read_to_string(&mut stderr, &mut err);
                }
                if status.code().is_none() {
                    return Err(FfmpegExitStatus::Error(FfmpegErrorKind::Crash(format!(
                        "signal kill: {}",
                        err.trim()
                    ))));
                }
                return Err(FfmpegExitStatus::Error(FfmpegErrorKind::IoError(format!(
                    "exited with {}: {}",
                    status,
                    err.trim()
                ))));
            }
            Ok(None) => {}
            Err(e) => {
                return Err(FfmpegExitStatus::Error(FfmpegErrorKind::IoError(
                    e.to_string(),
                )));
            }
        }

        if !flag.load(Ordering::SeqCst) {
            eprintln!("[{}] Stopping worker...", label);
            let _ = child.kill();
            return Err(FfmpegExitStatus::Normal);
        }
        if shutdown_rx.try_recv().is_ok() {
            let _ = child.kill();
            return Err(FfmpegExitStatus::Normal);
        }
    }
}

// ————————————————————————————————————————————————————————
//  FFI 模式 — unsafe FFmpeg C 库
// ————————————————————————————————————————————————————————

#[cfg(feature = "ffi-ffmpeg")]
mod ffi {
    use super::*;
    use crate::ffmpeg::{self, AVMediaType, InputContext, OutputContext};

    pub fn do_stream(
        input_url: &str,
        output_url: &str,
        flag: &Arc<AtomicBool>,
        shutdown_rx: &mut broadcast::Receiver<()>,
    ) -> Result<(), FfmpegExitStatus> {
        do_stream_once(input_url, output_url, flag, shutdown_rx)
            .map_err(|e| FfmpegExitStatus::Error(FfmpegErrorKind::IoError(e.to_string())))
    }

    fn do_stream_once(
        input_url: &str,
        output_url: &str,
        flag: &Arc<AtomicBool>,
        shutdown_rx: &mut broadcast::Receiver<()>,
    ) -> Result<(), AppError> {
        ffmpeg::init(); // 日志级别
        eprintln!(
            "[FFmpeg] ffmpeg -i {} -c copy -f flv {}",
            input_url, output_url
        );
        // 主动停止时 av_read_frame 立刻 AVERROR_EXIT 返回
        let ictx = InputContext::open(input_url, Arc::as_ptr(flag) as *mut std::ffi::c_void)?;
        let stream_count = ictx.stream_count();

        let mut descs = Vec::new();
        for i in 0..stream_count {
            if let Some(s) = ictx.stream(i)
                && let Some(cp) = ffmpeg::stream_codecpar(s)
            {
                let ct = ffmpeg::codec_type(cp);
                let cid = ffmpeg::codec_id(cp);
                let kind = if ct == AVMediaType::AVMEDIA_TYPE_VIDEO {
                    "Video:"
                } else if ct == AVMediaType::AVMEDIA_TYPE_AUDIO {
                    "Audio:"
                } else {
                    "?: "
                };
                descs.push(format!("#{} {} {}", i, kind, ffmpeg::codec_name(cid)));
            }
        }
        eprintln!(
            "[FFmpeg] {} stream(s) - {} - Streaming",
            stream_count,
            descs.join(" | ")
        );

        let mut octx = OutputContext::create(output_url, "flv")?;
        let mut stream_map: Vec<i32> = vec![-1; stream_count as usize];
        let mut mapped = 0i32;

        for i in 0..stream_count {
            if let Some(s) = ictx.stream(i)
                && let Some(cp) = ffmpeg::stream_codecpar(s)
            {
                let ct = ffmpeg::codec_type(cp);
                if ct == AVMediaType::AVMEDIA_TYPE_VIDEO || ct == AVMediaType::AVMEDIA_TYPE_AUDIO {
                    octx.add_stream(cp)?;
                    stream_map[i as usize] = mapped;
                    mapped += 1;
                }
            }
        }

        octx.write_header()?;
        let mut packet_count: u64 = 0;
        let start_time = std::time::Instant::now();
        loop {
            if !flag.load(Ordering::SeqCst) {
                eprintln!("[FFmpeg] Stopped by user");
                break;
            }
            if shutdown_rx.try_recv().is_ok() {
                break;
            }

            match ictx.read_packet() {
                Ok(Some(pkt)) => {
                    let idx = ffmpeg::pkt_stream_index(pkt) as usize;
                    if idx < stream_map.len() && stream_map[idx] >= 0 {
                        ffmpeg::pkt_set_stream_index(pkt, stream_map[idx]);
                        octx.write_packet(pkt)?;
                        packet_count += 1;
                    }
                    ffmpeg::pkt_unref(pkt);
                    ffmpeg::pkt_free(pkt);
                }
                Ok(None) => {
                    eprintln!("[FFmpeg] Input ended -> Stopped");
                    break;
                }
                Err(ref e) if e.to_string().contains("End of file") => break,
                Err(e) => return Err(e),
            }
        }

        let _ = octx.write_trailer();
        let e = start_time.elapsed().as_secs_f64().max(0.001);
        eprintln!(
            "[FFmpeg] Done: {} packets in {:.1}s ({:.0} pkt/s)",
            packet_count,
            e,
            packet_count as f64 / e
        );
        Ok(())
    }
}

// ————————————————————————————————————————————————————————
//  子进程模式 — external-ffmpeg / 系统 ffmpeg
// ————————————————————————————————————————————————————————

#[cfg(not(feature = "ffi-ffmpeg"))]
mod external {
    use super::*;
    use std::process::{Command, Stdio};

    #[cfg(feature = "external-ffmpeg")]
    fn find_stream_bin() -> String {
        if let Ok(exe) = std::env::current_exe() {
            let dir = exe.parent().unwrap();
            #[cfg(windows)]
            let name = "pslinkb-stream.exe";
            #[cfg(unix)]
            let name = "pslinkb-stream";
            let path = dir.join(name);
            if path.exists() {
                return path.display().to_string();
            }
        }
        "pslinkb-stream".to_string()
    }

    pub fn do_stream(
        input_url: &str,
        output_url: &str,
        flag: &Arc<AtomicBool>,
        shutdown_rx: &mut broadcast::Receiver<()>,
    ) -> Result<(), FfmpegExitStatus> {
        #[cfg(feature = "external-ffmpeg")]
        let bin = find_stream_bin();

        #[cfg(not(feature = "external-ffmpeg"))]
        let bin = "ffmpeg";

        eprintln!("[FFmpeg] {} {} {}", bin, input_url, output_url);

        #[cfg(feature = "external-ffmpeg")]
        let args: [&str; 2] = [input_url, output_url];

        #[cfg(not(feature = "external-ffmpeg"))]
        let args = ["-i", input_url, "-c", "copy", "-f", "flv", output_url];

        let child = Command::new(&bin)
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| FfmpegExitStatus::Error(FfmpegErrorKind::IoError(e.to_string())))?;

        super::run_process(child, "FFmpeg", flag, shutdown_rx)
    }
}
