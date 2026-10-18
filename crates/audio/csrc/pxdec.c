#include "pxdec.h"
#include <libavformat/avformat.h>
#include <libavcodec/avcodec.h>
#include <libswresample/swresample.h>
#include <libavutil/channel_layout.h>
#include <libavutil/opt.h>
#include <libavutil/mem.h>
#include <stdlib.h>
#include <string.h>

#define IO_BUF 32768

struct PxDec {
    AVFormatContext *fmt;
    AVIOContext *io;
    AVCodecContext *cod;
    SwrContext *swr;
    AVPacket *pkt;
    AVFrame *frm;
    int stream;
    int rate;
    int src_channels;
    int sent_eof;
    int swr_flushed;
    int seek_pending;
    int64_t seek_target; /* in samples at `rate` */
    int64_t cursor;      /* audible position of the next frame while a seek lead-in is dropped, or -1 */
    float *out;
    int out_cap;         /* frames */
    void *opaque;
    px_read_fn rd;
    px_seek_fn sk;
    px_size_fn sz;
    int io_failed;
    int is_aac;
};

static int read_cb(void *o, uint8_t *buf, int len) {
    PxDec *d = o;
    int n = d->rd(d->opaque, buf, len);
    if (n > 0) return n;
    if (n == 0) return AVERROR_EOF;
    d->io_failed = 1;
    return AVERROR(EIO);
}

static int64_t seek_cb(void *o, int64_t off, int whence) {
    PxDec *d = o;
    if (whence & AVSEEK_SIZE) return d->sz(d->opaque);
    int64_t r = d->sk(d->opaque, off, whence & ~AVSEEK_FORCE);
    if (r < 0) {
        d->io_failed = 1;
        return AVERROR(EIO);
    }
    return r;
}

void px_close(PxDec *d) {
    if (!d) return;
    swr_free(&d->swr);
    av_frame_free(&d->frm);
    av_packet_free(&d->pkt);
    avcodec_free_context(&d->cod);
    avformat_close_input(&d->fmt);
    if (d->io) {
        av_freep(&d->io->buffer);
        avio_context_free(&d->io);
    }
    av_freep(&d->out);
    free(d);
}

PxDec *px_open(void *opaque, px_read_fn rd, px_seek_fn sk, px_size_fn sz, int *err, int *io_failed) {
    int ret = 0;
    *io_failed = 0;
    av_log_set_level(AV_LOG_ERROR); /* libav's warnings are chatter, not errors */
    PxDec *d = calloc(1, sizeof *d);
    if (!d) { *err = AVERROR(ENOMEM); return NULL; }
    d->opaque = opaque; d->rd = rd; d->sk = sk; d->sz = sz;
    uint8_t *iobuf = av_malloc(IO_BUF);
    d->fmt = avformat_alloc_context();
    if (!iobuf || !d->fmt) { ret = AVERROR(ENOMEM); av_free(iobuf); goto fail; }
    d->io = avio_alloc_context(iobuf, IO_BUF, 0, d, read_cb, NULL, seek_cb);
    if (!d->io) { av_free(iobuf); ret = AVERROR(ENOMEM); goto fail; }
    d->fmt->pb = d->io;
    if ((ret = avformat_open_input(&d->fmt, "", NULL, NULL)) < 0) goto fail;
    if ((ret = avformat_find_stream_info(d->fmt, NULL)) < 0) goto fail;
    const AVCodec *codec = NULL;
    d->stream = av_find_best_stream(d->fmt, AVMEDIA_TYPE_AUDIO, -1, -1, &codec, 0);
    if (d->stream < 0) { ret = d->stream; goto fail; }
    for (unsigned i = 0; i < d->fmt->nb_streams; i++)
        if ((int)i != d->stream) d->fmt->streams[i]->discard = AVDISCARD_ALL;
    d->cod = avcodec_alloc_context3(codec);
    if (!d->cod) { ret = AVERROR(ENOMEM); goto fail; }
    if ((ret = avcodec_parameters_to_context(d->cod, d->fmt->streams[d->stream]->codecpar)) < 0) goto fail;
    if ((ret = avcodec_open2(d->cod, codec, NULL)) < 0) goto fail;
    d->is_aac = d->cod->codec_id == AV_CODEC_ID_AAC;
    d->rate = d->cod->sample_rate;
    d->src_channels = d->cod->ch_layout.nb_channels;
    if (d->rate <= 0 || d->src_channels <= 0) { ret = AVERROR_INVALIDDATA; goto fail; }
    d->pkt = av_packet_alloc();
    d->frm = av_frame_alloc();
    if (!d->pkt || !d->frm) { ret = AVERROR(ENOMEM); goto fail; }
    return d;
fail:
    *io_failed = d->io_failed;
    *err = ret;
    px_close(d);
    return NULL;
}

int px_sample_rate(const PxDec *d) { return d->rate; }
int px_source_channels(const PxDec *d) { return d->src_channels; }
int px_is_aac(const PxDec *d) { return d->is_aac; }

static int ensure_swr(PxDec *d, const AVFrame *f) {
    if (d->swr) return 0;
    AVChannelLayout out = AV_CHANNEL_LAYOUT_STEREO;
    AVChannelLayout in;
    if (f->ch_layout.order != AV_CHANNEL_ORDER_UNSPEC) {
        int r = av_channel_layout_copy(&in, &f->ch_layout);
        if (r < 0) return r;
    } else {
        av_channel_layout_default(&in, f->ch_layout.nb_channels);
    }
    int r = swr_alloc_set_opts2(&d->swr, &out, AV_SAMPLE_FMT_FLT, d->rate,
                                &in, (enum AVSampleFormat)f->format, f->sample_rate, 0, NULL);
    av_channel_layout_uninit(&in);
    if (r < 0) return r;
    return swr_init(d->swr);
}

