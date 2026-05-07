// Stub of gemrb's gstmvedemux.h — re-implements the same surface used
// by mvevideodec8.cpp / mvevideodec16.cpp / mveaudiodec.cpp without
// pulling in any of the gemrb engine. Logging goes to stderr.

#ifndef __GST_MVE_DEMUX_H__
#define __GST_MVE_DEMUX_H__

#include <cstdio>
#include <cstddef>
#include <cstdint>
#include <cstring>

using std::ptrdiff_t;

using ieByte  = unsigned char;
using ieWord  = uint16_t;
using ieDword = uint32_t;

#define G_UNLIKELY(x) (x)

// Logging stubs — print to stderr so we can see warnings/errors during
// validation. Format strings here use printf semantics, while gemrb's
// real Log() is fmt-style. We rely on the substitution being trivial
// (%d / %ld / %u / %x).
inline void GST_WARNING(const char* msg) {
    std::fprintf(stderr, "[gemrb-mve WARN] %s\n", msg);
}
template <typename A>
inline void GST_ERROR(const char* fmt, A a) {
    std::fprintf(stderr, "[gemrb-mve ERR] ");
    std::fprintf(stderr, fmt, a);
    std::fprintf(stderr, "\n");
}
template <typename A, typename B>
inline void GST_ERROR2(const char* fmt, A a, B b) {
    std::fprintf(stderr, "[gemrb-mve ERR] ");
    std::fprintf(stderr, fmt, a, b);
    std::fprintf(stderr, "\n");
}

#define _GST_GET(__data, __idx, __size, __shift) \
    (((uint##__size##_t)(((uint8_t*) (__data))[__idx])) << __shift)

#define GST_READ_UINT16_LE(data) \
    (_GST_GET(data, 1, 16, 8) | _GST_GET(data, 0, 16, 0))

#define GST_READ_UINT32_LE(data) \
    (_GST_GET(data, 3, 32, 24) | _GST_GET(data, 2, 32, 16) \
   | _GST_GET(data, 1, 32, 8)  | _GST_GET(data, 0, 32, 0))

typedef struct _GstMveDemuxStream GstMveDemuxStream;
using gint = int;
using gboolean = gint;
using guint8 = ieByte;
using guint16 = ieWord;
using guint32 = ieDword;

struct _GstMveDemuxStream {
    guint16 width;
    guint16 height;
    guint8* code_map;
    guint16* back_buf1;
    guint16* back_buf2;
    guint32 max_block_offset;
};

int  ipvideo_decode_frame8 (const GstMveDemuxStream*, const unsigned char*, unsigned short);
int  ipvideo_decode_frame16(const GstMveDemuxStream*, const unsigned char*, unsigned short);
void ipaudio_uncompress(short int*, unsigned short, const unsigned char*, unsigned char);

#endif
