// FFmpeg struct field accessors — C compiler guarantees correct layout
#include <libavformat/avformat.h>
#include <libavcodec/avcodec.h>

unsigned int ps_nb_streams(AVFormatContext *ctx) { return ctx->nb_streams; }
AVStream **ps_streams(AVFormatContext *ctx) { return ctx->streams; }
int64_t ps_duration(AVFormatContext *ctx) { return ctx->duration; }

AVCodecParameters *ps_codecpar(AVStream *s) { return s->codecpar; }
int ps_stream_index(AVStream *s) { return s->index; }
int ps_tb_num(AVStream *s) { return s->time_base.num; }
int ps_tb_den(AVStream *s) { return s->time_base.den; }

int ps_pkt_stream_index(AVPacket *pkt) { return pkt->stream_index; }
void ps_pkt_set_stream_index(AVPacket *pkt, int idx) { pkt->stream_index = idx; }

enum AVMediaType ps_codec_type(AVCodecParameters *cp) { return cp->codec_type; }
int ps_codec_id(AVCodecParameters *cp) { return (int)cp->codec_id; }

int ps_avio_open(AVFormatContext *ctx, const char *url) {
    return avio_open(&ctx->pb, url, AVIO_FLAG_WRITE);
}

// 中断回调: opaque 指向 streaming_flag 的字节. 1=推流中(继续) 0=停止(中止阻塞读)
// 返回非 0 时 av_read_frame 等阻塞 I/O 立刻以 AVERROR_EXIT 返回 -> 主动停=干净停
static int ps_interrupt_cb(void *opaque) {
    return (*(volatile unsigned char *)opaque) ? 0 : 1;
}
void ps_set_interrupt(AVFormatContext *ctx, void *opaque) {
    ctx->interrupt_callback.callback = ps_interrupt_cb;
    ctx->interrupt_callback.opaque = opaque;
}
