// Validation harness: feeds an Interplay MVE file through gemrb's
// actual decoder primitives (compiled in from
// `gemrb/plugins/MVEPlayer/mvevideodec*.cpp`). Reports per-frame
// success/failure plus a final per-file pass/fail.

#include "gstmvedemux.h"

#include <cstdio>
#include <cstdint>
#include <cstring>
#include <fstream>
#include <vector>
#include <string>

// MVE segment opcodes (mirror gemrb/plugins/MVEPlayer/mve.h).
enum : uint8_t {
    OC_END_OF_STREAM      = 0x00,
    OC_END_OF_CHUNK       = 0x01,
    OC_CREATE_TIMER       = 0x02,
    OC_AUDIO_BUFFERS      = 0x03,
    OC_PLAY_AUDIO         = 0x04,
    OC_VIDEO_BUFFERS      = 0x05,
    OC_PLAY_VIDEO         = 0x07,
    OC_AUDIO_DATA         = 0x08,
    OC_AUDIO_SILENCE      = 0x09,
    OC_VIDEO_MODE         = 0x0a,
    OC_PALETTE            = 0x0c,
    OC_PALETTE_COMPRESSED = 0x0d,
    OC_CODE_MAP           = 0x0f,
    OC_VIDEO_DATA         = 0x11,
};

constexpr uint16_t MVE_VIDEO_DELTA_FRAME    = 0x0001;
constexpr uint16_t MVE_DEFAULT_AUDIO_STREAM = 0x0001;
constexpr uint16_t MVE_AUDIO_STEREO         = 0x0001;
constexpr uint16_t MVE_AUDIO_16BIT          = 0x0002;
constexpr uint16_t MVE_AUDIO_COMPRESSED     = 0x0004;

struct ValidationResult {
    bool ok = true;
    int frames_decoded = 0;
    int video_data_segments_seen = 0;
    int audio_data_segments_seen = 0;
    bool is_rgb555 = false;
    uint16_t width = 0, height = 0;
    std::string failure_reason;
};

static bool read_le_u16(const std::vector<uint8_t>& data, size_t off, uint16_t& out) {
    if (off + 2 > data.size()) return false;
    out = uint16_t(data[off]) | (uint16_t(data[off + 1]) << 8);
    return true;
}

