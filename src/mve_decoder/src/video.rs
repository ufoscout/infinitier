use crate::error::Error;

// ---------------------------------------------------------------------------
// Shared block-copy helpers
// ---------------------------------------------------------------------------

/// Copy an 8×8 block within a single buffer (same buffer, different positions).
/// Used by opcodes 0x2 and 0x3 (current-frame motion vectors).
fn copy_block_within(buf: &mut [u8], dst: usize, src: usize, stride: usize, bpp: usize) -> Result<(), Error> {
    for row in 0..8usize {
        let d = dst + row * stride * bpp;
        let s = src + row * stride * bpp;
        let end_s = s + 8 * bpp;
        let end_d = d + 8 * bpp;
        if end_s > buf.len() || end_d > buf.len() {
            return Err(Error::VideoDecode(format!(
                "copy_block_within out of bounds: src={s}..{end_s} dst={d}..{end_d} len={}",
                buf.len()
            )));
        }
        buf.copy_within(s..end_s, d);
    }
    Ok(())
}

/// Copy an 8×8 block from `src_buf` into `dst_buf`.
/// Used by opcodes 0x0, 0x4, 0x5 (previous-frame copies).
fn copy_block(
    dst_buf: &mut [u8],
    src_buf: &[u8],
    dst: usize,
    src: usize,
    stride: usize,
    bpp: usize,
) -> Result<(), Error> {
    for row in 0..8usize {
        let d = dst + row * stride * bpp;
        let s = src + row * stride * bpp;
        let end_s = s + 8 * bpp;
        let end_d = d + 8 * bpp;
        if end_s > src_buf.len() || end_d > dst_buf.len() {
            return Err(Error::VideoDecode(format!(
                "copy_block out of bounds: src={s}..{end_s} dst={d}..{end_d}"
            )));
        }
        dst_buf[d..end_d].copy_from_slice(&src_buf[s..end_s]);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// 8-bit video decoder
// ---------------------------------------------------------------------------

struct Reader8<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader8<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn read_byte(&mut self) -> Result<u8, Error> {
        if self.pos >= self.data.len() {
            return Err(Error::VideoDecode("unexpected end of video data".into()));
        }
        let b = self.data[self.pos];
        self.pos += 1;
        Ok(b)
    }

    fn read_i8(&mut self) -> Result<i8, Error> {
        self.read_byte().map(|b| b as i8)
    }
}

fn decode8_0x2(
    r: &mut Reader8<'_>,
    buf: &mut [u8],
    dst: usize,
    width: usize,
) -> Result<(), Error> {
    let b = r.read_byte()? as i32;
    let (x, y) = if b < 56 {
        (8 + b % 7, b / 7)
    } else {
        (-14 + (b - 56) % 29, 8 + (b - 56) / 29)
    };
    let offset = (y * width as i32 + x) as isize;
    let src = (dst as isize + offset) as usize;
    copy_block_within(buf, dst, src, width, 1)
}

fn decode8_0x3(
    r: &mut Reader8<'_>,
    buf: &mut [u8],
    dst: usize,
    width: usize,
) -> Result<(), Error> {
    let b = r.read_byte()? as i32;
    let (x, y) = if b < 56 {
        (-(8 + b % 7), -(b / 7))
    } else {
        (-(-14 + (b - 56) % 29), -(8 + (b - 56) / 29))
    };
    let offset = (y * width as i32 + x) as isize;
    let src = (dst as isize + offset) as usize;
    copy_block_within(buf, dst, src, width, 1)
}

fn decode8_0x4(
    r: &mut Reader8<'_>,
    buf1: &mut [u8],
    buf2: &[u8],
    dst: usize,
    width: usize,
) -> Result<(), Error> {
    let b = r.read_byte()? as i32;
    let x = -8 + (b & 0x0f);
    let y = -8 + (b >> 4);
    let offset = (y * width as i32 + x) as isize;
    let src = (dst as isize + offset) as usize;
    copy_block(buf1, buf2, dst, src, width, 1)
}

fn decode8_0x5(
    r: &mut Reader8<'_>,
    buf1: &mut [u8],
    buf2: &[u8],
    dst: usize,
    width: usize,
) -> Result<(), Error> {
    let x = r.read_i8()? as i32;
    let y = r.read_i8()? as i32;
    let offset = (y * width as i32 + x) as isize;
    let src = (dst as isize + offset) as usize;
    copy_block(buf1, buf2, dst, src, width, 1)
}

fn decode8_0x7(
    r: &mut Reader8<'_>,
    buf: &mut [u8],
    dst: usize,
    width: usize,
) -> Result<(), Error> {
    let p0 = r.read_byte()?;
    let p1 = r.read_byte()?;

    if p0 <= p1 {
        // Per-row 8-bit mask
        for y in 0..8usize {
            let flags = r.read_byte()?;
            for x in 0..8usize {
                buf[dst + y * width + x] = if flags & (1 << x) != 0 { p1 } else { p0 };
            }
        }
    } else {
        // Per-2×2-block mask (16 bits)
        let flags = r.read_byte()? as u16 | ((r.read_byte()? as u16) << 8);
        let mut mask = 1u16;
        let mut y = 0;
        while y < 8 {
            let mut x = 0;
            while x < 8 {
                let px = if flags & mask != 0 { p1 } else { p0 };
                buf[dst + y * width + x] = px;
                buf[dst + y * width + x + 1] = px;
                buf[dst + (y + 1) * width + x] = px;
                buf[dst + (y + 1) * width + x + 1] = px;
                mask <<= 1;
                x += 2;
            }
            y += 2;
        }
    }
    Ok(())
}

fn decode8_0x8(
    r: &mut Reader8<'_>,
    buf: &mut [u8],
    dst: usize,
    width: usize,
) -> Result<(), Error> {
    let p: [u8; 8];
    let b: [u8; 8];

    let p0 = r.read_byte()?;
    let p1 = r.read_byte()?;
    let b0 = r.read_byte()?;
    let b1 = r.read_byte()?;

    if p0 <= p1 {
        // 4 quadrants, 2 colours each — read 12 more bytes interleaved (P2,P3,B2,B3,P4,P5,B4,B5,P6,P7,B6,B7)
        let (p2, p3, b2, b3) = (r.read_byte()?, r.read_byte()?, r.read_byte()?, r.read_byte()?);
        let (p4, p5, b4, b5) = (r.read_byte()?, r.read_byte()?, r.read_byte()?, r.read_byte()?);
        let (p6, p7, b6, b7) = (r.read_byte()?, r.read_byte()?, r.read_byte()?, r.read_byte()?);
        p = [p0, p1, p2, p3, p4, p5, p6, p7];
        b = [b0, b1, b2, b3, b4, b5, b6, b7];

        let mut lower_half;
        for y in 0..8usize {
            if y == 4 {
                let flags = pack_flags_8(&b, 2, 3, 6, 7);
                let _ = flags; // recomputed below
            }
            lower_half = if y >= 4 { 2usize } else { 0usize };
            let flags = pack_flags_8(&b, lower_half, lower_half + 1, lower_half + 4, lower_half + 5);
            let mut pp0 = p[lower_half];
            let mut pp1 = p[lower_half + 1];
            for x in 0..8usize {
                if x == 4 {
                    pp0 = p[lower_half + 4];
                    pp1 = p[lower_half + 5];
                }
                buf[dst + y * width + x] = if flags & (1 << (y % 4 * 8 + x)) != 0 { pp1 } else { pp0 };
            }
        }
    } else {
        // 2 halves, vertical or horizontal split
        let b2 = r.read_byte()?;
        let b3 = r.read_byte()?;
        let p2 = r.read_byte()?;
        let p3 = r.read_byte()?;
        let b4 = r.read_byte()?;
        let b5 = r.read_byte()?;
        let b6 = r.read_byte()?;
        let b7 = r.read_byte()?;

        p = [p0, p1, p2, p3, 0, 0, 0, 0];
        b = [b0, b1, b2, b3, b4, b5, b6, b7];

        if p2 <= p3 {
            // Vertical split (left/right halves)
            for y in 0..8usize {
                let flags = pack_flags_8(&b, y / 4 * 2, y / 4 * 2 + 1, y / 4 * 2 + 4, y / 4 * 2 + 5);
                let mut pp0 = p[0];
                let mut pp1 = p[1];
                for x in 0..8usize {
                    if x == 4 {
                        pp0 = p[2];
                        pp1 = p[3];
                    }
                    buf[dst + y * width + x] = if flags & (1 << (y % 4 * 8 + x)) != 0 { pp1 } else { pp0 };
                }
            }
        } else {
            // Horizontal split (top/bottom halves)
            let mut pp0 = p0;
            let mut pp1 = p1;
            for y in 0..8usize {
                let flags = b[y];
                if y == 4 {
                    pp0 = p2;
                    pp1 = p3;
                }
                for x in 0..8usize {
                    buf[dst + y * width + x] = if flags & (1 << x) != 0 { pp1 } else { pp0 };
                }
            }
        }
    }
    Ok(())
}

/// Pack the quadrant flags into a 32-bit value matching the C++ layout.
fn pack_flags_8(b: &[u8], i0: usize, i1: usize, i4: usize, i5: usize) -> u32 {
    ((b[i0] & 0xf0) as u32) << 4
        | ((b[i4] & 0xf0) as u32) << 8
        | ((b[i0] & 0x0f) as u32)
        | ((b[i4] & 0x0f) as u32) << 4
        | ((b[i1] & 0xf0) as u32) << 20
        | ((b[i5] & 0xf0) as u32) << 24
        | ((b[i1] & 0x0f) as u32) << 16
        | ((b[i5] & 0x0f) as u32) << 20
}

fn decode8_0x9(
    r: &mut Reader8<'_>,
    buf: &mut [u8],
    dst: usize,
    width: usize,
) -> Result<(), Error> {
    let p: [u8; 4] = [r.read_byte()?, r.read_byte()?, r.read_byte()?, r.read_byte()?];

    if p[0] <= p[1] && p[2] <= p[3] {
        // 4 colours per pixel
        for y in 0..8usize {
            let flags = r.read_byte()? as u16 | ((r.read_byte()? as u16) << 8);
            for x in 0..8usize {
                buf[dst + y * width + x] = p[((flags >> (x * 2)) & 0x03) as usize];
            }
        }
    } else if p[0] <= p[1] {
        // 4 colours per 2×2 block
        let flags = r.read_byte()? as u32
            | ((r.read_byte()? as u32) << 8)
            | ((r.read_byte()? as u32) << 16)
            | ((r.read_byte()? as u32) << 24);
        let mut shifter = 0;
        let mut y = 0;
        while y < 8 {
            let mut x = 0;
            while x < 8 {
                let px = p[((flags >> shifter) & 0x03) as usize];
                buf[dst + y * width + x] = px;
                buf[dst + y * width + x + 1] = px;
                buf[dst + (y + 1) * width + x] = px;
                buf[dst + (y + 1) * width + x + 1] = px;
                shifter += 2;
                x += 2;
            }
            y += 2;
        }
    } else if p[2] <= p[3] {
        // 4 colours per 2×1 block (wide): reload flags every 4 rows
        let mut y = 0;
        while y < 8 {
            let flags = r.read_byte()? as u32
                | ((r.read_byte()? as u32) << 8)
                | ((r.read_byte()? as u32) << 16)
                | ((r.read_byte()? as u32) << 24);
            let mut shifter = 0;
            for dy in 0..4usize {
                let mut x = 0;
                while x < 8 {
                    let px = p[((flags >> shifter) & 0x03) as usize];
                    buf[dst + (y + dy) * width + x] = px;
                    buf[dst + (y + dy) * width + x + 1] = px;
                    shifter += 2;
                    x += 2;
                }
            }
            y += 4;
        }
    } else {
        // 4 colours per 1×2 block (tall)
        let mut y = 0;
        while y < 8 {
            let flags = r.read_byte()? as u32
                | ((r.read_byte()? as u32) << 8)
                | ((r.read_byte()? as u32) << 16)
                | ((r.read_byte()? as u32) << 24);
            let mut shifter = 0;
            let mut dy = 0;
            while dy < 4 {
                for x in 0..8usize {
                    let px = p[((flags >> shifter) & 0x03) as usize];
                    buf[dst + (y + dy) * width + x] = px;
                    buf[dst + (y + dy + 1) * width + x] = px;
                    shifter += 2;
                }
                dy += 2;
            }
            y += 4;
        }
    }
    Ok(())
}

fn decode8_0xa(
    r: &mut Reader8<'_>,
    buf: &mut [u8],
    dst: usize,
    width: usize,
) -> Result<(), Error> {
    let p0 = r.read_byte()?;
    let p1 = r.read_byte()?;
    let p2 = r.read_byte()?;
    let p3 = r.read_byte()?;
    let b0 = r.read_byte()?;
    let b1 = r.read_byte()?;
    let b2 = r.read_byte()?;
    let b3 = r.read_byte()?;

    if p0 <= p1 {
        // 4-colour per quadrant — 4 sets of [P0..P3, B0..B3]
        let mut p = [p0, p1, p2, p3, 0u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let mut b = [b0, b1, b2, b3, 0u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        for chunk in 1..4usize {
            for i in 0..4 { p[chunk * 4 + i] = r.read_byte()?; }
            for i in 0..4 { b[chunk * 4 + i] = r.read_byte()?; }
        }
        for y in 0..8usize {
            let lower = if y >= 4 { 4usize } else { 0usize };
            let flags = (b[y + 8] as u16) << 8 | b[y] as u16;
            for x in 0..8usize {
                let split = if x >= 4 { 8usize } else { 0usize };
                let idx = split + lower + ((flags >> (x * 2)) & 0x03) as usize;
                buf[dst + y * width + x] = p[idx];
            }
        }
    } else {
        // 4-colour for left/right or top/bottom halves
        let mut b = [b0, b1, b2, b3, r.read_byte()?, r.read_byte()?, r.read_byte()?, r.read_byte()?,
                     0u8, 0, 0, 0, 0, 0, 0, 0];
        let p4 = r.read_byte()?;
        let p5 = r.read_byte()?;
        let p6 = r.read_byte()?;
        let p7 = r.read_byte()?;
        for i in 8..16 { b[i] = r.read_byte()?; }
        let p = [p0, p1, p2, p3, p4, p5, p6, p7, 0u8, 0, 0, 0, 0, 0, 0, 0];

        if p4 <= p5 {
            // Left/right halves
            for y in 0..8usize {
                let flags = (b[y + 8] as u16) << 8 | b[y] as u16;
                for x in 0..8usize {
                    let split = if x >= 4 { 4usize } else { 0usize };
                    buf[dst + y * width + x] = p[split + ((flags >> (x * 2)) & 0x03) as usize];
                }
            }
        } else {
            // Top/bottom halves
            for y in 0..8usize {
                let flags = (b[y * 2 + 1] as u16) << 8 | b[y * 2] as u16;
                let split = if y >= 4 { 4usize } else { 0usize };
                for x in 0..8usize {
                    buf[dst + y * width + x] = p[split + ((flags >> (x * 2)) & 0x03) as usize];
                }
            }
        }
    }
    Ok(())
}

fn decode8_0xb(
    r: &mut Reader8<'_>,
    buf: &mut [u8],
    dst: usize,
    width: usize,
) -> Result<(), Error> {
    for y in 0..8usize {
        for x in 0..8usize {
            buf[dst + y * width + x] = r.read_byte()?;
        }
    }
    Ok(())
}

fn decode8_0xc(
    r: &mut Reader8<'_>,
    buf: &mut [u8],
    dst: usize,
    width: usize,
) -> Result<(), Error> {
    let mut y = 0;
    while y < 8 {
        let mut x = 0;
        while x < 8 {
            let px = r.read_byte()?;
            buf[dst + y * width + x] = px;
            buf[dst + y * width + x + 1] = px;
            buf[dst + (y + 1) * width + x] = px;
            buf[dst + (y + 1) * width + x + 1] = px;
            x += 2;
        }
        y += 2;
    }
    Ok(())
}

fn decode8_0xd(
    r: &mut Reader8<'_>,
    buf: &mut [u8],
    dst: usize,
    width: usize,
) -> Result<(), Error> {
    let p: [u8; 4] = [r.read_byte()?, r.read_byte()?, r.read_byte()?, r.read_byte()?];
    for y in 0..8usize {
        let base = if y < 4 { 0usize } else { 2 };
        for x in 0..8usize {
            let idx = base + if x >= 4 { 1 } else { 0 };
            buf[dst + y * width + x] = p[idx];
        }
    }
    Ok(())
}

fn decode8_0xe(
    r: &mut Reader8<'_>,
    buf: &mut [u8],
    dst: usize,
    width: usize,
) -> Result<(), Error> {
    let px = r.read_byte()?;
    for y in 0..8usize {
        for x in 0..8usize {
            buf[dst + y * width + x] = px;
        }
    }
    Ok(())
}

fn decode8_0xf(
    r: &mut Reader8<'_>,
    buf: &mut [u8],
    dst: usize,
    width: usize,
) -> Result<(), Error> {
    let p = [r.read_byte()?, r.read_byte()?];
    for y in 0..8usize {
        for x in 0..8usize {
            buf[dst + y * width + x] = p[(y ^ x) & 1];
        }
    }
    Ok(())
}

/// Decode one 8-bit paletted video frame into `buf1`.
/// `buf2` holds the previous frame.
pub fn decode_frame8(
    buf1: &mut Vec<u8>,
    buf2: &mut Vec<u8>,
    code_map: &[u8],
    data: &[u8],
    width: u16,
    height: u16,
) -> Result<(), Error> {
    let w = width as usize;
    let h = height as usize;
    let mut r = Reader8::new(data);
    let mut code_idx = 0usize;

    let bx_count = w >> 3;
    let by_count = h >> 3;

    for by in 0..by_count {
        for bx in 0..bx_count {
            let opcode = if code_idx & 1 == 0 {
                code_map[code_idx >> 1] & 0x0f
            } else {
                code_map[code_idx >> 1] >> 4
            };
            code_idx += 1;

            let dst = by * 8 * w + bx * 8;

            match opcode {
                0x0 => copy_block(buf1, buf2, dst, dst, w, 1)?,
                0x1 => {} // keep existing pixels (already from 2 frames ago)
                0x2 => decode8_0x2(&mut r, buf1, dst, w)?,
                0x3 => decode8_0x3(&mut r, buf1, dst, w)?,
                0x4 => decode8_0x4(&mut r, buf1, buf2, dst, w)?,
                0x5 => decode8_0x5(&mut r, buf1, buf2, dst, w)?,
                0x6 => return Err(Error::VideoDecode("unsupported opcode 0x6".into())),
                0x7 => decode8_0x7(&mut r, buf1, dst, w)?,
                0x8 => decode8_0x8(&mut r, buf1, dst, w)?,
                0x9 => decode8_0x9(&mut r, buf1, dst, w)?,
                0xa => decode8_0xa(&mut r, buf1, dst, w)?,
                0xb => decode8_0xb(&mut r, buf1, dst, w)?,
                0xc => decode8_0xc(&mut r, buf1, dst, w)?,
                0xd => decode8_0xd(&mut r, buf1, dst, w)?,
                0xe => decode8_0xe(&mut r, buf1, dst, w)?,
                0xf => decode8_0xf(&mut r, buf1, dst, w)?,
                _ => unreachable!(),
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// 16-bit video decoder
// ---------------------------------------------------------------------------

/// Read a little-endian u16 from a byte slice at the given position.
#[inline]
fn read_u16_le(data: &[u8], pos: usize) -> u16 {
    u16::from_le_bytes([data[pos], data[pos + 1]])
}

struct Reader16<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader16<'a> {
    fn new(data: &'a [u8], start: usize) -> Self {
        Self { data, pos: start }
    }

    fn read_byte(&mut self) -> Result<u8, Error> {
        if self.pos >= self.data.len() {
            return Err(Error::VideoDecode("unexpected end of 16-bit video data".into()));
        }
        let b = self.data[self.pos];
        self.pos += 1;
        Ok(b)
    }

    fn read_u16(&mut self) -> Result<u16, Error> {
        if self.pos + 2 > self.data.len() {
            return Err(Error::VideoDecode("unexpected end of 16-bit video data (u16)".into()));
        }
        let v = read_u16_le(self.data, self.pos);
        self.pos += 2;
        Ok(v)
    }

    fn read_i8(&mut self) -> Result<i8, Error> {
        self.read_byte().map(|b| b as i8)
    }
}

// For 16-bit, the "frame" buffer is indexed in u16 units but stored as &[u8].
// dst and src are u16-pixel offsets; we convert to byte offsets with *2.
const BPP16: usize = 2;

fn copy_block16_within(buf: &mut [u8], dst_px: usize, src_px: usize, stride: usize) -> Result<(), Error> {
    copy_block_within(buf, dst_px * BPP16, src_px * BPP16, stride, BPP16)
}

fn copy_block16(
    dst_buf: &mut [u8],
    src_buf: &[u8],
    dst_px: usize,
    src_px: usize,
    stride: usize,
) -> Result<(), Error> {
    copy_block(dst_buf, src_buf, dst_px * BPP16, src_px * BPP16, stride, BPP16)
}

fn decode16_0x2(
    r: &mut Reader16<'_>,
    buf: &mut [u8],
    dst_px: usize,
    width: usize,
) -> Result<(), Error> {
    let b = r.read_byte()? as i32;
    let (x, y) = if b < 56 {
        (8 + b % 7, b / 7)
    } else {
        (-14 + (b - 56) % 29, 8 + (b - 56) / 29)
    };
    let offset = (y * width as i32 + x) as isize;
    let src = (dst_px as isize + offset) as usize;
    copy_block16_within(buf, dst_px, src, width)
}

fn decode16_0x3(
    r: &mut Reader16<'_>,
    buf: &mut [u8],
    dst_px: usize,
    width: usize,
) -> Result<(), Error> {
    let b = r.read_byte()? as i32;
    let (x, y) = if b < 56 {
        (-(8 + b % 7), -(b / 7))
    } else {
        (-(-14 + (b - 56) % 29), -(8 + (b - 56) / 29))
    };
    let offset = (y * width as i32 + x) as isize;
    let src = (dst_px as isize + offset) as usize;
    copy_block16_within(buf, dst_px, src, width)
}

fn decode16_0x4(
    r: &mut Reader16<'_>,
    buf1: &mut [u8],
    buf2: &[u8],
    dst_px: usize,
    width: usize,
) -> Result<(), Error> {
    let b = r.read_byte()? as i32;
    let x = -8 + (b & 0x0f);
    let y = -8 + (b >> 4);
    let offset = (y * width as i32 + x) as isize;
    let src = (dst_px as isize + offset) as usize;
    copy_block16(buf1, buf2, dst_px, src, width)
}

fn decode16_0x5(
    r: &mut Reader16<'_>,
    buf1: &mut [u8],
    buf2: &[u8],
    dst_px: usize,
    width: usize,
) -> Result<(), Error> {
    let x = r.read_i8()? as i32;
    let y = r.read_i8()? as i32;
    let offset = (y * width as i32 + x) as isize;
    let src = (dst_px as isize + offset) as usize;
    copy_block16(buf1, buf2, dst_px, src, width)
}

fn decode16_0x7(
    r: &mut Reader16<'_>,
    buf: &mut [u8],
    dst_px: usize,
    width: usize,
) -> Result<(), Error> {
    let p0 = r.read_u16()?;
    let p1 = r.read_u16()?;

    if p0 & 0x8000 == 0 {
        // Per-row mask (8 bytes)
        for y in 0..8usize {
            let flags = r.read_byte()?;
            for x in 0..8usize {
                let px = if flags & (1 << x) != 0 { p1 } else { p0 };
                let off = (dst_px + y * width + x) * BPP16;
                buf[off..off + 2].copy_from_slice(&px.to_le_bytes());
            }
        }
    } else {
        let p0 = p0 & !0x8000;
        // Per-2×2-block mask (2 bytes)
        let flags = r.read_byte()? as u16 | ((r.read_byte()? as u16) << 8);
        let mut mask = 1u16;
        let mut y = 0;
        while y < 8 {
            let mut x = 0;
            while x < 8 {
                let px = if flags & mask != 0 { p1 } else { p0 };
                let pb = px.to_le_bytes();
                for (dy, dx) in [(0,0),(0,1),(1,0),(1,1)] {
                    let off = (dst_px + (y + dy) * width + x + dx) * BPP16;
                    buf[off..off + 2].copy_from_slice(&pb);
                }
                mask <<= 1;
                x += 2;
            }
            y += 2;
        }
    }
    Ok(())
}

fn decode16_0x8(
    r: &mut Reader16<'_>,
    buf: &mut [u8],
    dst_px: usize,
    width: usize,
) -> Result<(), Error> {
    let p0 = r.read_u16()?;
    let p1 = r.read_u16()?;
    let b0 = r.read_byte()?;
    let b1 = r.read_byte()?;

    if p0 & 0x8000 == 0 {
        // 4 quadrants
        let p2 = r.read_u16()?; let p3 = r.read_u16()?;
        let b2 = r.read_byte()?; let b3 = r.read_byte()?;
        let p4 = r.read_u16()?; let p5 = r.read_u16()?;
        let b4 = r.read_byte()?; let b5 = r.read_byte()?;
        let p6 = r.read_u16()?; let p7 = r.read_u16()?;
        let b6 = r.read_byte()?; let b7 = r.read_byte()?;

        let p = [p0,p1,p2,p3,p4,p5,p6,p7];
        let b = [b0,b1,b2,b3,b4,b5,b6,b7];

        for y in 0..8usize {
            let lower = if y >= 4 { 2usize } else { 0 };
            let flags = pack_flags_8(&b, lower, lower+1, lower+4, lower+5);
            let mut pp0 = p[lower];
            let mut pp1 = p[lower + 1];
            for x in 0..8usize {
                if x == 4 { pp0 = p[lower + 4]; pp1 = p[lower + 5]; }
                let px = if flags & (1 << (y % 4 * 8 + x)) != 0 { pp1 } else { pp0 };
                let off = (dst_px + y * width + x) * BPP16;
                buf[off..off+2].copy_from_slice(&px.to_le_bytes());
            }
        }
    } else {
        let p0 = p0 & !0x8000;
        let b2 = r.read_byte()?; let b3 = r.read_byte()?;
        let p2 = r.read_u16()?;  let p3 = r.read_u16()?;
        let b4 = r.read_byte()?; let b5 = r.read_byte()?;
        let b6 = r.read_byte()?; let b7 = r.read_byte()?;

        let p = [p0, p1, p2, p3];
        let b = [b0, b1, b2, b3, b4, b5, b6, b7];

        if p2 & 0x8000 == 0 {
            // Vertical split
            for y in 0..8usize {
                let flags = pack_flags_8(&b, y/4*2, y/4*2+1, y/4*2+4, y/4*2+5);
                let mut pp0 = p[0]; let mut pp1 = p[1];
                for x in 0..8usize {
                    if x == 4 { pp0 = p[2]; pp1 = p[3]; }
                    let px = if flags & (1 << (y%4*8+x)) != 0 { pp1 } else { pp0 };
                    let off = (dst_px + y * width + x) * BPP16;
                    buf[off..off+2].copy_from_slice(&px.to_le_bytes());
                }
            }
        } else {
            let p2 = p2 & !0x8000;
            // Horizontal split
            let mut pp0 = p[0]; let mut pp1 = p[1];
            for y in 0..8usize {
                let flags = b[y];
                if y == 4 { pp0 = p2; pp1 = p[3]; }
                for x in 0..8usize {
                    let px = if flags & (1<<x) != 0 { pp1 } else { pp0 };
                    let off = (dst_px + y * width + x) * BPP16;
                    buf[off..off+2].copy_from_slice(&px.to_le_bytes());
                }
            }
        }
    }
    Ok(())
}

fn decode16_0x9(
    r: &mut Reader16<'_>,
    buf: &mut [u8],
    dst_px: usize,
    width: usize,
) -> Result<(), Error> {
    let p: [u16; 4] = [r.read_u16()?, r.read_u16()?, r.read_u16()?, r.read_u16()?];

    let write_px = |buf: &mut [u8], px_idx: usize, v: u16| {
        let off = px_idx * BPP16;
        buf[off..off+2].copy_from_slice(&v.to_le_bytes());
    };

    if p[0] & 0x8000 == 0 && p[2] & 0x8000 == 0 {
        // per-pixel 4-colour
        for y in 0..8usize {
            let flags = r.read_byte()? as u16 | ((r.read_byte()? as u16) << 8);
            for x in 0..8usize {
                write_px(buf, dst_px + y * width + x, p[((flags >> (x*2)) & 0x03) as usize]);
            }
        }
    } else if p[0] & 0x8000 == 0 {
        let p2 = p[2] & !0x8000; let p = [p[0], p[1], p2, p[3]];
        // per-2×2-block
        let flags = r.read_byte()? as u32 | ((r.read_byte()? as u32) << 8)
            | ((r.read_byte()? as u32) << 16) | ((r.read_byte()? as u32) << 24);
        let mut shifter = 0;
        let mut y = 0;
        while y < 8 {
            let mut x = 0;
            while x < 8 {
                let v = p[((flags >> shifter) & 0x03) as usize];
                for (dy,dx) in [(0,0),(0,1),(1,0),(1,1)] {
                    write_px(buf, dst_px+(y+dy)*width+x+dx, v);
                }
                shifter += 2; x += 2;
            }
            y += 2;
        }
    } else if p[2] & 0x8000 == 0 {
        let p0 = p[0] & !0x8000; let p = [p0, p[1], p[2], p[3]];
        // per-2×1-block (wide)
        let mut y = 0;
        while y < 8 {
            let flags = r.read_byte()? as u32 | ((r.read_byte()? as u32)<<8)
                | ((r.read_byte()? as u32)<<16) | ((r.read_byte()? as u32)<<24);
            let mut shifter = 0;
            for dy in 0..4usize {
                let mut x = 0;
                while x < 8 {
                    let v = p[((flags >> shifter) & 0x03) as usize];
                    write_px(buf, dst_px+(y+dy)*width+x,   v);
                    write_px(buf, dst_px+(y+dy)*width+x+1, v);
                    shifter += 2; x += 2;
                }
            }
            y += 4;
        }
    } else {
        let p0 = p[0] & !0x8000; let p2 = p[2] & !0x8000; let p = [p0, p[1], p2, p[3]];
        // per-1×2-block (tall)
        let mut y = 0;
        while y < 8 {
            let flags = r.read_byte()? as u32 | ((r.read_byte()? as u32)<<8)
                | ((r.read_byte()? as u32)<<16) | ((r.read_byte()? as u32)<<24);
            let mut shifter = 0;
            let mut dy = 0;
            while dy < 4 {
                for x in 0..8usize {
                    let v = p[((flags >> shifter) & 0x03) as usize];
                    write_px(buf, dst_px+(y+dy)*width+x, v);
                    write_px(buf, dst_px+(y+dy+1)*width+x, v);
                    shifter += 2;
                }
                dy += 2;
            }
            y += 4;
        }
    }
    Ok(())
}

fn decode16_0xa(
    r: &mut Reader16<'_>,
    buf: &mut [u8],
    dst_px: usize,
    width: usize,
) -> Result<(), Error> {
    let p0 = r.read_u16()?; let p1 = r.read_u16()?;
    let p2 = r.read_u16()?; let p3 = r.read_u16()?;

    let write_px = |buf: &mut [u8], px_idx: usize, v: u16| {
        let off = px_idx * BPP16;
        buf[off..off+2].copy_from_slice(&v.to_le_bytes());
    };

    if p0 & 0x8000 == 0 {
        // 4 quadrants
        let mut b = [0u8; 16];
        for i in 0..4 { b[i] = r.read_byte()?; }
        let mut p = [p0,p1,p2,p3, 0u16,0,0,0, 0,0,0,0, 0,0,0,0];
        for chunk in 1..4usize {
            for i in 0..4 { p[chunk*4+i] = r.read_u16()?; }
            for i in 0..4 { b[chunk*4+i] = r.read_byte()?; }
        }
        for y in 0..8usize {
            let lower = if y >= 4 { 4usize } else { 0 };
            let flags = (b[y+8] as u16) << 8 | b[y] as u16;
            for x in 0..8usize {
                let split = if x >= 4 { 8usize } else { 0 };
                let idx = split + lower + ((flags >> (x*2)) & 0x03) as usize;
                write_px(buf, dst_px + y*width + x, p[idx]);
            }
        }
    } else {
        let p0 = p0 & !0x8000;
        let mut b = [0u8; 16];
        for i in 0..8 { b[i] = r.read_byte()?; }
        let p4 = r.read_u16()?; let p5 = r.read_u16()?;
        let p6 = r.read_u16()?; let p7 = r.read_u16()?;
        for i in 8..16 { b[i] = r.read_byte()?; }
        let p = [p0,p1,p2,p3, p4,p5,p6,p7, 0u16,0,0,0,0,0,0,0];

        if p4 & 0x8000 == 0 {
            // Left/right halves
            for y in 0..8usize {
                let flags = (b[y+8] as u16) << 8 | b[y] as u16;
                for x in 0..8usize {
                    let split = if x >= 4 { 4usize } else { 0 };
                    write_px(buf, dst_px+y*width+x, p[split+((flags>>(x*2))&0x03) as usize]);
                }
            }
        } else {
            let p4 = p4 & !0x8000; let p = [p0,p1,p2,p3, p4,p5,p6,p7, 0u16,0,0,0,0,0,0,0];
            // Top/bottom halves
            for y in 0..8usize {
                let flags = (b[y*2+1] as u16) << 8 | b[y*2] as u16;
                let split = if y >= 4 { 4usize } else { 0 };
                for x in 0..8usize {
                    write_px(buf, dst_px+y*width+x, p[split+((flags>>(x*2))&0x03) as usize]);
                }
            }
        }
    }
    Ok(())
}

fn decode16_0xb(
    r: &mut Reader16<'_>,
    buf: &mut [u8],
    dst_px: usize,
    width: usize,
) -> Result<(), Error> {
    for y in 0..8usize {
        for x in 0..8usize {
            let v = r.read_u16()?;
            let off = (dst_px + y * width + x) * BPP16;
            buf[off..off+2].copy_from_slice(&v.to_le_bytes());
        }
    }
    Ok(())
}

fn decode16_0xc(
    r: &mut Reader16<'_>,
    buf: &mut [u8],
    dst_px: usize,
    width: usize,
) -> Result<(), Error> {
    let mut y = 0;
    while y < 8 {
        let mut x = 0;
        while x < 8 {
            let v = r.read_u16()?;
            let vb = v.to_le_bytes();
            for (dy, dx) in [(0,0),(0,1),(1,0),(1,1)] {
                let off = (dst_px + (y+dy)*width + x+dx) * BPP16;
                buf[off..off+2].copy_from_slice(&vb);
            }
            x += 2;
        }
        y += 2;
    }
    Ok(())
}

fn decode16_0xd(
    r: &mut Reader16<'_>,
    buf: &mut [u8],
    dst_px: usize,
    width: usize,
) -> Result<(), Error> {
    let p = [r.read_u16()?, r.read_u16()?, r.read_u16()?, r.read_u16()?];
    for y in 0..8usize {
        let base = if y < 4 { 0usize } else { 2 };
        for x in 0..8usize {
            let idx = base + if x >= 4 { 1 } else { 0 };
            let off = (dst_px + y * width + x) * BPP16;
            buf[off..off+2].copy_from_slice(&p[idx].to_le_bytes());
        }
    }
    Ok(())
}

fn decode16_0xe(
    r: &mut Reader16<'_>,
    buf: &mut [u8],
    dst_px: usize,
    width: usize,
) -> Result<(), Error> {
    let v = r.read_u16()?;
    let vb = v.to_le_bytes();
    for y in 0..8usize {
        for x in 0..8usize {
            let off = (dst_px + y * width + x) * BPP16;
            buf[off..off+2].copy_from_slice(&vb);
        }
    }
    Ok(())
}

fn decode16_0xf(
    r: &mut Reader16<'_>,
    buf: &mut [u8],
    dst_px: usize,
    width: usize,
) -> Result<(), Error> {
    let p = [r.read_u16()?, r.read_u16()?];
    for y in 0..8usize {
        for x in 0..8usize {
            let v = p[(y ^ x) & 1];
            let off = (dst_px + y * width + x) * BPP16;
            buf[off..off+2].copy_from_slice(&v.to_le_bytes());
        }
    }
    Ok(())
}

/// Decode one 16-bit (RGB555) video frame into `buf1`.
/// `buf2` holds the previous frame.
pub fn decode_frame16(
    buf1: &mut Vec<u8>,
    buf2: &mut Vec<u8>,
    code_map: &[u8],
    data: &[u8],
    width: u16,
    height: u16,
) -> Result<(), Error> {
    let w = width as usize;
    let h = height as usize;

    if data.len() < 2 {
        return Err(Error::VideoDecode("16-bit frame data too short".into()));
    }

    // First 2 bytes: offset to the motion-vector sub-stream (data2)
    let offset = u16::from_le_bytes([data[0], data[1]]) as usize;
    if offset > data.len() {
        return Err(Error::VideoDecode("16-bit frame: invalid data2 offset".into()));
    }

    // data  — colour/pixel data starting at byte 2
    // data2 — motion-vector data starting at byte `offset`
    let mut rc = Reader16::new(data, 2);       // opcodes 0x5, 0x7-0xf
    let mut rd = Reader16::new(data, offset);  // opcodes 0x2, 0x3, 0x4

    let mut code_idx = 0usize;
    let bx_count = w >> 3;
    let by_count = h >> 3;

    for by in 0..by_count {
        for bx in 0..bx_count {
            let opcode = if code_idx & 1 == 0 {
                code_map[code_idx >> 1] & 0x0f
            } else {
                code_map[code_idx >> 1] >> 4
            };
            code_idx += 1;

            let dst_px = by * 8 * w + bx * 8;

            match opcode {
                0x0 => copy_block16(buf1, buf2, dst_px, dst_px, w)?,
                0x1 => {}
                0x2 => decode16_0x2(&mut rd, buf1, dst_px, w)?,
                0x3 => decode16_0x3(&mut rd, buf1, dst_px, w)?,
                0x4 => decode16_0x4(&mut rd, buf1, buf2, dst_px, w)?,
                0x5 => decode16_0x5(&mut rc, buf1, buf2, dst_px, w)?,
                0x6 => return Err(Error::VideoDecode("unsupported opcode 0x6".into())),
                0x7 => decode16_0x7(&mut rc, buf1, dst_px, w)?,
                0x8 => decode16_0x8(&mut rc, buf1, dst_px, w)?,
                0x9 => decode16_0x9(&mut rc, buf1, dst_px, w)?,
                0xa => decode16_0xa(&mut rc, buf1, dst_px, w)?,
                0xb => decode16_0xb(&mut rc, buf1, dst_px, w)?,
                0xc => decode16_0xc(&mut rc, buf1, dst_px, w)?,
                0xd => decode16_0xd(&mut rc, buf1, dst_px, w)?,
                0xe => decode16_0xe(&mut rc, buf1, dst_px, w)?,
                0xf => decode16_0xf(&mut rc, buf1, dst_px, w)?,
                _ => unreachable!(),
            }
        }
    }
    Ok(())
}
