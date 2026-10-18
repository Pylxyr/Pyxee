/* Thin decode shim over the minimal static libav* build. Every libav struct is touched only in
 * pxdec.c, against the real headers, so Rust never mirrors a libav struct layout.
 * Output is always interleaved stereo f32 at the stream's own sample rate. */
#ifndef PXDEC_H
#define PXDEC_H
#include <stdint.h>

typedef struct PxDec PxDec;

/* Return > 0 bytes read, 0 at end of stream, < 0 on an I/O error (the Rust side keeps the details). */
typedef int (*px_read_fn)(void *opaque, uint8_t *buf, int len);
/* whence is SEEK_SET/SEEK_CUR/SEEK_END (0/1/2). Returns the new position, or -1. */
typedef int64_t (*px_seek_fn)(void *opaque, int64_t offset, int whence);
/* Total length in bytes, or -1 if unknown. */
typedef int64_t (*px_size_fn)(void *opaque);

/* Opens and probes by content. NULL on failure, with a libav error code in *err (see px_strerror)
 * and *io_failed set when the failure came from the read/seek callbacks. */
PxDec *px_open(void *opaque, px_read_fn rd, px_seek_fn sk, px_size_fn sz, int *err, int *io_failed);

int px_sample_rate(const PxDec *d);
int px_source_channels(const PxDec *d);
/* 1 for AAC streams, whose containers often carry no (or incomplete) encoder delay/padding data. */
int px_is_aac(const PxDec *d);

/* 1 = *samples points at *frames interleaved stereo frames, valid until the next call.
 * 0 = end of stream. < 0 = error (libav code); *io_failed set if a callback failed. */
int px_read(PxDec *d, const float **samples, int *frames, int *io_failed);

/* Seeks to `ms` (sample-accurate: the decoder lands on or before it, the lead-in is dropped). */
int px_seek_ms(PxDec *d, int64_t ms);

void px_close(PxDec *d);
void px_strerror(int err, char *buf, int len);
int px_err_decoder_not_found(void);
int px_err_invaliddata(void);
#endif
