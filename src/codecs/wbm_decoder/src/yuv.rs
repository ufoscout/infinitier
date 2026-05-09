//! YUV 4:2:0 → RGBA8 conversion.
//!
//! Mirrors `bik_decoder::streaming::yuv420p_to_rgba8` semantics
//! (BT.601, nearest-neighbour chroma upsample) so that movies coming
//! from either container reach the GPU through the same color
//! pipeline.

/// Convert tightly-strided YUV420p planes (Y at full res, U/V at half
/// res in both axes) to a row-major RGBA8 buffer of `width * height *
/// 4` bytes.
pub(crate) fn yuv420p_to_rgba8(
    y: &[u8],
    u: &[u8],
    v: &[u8],
    y_stride: usize,
    uv_stride: usize,
    width: u32,
    height: u32,
) -> Vec<u8> {
    let w = width as usize;
    let h = height as usize;
    let mut out = vec![0u8; w * h * 4];
    for row in 0..h {
        let y_row = &y[row * y_stride..row * y_stride + w];
        let chroma_row = row / 2;
        let chroma_w = w.div_ceil(2);
        let u_row = &u[chroma_row * uv_stride..chroma_row * uv_stride + chroma_w];
        let v_row = &v[chroma_row * uv_stride..chroma_row * uv_stride + chroma_w];
        for col in 0..w {
            let y = y_row[col] as f32;
            let u = u_row[col / 2] as f32 - 128.0;
            let v = v_row[col / 2] as f32 - 128.0;
            let r = (y + 1.402 * v).clamp(0.0, 255.0) as u8;
            let g = (y - 0.344_136 * u - 0.714_136 * v).clamp(0.0, 255.0) as u8;
            let b = (y + 1.772 * u).clamp(0.0, 255.0) as u8;
            let off = (row * w + col) * 4;
            out[off] = r;
            out[off + 1] = g;
            out[off + 2] = b;
            out[off + 3] = 255;
        }
    }
    out
}
