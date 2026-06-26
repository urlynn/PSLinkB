// FFmpeg FFI — 仅在非 external-ffmpeg 模式下编译。
// 外部模式用子进程 pslinkb-stream，不链接任何 FFmpeg 库。

#![allow(non_camel_case_types, dead_code, unused_imports)]

use std::ffi::{CStr, CString, c_void};
use std::os::raw::{c_char, c_int, c_uint};
use std::ptr;

use crate::core::error::AppError;

// ── 始终可见 ──

#[repr(i32)]
#[derive(PartialEq, Eq)]
pub enum AVMediaType {
    AVMEDIA_TYPE_VIDEO = 0,
    AVMEDIA_TYPE_AUDIO = 1,
}

// ————————————————————————————————————————————————————————
//  external-ffmpeg 模式 — 桩函数
// ————————————————————————————————————————————————————————

#[cfg(feature = "external-ffmpeg")]
mod ext_stubs {
    use super::*;

    pub fn codec_name(id: i32) -> &'static str {
        match id {
            27 => "AVC", 28 => "HEVC", 12 => "MPEG2",
            86018 => "AAC", 86017 => "MP3", 86056 => "OPUS",
            _ => "?",
        }
    }

    pub fn codec_id(_cp: *const c_void) -> i32 { 0 }

    pub fn codec_type(_cp: *const c_void) -> AVMediaType { AVMediaType::AVMEDIA_TYPE_VIDEO }

    pub fn stream_codecpar(_s: *mut c_void) -> Option<*mut c_void> { None }
}

#[cfg(feature = "external-ffmpeg")]
pub use ext_stubs::*;

// ————————————————————————————————————————————————————————
//  FFI 模式 — 以下仅在非 external-ffmpeg 时编译
// ————————————————————————————————————————————————————————

#[cfg(feature = "ffi-ffmpeg")]
mod ffi_impl {
    use super::*;

    pub use super::*;

    mod ffi_types {
        pub type AVFormatContext = std::ffi::c_void;
        pub type AVStream = std::ffi::c_void;
        pub type AVCodecParameters = std::ffi::c_void;
        pub type AVPacket = std::ffi::c_void;
        pub type AVOutputFormat = std::ffi::c_void;
    }
    use self::ffi_types::*;

    unsafe extern "C" {
        fn avformat_alloc_context() -> *mut AVFormatContext;
        fn avformat_open_input(ps: *mut *mut AVFormatContext, url: *const c_char, _: *mut c_void, _: *mut c_void) -> c_int;
        fn avformat_find_stream_info(ctx: *mut AVFormatContext, _: *mut c_void) -> c_int;
        fn avformat_close_input(ps: *mut *mut AVFormatContext);
        fn avformat_free_context(ctx: *mut AVFormatContext);
        fn avformat_alloc_output_context2(ctx: *mut *mut AVFormatContext, fmt: *mut AVOutputFormat, fmt_name: *const c_char, filename: *const c_char) -> c_int;
        fn avformat_new_stream(s: *mut AVFormatContext, _: *const c_void) -> *mut AVStream;
        fn avformat_write_header(ctx: *mut AVFormatContext, _: *mut c_void) -> c_int;
        fn av_write_trailer(ctx: *mut AVFormatContext) -> c_int;
        fn av_packet_alloc() -> *mut AVPacket;
        fn av_packet_free(pkt: *mut *mut AVPacket);
        fn av_packet_unref(pkt: *mut AVPacket);
        fn av_read_frame(ctx: *mut AVFormatContext, pkt: *mut AVPacket) -> c_int;
        fn av_write_frame(ctx: *mut AVFormatContext, pkt: *mut AVPacket) -> c_int;
        fn av_interleaved_write_frame(ctx: *mut AVFormatContext, pkt: *mut AVPacket) -> c_int;
        fn avcodec_parameters_copy(dst: *mut AVCodecParameters, src: *const AVCodecParameters) -> c_int;
        fn av_strerror(errnum: c_int, errbuf: *mut u8, errbuf_size: usize) -> c_int;
        fn av_log_set_level(level: c_int);
    }