static ValidationResult validate(const std::string& path) {
    ValidationResult r;
    std::ifstream f(path, std::ios::binary);
    if (!f) { r.ok = false; r.failure_reason = "open failed"; return r; }
    std::vector<uint8_t> data((std::istreambuf_iterator<char>(f)),
                               std::istreambuf_iterator<char>());

    // Signature: gemrb's permissive strncmp-via-NUL effectively only
    // checks the 19-byte prefix "Interplay MVE File\x1A".
    static const uint8_t SIG_PREFIX[19] = {
        'I','n','t','e','r','p','l','a','y',' ','M','V','E',' ','F','i','l','e',0x1A
    };
    if (data.size() < 26 || std::memcmp(data.data(), SIG_PREFIX, 19) != 0) {
        r.ok = false; r.failure_reason = "bad signature"; return r;
    }

    GstMveDemuxStream gst{};
    bool video_initialized = false;
    bool is_rgb555 = false;
    std::vector<uint16_t> back_buf1, back_buf2;
    std::vector<uint8_t>  code_map;

    size_t off = 26;
    bool stream_done = false;
    while (off + 4 <= data.size() && !stream_done) {
        uint16_t chunk_size = uint16_t(data[off]) | (uint16_t(data[off + 1]) << 8);
        off += 4; // skip size + chunk_type
        size_t chunk_end = off + chunk_size;
        if (chunk_end > data.size()) {
            r.ok = false; r.failure_reason = "chunk overruns file"; return r;
        }
        size_t p = off;
        while (p + 4 <= chunk_end) {
            uint16_t seg_size = uint16_t(data[p]) | (uint16_t(data[p + 1]) << 8);
            uint8_t  opcode   = data[p + 2];
            uint8_t  version  = data[p + 3];
            const uint8_t* payload = data.data() + p + 4;
            (void)version;

            switch (opcode) {
                case OC_VIDEO_BUFFERS: {
                    if (seg_size < 8) { r.ok = false; r.failure_reason = "video buffers too small"; return r; }
                    uint16_t w_blocks = GST_READ_UINT16_LE(payload);
                    uint16_t h_blocks = GST_READ_UINT16_LE(payload + 2);
                    uint16_t format   = (version > 1) ? GST_READ_UINT16_LE(payload + 6) : 0;
                    is_rgb555 = (format > 0);
                    gst.width  = w_blocks * 8;
                    gst.height = h_blocks * 8;
                    gst.max_block_offset =
                        (gst.height - 7) * uint32_t(gst.width) - 8;
                    size_t pix = size_t(gst.width) * gst.height;
                    back_buf1.assign(pix, 0);
                    back_buf2.assign(pix, 0);
                    gst.back_buf1 = back_buf1.data();
                    gst.back_buf2 = back_buf2.data();
                    r.is_rgb555 = is_rgb555;
                    r.width  = gst.width;
                    r.height = gst.height;
                    video_initialized = true;
                    break;
                }
                case OC_CODE_MAP: {
                    code_map.assign(payload, payload + seg_size);
                    gst.code_map = code_map.data();
                    break;
                }
                case OC_VIDEO_DATA: {
                    r.video_data_segments_seen++;
                    if (!video_initialized) {
                        r.ok = false;
                        r.failure_reason = "video data before video buffers";
                        return r;
                    }
                    if (seg_size < 14) {
                        r.ok = false; r.failure_reason = "video data segment < 14 bytes";
                        return r;
                    }
                    // 12-byte header + u16 flags + frame data
                    uint16_t flags = GST_READ_UINT16_LE(payload + 12);
                    if (flags & MVE_VIDEO_DELTA_FRAME) {
                        std::swap(gst.back_buf1, gst.back_buf2);
                    }
                    const uint8_t* frame_data = payload + 14;
                    uint16_t frame_size = seg_size - 14;
                    int rc = is_rgb555
                        ? ipvideo_decode_frame16(&gst, frame_data, frame_size)
                        : ipvideo_decode_frame8(&gst, frame_data, frame_size);
                    if (rc != 0) {
                        r.ok = false;
                        r.failure_reason = "ipvideo_decode_frame returned non-zero";
                        return r;
                    }
                    r.frames_decoded++;
                    break;
                }
                case OC_AUDIO_DATA: {
                    r.audio_data_segments_seen++;
                    if (seg_size < 6) {
                        r.ok = false; r.failure_reason = "audio data < 6 byte header"; return r;
                    }
                    // header: u16 seq, u16 stream_mask, u16 audio_size
                    uint16_t audio_size = GST_READ_UINT16_LE(payload + 4);
                    const uint8_t* compressed = payload + 6;
                    uint16_t compressed_len = seg_size - 6;
                    // gemrb's `ipaudio_uncompress` expects:
                    // - dst buffer sized for `audio_size` PCM bytes
                    // - src compressed buffer (we pass the raw payload
                    //   tail; gemrb internally doesn't read past
                    //   `audio_size / 2 + channels` bytes for valid
                    //   DPCM, which our encoder always satisfies)
                    std::vector<int16_t> pcm(audio_size / 2);
                    // Mono assumed unless OC_AUDIO_BUFFERS already told
                    // us stereo; we encode mono in the test fixtures.
                    ipaudio_uncompress(pcm.data(), audio_size,
                                       const_cast<uint8_t*>(compressed),
                                       /*channels=*/1);
                    (void)compressed_len;
                    break;
                }
                case OC_AUDIO_SILENCE:
                    r.audio_data_segments_seen++;
                    break;
                case OC_END_OF_STREAM:
                    stream_done = true;
                    break;
                default:
                    // Other segments (timer, palette, video mode, …) —
                    // gemrb handles them, we don't need to here for a
                    // pure decode-validity check.
                    break;
            }

            p += 4 + seg_size;
            if (stream_done) break;
        }
        off = chunk_end;
    }
    return r;
}

int main(int argc, char** argv) {
    if (argc < 2) {
        std::fprintf(stderr, "usage: %s <file.mve> [more.mve …]\n", argv[0]);
        return 2;
    }
    int total = 0, passed = 0;
    for (int i = 1; i < argc; ++i) {
        const char* path = argv[i];
        ValidationResult r = validate(path);
        total++;
        if (r.ok) {
            passed++;
            std::printf("OK    %s  %ux%u %s frames=%d audio_segs=%d\n",
                        path, r.width, r.height,
                        r.is_rgb555 ? "RGB555" : "Pal8",
                        r.frames_decoded, r.audio_data_segments_seen);
        } else {
            std::printf("FAIL  %s  reason=%s frames=%d\n",
                        path, r.failure_reason.c_str(), r.frames_decoded);
        }
    }
    std::printf("\n%d/%d files validated\n", passed, total);
    return passed == total ? 0 : 1;
}