static int ensure_out(PxDec *d, int frames) {
    if (frames <= d->out_cap) return 0;
    float *p = av_realloc_array(d->out, (size_t)frames, 2 * sizeof(float));
    if (!p) return AVERROR(ENOMEM);
    d->out = p; d->out_cap = frames;
    return 0;
}

/* Converts the decoded frame (or flushes swr when f == NULL). Returns frames produced or < 0. */
static int convert(PxDec *d, const AVFrame *f) {
    int in_n = f ? f->nb_samples : 0;
    int r;
    if (f && (r = ensure_swr(d, f)) < 0) return r;
    if (!d->swr) return 0;
    int cap = swr_get_out_samples(d->swr, in_n);
    if (cap < 0) return cap;
    cap += 64;
    if ((r = ensure_out(d, cap)) < 0) return r;
    uint8_t *outp = (uint8_t *)d->out;
    return swr_convert(d->swr, &outp, cap, f ? (const uint8_t **)f->extended_data : NULL, in_n);
}

/* Applies the post-seek lead-in drop to `n` converted frames; returns how many leading frames to
 * drop. The decoder is warmed up on a pre-roll before the target (see px_seek_ms), possibly across
 * several frames, so the position is tracked until the target is reached. */
static int drop_lead(PxDec *d, const AVFrame *f, int n) {
    if (!d->seek_pending) return 0;
    AVRational tb = d->fmt->streams[d->stream]->time_base;
    if (d->cursor < 0) {
        if (!f || f->pts == AV_NOPTS_VALUE) { d->seek_pending = 0; return 0; }
        int64_t start = av_rescale_q(f->pts, tb, (AVRational){1, d->rate});
        /* After a seek libav re-applies the codec's start skip (Opus pre-skip) to the first packet:
         * the frame comes back shorter than the packet's duration but keeps the packet's pts, so
         * the audio really begins that many samples later. */
        if (f->duration > 0) {
            int64_t dur = av_rescale_q(f->duration, tb, (AVRational){1, d->rate});
            if (dur > f->nb_samples) start += dur - f->nb_samples;
        }
        d->cursor = start;
    }
    int64_t skip = d->seek_target - d->cursor;
    int drop = skip <= 0 ? 0 : (skip > n ? n : (int)skip);
    d->cursor += n;
    if (d->cursor >= d->seek_target) d->seek_pending = 0;
    return drop;
}

int px_read(PxDec *d, const float **samples, int *frames, int *io_failed) {
    for (;;) {
        int r = avcodec_receive_frame(d->cod, d->frm);
        if (r >= 0) {
            int n = convert(d, d->frm);
            int first = n > 0 ? drop_lead(d, d->frm, n) : 0;
            av_frame_unref(d->frm);
            if (n < 0) { *io_failed = d->io_failed; return n; }
            if (n - first <= 0) continue;
            *samples = d->out + (size_t)first * 2;
            *frames = n - first;
            return 1;
        }
        if (r == AVERROR_EOF) {
            if (!d->swr_flushed) {
                d->swr_flushed = 1;
                int n = convert(d, NULL);
                if (n > 0) { *samples = d->out; *frames = n; return 1; }
            }
            return 0;
        }
        if (r != AVERROR(EAGAIN)) { *io_failed = d->io_failed; return r; }
        if (d->sent_eof) return 0;
        r = av_read_frame(d->fmt, d->pkt);
        if (r == AVERROR_EOF) {
            d->sent_eof = 1;
            avcodec_send_packet(d->cod, NULL);
            continue;
        }
        if (r < 0) { *io_failed = d->io_failed; return r; }
        if (d->pkt->stream_index != d->stream) { av_packet_unref(d->pkt); continue; }
        r = avcodec_send_packet(d->cod, d->pkt);
        av_packet_unref(d->pkt);
        if (r == AVERROR_INVALIDDATA) continue; /* skip a corrupt packet */
        if (r < 0 && r != AVERROR(EAGAIN)) { *io_failed = d->io_failed; return r; }
    }
}

int px_seek_ms(PxDec *d, int64_t ms) {
    AVStream *st = d->fmt->streams[d->stream];
    AVRational tb = st->time_base;
    int64_t target = av_rescale(ms, d->rate, 1000);
    /* Start the decoder `seek_preroll` samples early (Opus: 80 ms) so it is warmed up at the target. */
    int64_t from = target - st->codecpar->seek_preroll;
    if (from < 0) from = 0;
    int64_t ts = av_rescale_q(from, (AVRational){1, d->rate}, tb);
    int r = av_seek_frame(d->fmt, d->stream, ts, AVSEEK_FLAG_BACKWARD);
    if (r < 0) return r;
    avcodec_flush_buffers(d->cod);
    swr_free(&d->swr);
    d->sent_eof = 0;
    d->swr_flushed = 0;
    d->seek_pending = 1;
    d->seek_target = target;
    d->cursor = -1;
    return 0;
}

void px_strerror(int err, char *buf, int len) { av_strerror(err, buf, len); }
int px_err_decoder_not_found(void) { return AVERROR_DECODER_NOT_FOUND; }
int px_err_invaliddata(void) { return AVERROR_INVALIDDATA; }