    unsafe extern "C" {
        fn ps_nb_streams(ctx: *mut AVFormatContext) -> c_uint;
        fn ps_streams(ctx: *mut AVFormatContext) -> *mut *mut AVStream;
        fn ps_duration(ctx: *mut AVFormatContext) -> i64;
        fn ps_codecpar(s: *mut AVStream) -> *mut AVCodecParameters;
        fn ps_stream_index(s: *mut AVStream) -> c_int;
        fn ps_tb_num(s: *mut AVStream) -> c_int;
        fn ps_tb_den(s: *mut AVStream) -> c_int;
        fn ps_pkt_stream_index(pkt: *mut AVPacket) -> c_int;
        fn ps_pkt_set_stream_index(pkt: *mut AVPacket, idx: c_int);
        fn ps_codec_type(cp: *const AVCodecParameters) -> AVMediaType;
        fn ps_codec_id(cp: *const AVCodecParameters) -> c_int;
        fn ps_avio_open(ctx: *mut AVFormatContext, url: *const c_char) -> c_int;
        fn ps_set_interrupt(ctx: *mut AVFormatContext, opaque: *mut c_void);
    }

    /// 设 FFmpeg 内部日志级别
    pub fn init() {
        // AV_LOG_INFO=32, AV_LOG_ERROR=16
        let level = if cfg!(any(debug_assertions, feature = "debug-log")) { 32 } else { 16 };
        unsafe { av_log_set_level(level); }
    }

    pub fn strerror(err: c_int) -> String {
        let mut buf = vec![0i8; 256];
        unsafe { av_strerror(err, buf.as_mut_ptr() as *mut u8, buf.len()); }
        CStr::from_bytes_until_nul(&buf.iter().map(|&c| c as u8).collect::<Vec<_>>())
            .unwrap_or_default().to_string_lossy().into_owned()
    }

    fn check(ret: c_int) -> Result<c_int, AppError> {
        if ret < 0 { Err(strerror(ret).into()) } else { Ok(ret) }
    }

    pub fn nb_streams(ctx: *mut AVFormatContext) -> u32 { unsafe { ps_nb_streams(ctx) } }

    pub fn stream(ctx: *mut AVFormatContext, idx: u32) -> Option<*mut AVStream> {
        unsafe {
            let ptrs = ps_streams(ctx);
            if ptrs.is_null() { None } else { Some(ptrs.add(idx as usize).read()) }
        }
    }

    pub fn duration(ctx: *mut AVFormatContext) -> i64 { unsafe { ps_duration(ctx) } }

    pub fn stream_codecpar(s: *mut AVStream) -> Option<*mut AVCodecParameters> {
        let p = unsafe { ps_codecpar(s) }; if p.is_null() { None } else { Some(p) }
    }

    pub fn stream_index(s: *mut AVStream) -> i32 { unsafe { ps_stream_index(s) } }

    pub fn codec_type(cp: *const AVCodecParameters) -> AVMediaType { unsafe { ps_codec_type(cp) } }

    pub fn codec_id(cp: *const AVCodecParameters) -> i32 { unsafe { ps_codec_id(cp) } }

    pub fn time_base(stream: *mut AVStream) -> (i32, i32) {
        unsafe { (ps_tb_num(stream), ps_tb_den(stream)) }
    }

    pub fn pkt_stream_index(pkt: *mut AVPacket) -> i32 { unsafe { ps_pkt_stream_index(pkt) } }

    pub fn pkt_set_stream_index(pkt: *mut AVPacket, idx: i32) { unsafe { ps_pkt_set_stream_index(pkt, idx) } }

    pub fn pkt_unref(pkt: *mut AVPacket) { unsafe { av_packet_unref(pkt) } }

    pub fn pkt_free(pkt: *mut AVPacket) { unsafe { av_packet_free(&mut (pkt as *mut _)); } }

    pub struct InputContext { ptr: *mut AVFormatContext }

