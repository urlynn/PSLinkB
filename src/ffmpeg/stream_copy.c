// 独立 stream copy 工具
// gcc -O2 stream_copy.c -I../ffbuild/.../include -L../ffbuild/.../lib -lavformat -lavcodec -lavutil -o pslinkb-ffmpeg

#include <libavformat/avformat.h>
#include <libavcodec/avcodec.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#ifndef STREAM_VERSION
#define STREAM_VERSION "0.1.1"
#endif

static char errbuf[256];

int main(int argc, char **argv) {
    if (argc == 2 && (strcmp(argv[1], "-v") == 0 || strcmp(argv[1], "--version") == 0)) {
        printf("pslinkb-stream %s\n", STREAM_VERSION);
        return 0;
    }

    if (argc != 3) {
        fprintf(stderr, "Usage: %s [-v] <input_url> <output_url>\n", argv[0]);
        return 1;
    }

    const char *input_url = argv[1];
    const char *output_url = argv[2];

    AVFormatContext *ictx = NULL, *octx = NULL;
    AVPacket *pkt = NULL;
    int ret, *stream_map = NULL;
    int mapped = 0;

    // Open input
    if (avformat_open_input(&ictx, input_url, NULL, NULL) < 0) goto fail;
    if (avformat_find_stream_info(ictx, NULL) < 0) goto fail;

    // Create output
    if (avformat_alloc_output_context2(&octx, NULL, "flv", output_url) < 0) goto fail;
    if (avio_open(&octx->pb, output_url, AVIO_FLAG_WRITE) < 0) goto fail;

    // Map streams
    stream_map = calloc(ictx->nb_streams, sizeof(int));
    for (unsigned i = 0; i < ictx->nb_streams; i++) {
        AVCodecParameters *cp = ictx->streams[i]->codecpar;
        if (cp->codec_type != AVMEDIA_TYPE_VIDEO && cp->codec_type != AVMEDIA_TYPE_AUDIO) {
            stream_map[i] = -1;
            continue;
        }
        AVStream *ost = avformat_new_stream(octx, NULL);
        if (!ost) goto fail;
        avcodec_parameters_copy(ost->codecpar, cp);
        stream_map[i] = mapped++;
    }

    // Write header
    if (avformat_write_header(octx, NULL) < 0) goto fail;

    // Stream copy
    pkt = av_packet_alloc();
    while (av_read_frame(ictx, pkt) >= 0) {
        int idx = pkt->stream_index;
        if (idx < (int)ictx->nb_streams && stream_map[idx] >= 0) {
            pkt->stream_index = stream_map[idx];
            av_write_frame(octx, pkt);
        }
        av_packet_unref(pkt);
    }

    av_write_trailer(octx);
    avformat_free_context(octx);
    octx = NULL;
    avformat_close_input(&ictx);
    av_packet_free(&pkt);
    free(stream_map);
    return 0;

fail:
    av_strerror(ret, errbuf, sizeof(errbuf));
    fprintf(stderr, "Error: %s\n", errbuf);
    if (octx) { av_write_trailer(octx); avformat_free_context(octx); }
    if (ictx) avformat_close_input(&ictx);
    if (pkt) av_packet_free(&pkt);
    free(stream_map);
    return 1;
}