    impl Drop for InputContext {
        fn drop(&mut self) { if !self.ptr.is_null() { unsafe { avformat_close_input(&mut self.ptr); } } }
    }

    impl InputContext {
        /// 主动停止时 av_read_frame 立刻以 AVERROR_EXIT 返回
        pub fn open(url: &str, interrupt_opaque: *mut c_void) -> Result<Self, AppError> {
            let c_url = CString::new(url)?;
            let mut ctx = unsafe { avformat_alloc_context() };
            if ctx.is_null() { return Err("avformat_alloc_context failed".into()); }
            unsafe { ps_set_interrupt(ctx, interrupt_opaque); }
            check(unsafe { avformat_open_input(&mut ctx, c_url.as_ptr(), ptr::null_mut(), ptr::null_mut()) })?;
            check(unsafe { avformat_find_stream_info(ctx, ptr::null_mut()) })?;
            Ok(Self { ptr: ctx })
        }
        pub fn stream_count(&self) -> u32 { nb_streams(self.ptr) }
        pub fn stream(&self, idx: u32) -> Option<*mut AVStream> { stream(self.ptr, idx) }
        pub fn as_mut_ptr(&self) -> *mut AVFormatContext { self.ptr }
        pub fn read_packet(&self) -> Result<Option<*mut AVPacket>, AppError> {
            let pkt = unsafe { av_packet_alloc() };
            if pkt.is_null() { return Err("OOM".into()); }
            let ret = unsafe { av_read_frame(self.ptr, pkt) };
            if ret < 0 {
                unsafe { av_packet_free(&mut (pkt as *mut *mut AVPacket as *mut _)); }
                // -541478725 = AVERROR_EOF(流正常结束); -1414092869 = AVERROR_EXIT(中断回调=主动停) -> 干净结束
                if ret == -541478725 || ret == -1414092869 {
                    return Ok(None);
                }
                return Err(strerror(ret).into());
            }
            Ok(Some(pkt))
        }
    }

    pub struct OutputContext { ptr: *mut AVFormatContext }

    impl Drop for OutputContext {
        fn drop(&mut self) { if !self.ptr.is_null() { unsafe { avformat_free_context(self.ptr); } } }
    }

    impl OutputContext {
        pub fn create(url: &str, fmt: &str) -> Result<Self, AppError> {
            let c_url = CString::new(url)?;
            let c_fmt = CString::new(fmt)?;
            let mut ctx: *mut AVFormatContext = ptr::null_mut();
            check(unsafe { avformat_alloc_output_context2(&mut ctx, ptr::null_mut(), c_fmt.as_ptr(), c_url.as_ptr()) })?;
            check(unsafe { ps_avio_open(ctx, c_url.as_ptr()) })?;
            Ok(Self { ptr: ctx })
        }
        pub fn add_stream(&mut self, cp: *const AVCodecParameters) -> Result<(), AppError> {
            let s = unsafe { avformat_new_stream(self.ptr, ptr::null()) };
            if s.is_null() { return Err("OOM".into()); }
            check(unsafe { avcodec_parameters_copy(stream_codecpar(s).unwrap(), cp) })?;
            Ok(())
        }
        pub fn write_header(&mut self) -> Result<(), AppError> {
            check(unsafe { avformat_write_header(self.ptr, ptr::null_mut()) })?;
            Ok(())
        }
        pub fn write_trailer(&mut self) -> Result<(), AppError> {
            check(unsafe { av_write_trailer(self.ptr) })?;
            Ok(())
        }
        pub fn write_packet(&mut self, pkt: *mut AVPacket) -> Result<(), AppError> {
            check(unsafe { av_interleaved_write_frame(self.ptr, pkt) })?;
            Ok(())
        }
    }

    pub fn codec_name(id: i32) -> &'static str {
        match id {
            27 => "AVC", 28 => "HEVC", 12 => "MPEG2",
            86018 => "AAC", 86017 => "MP3", 86056 => "OPUS",
            _ => "?",
        }
    }
}

#[cfg(feature = "ffi-ffmpeg")]
pub use ffi_impl::*;
