//! Median-cut palette quantisation for true-colour input.
//!
//! Takes a slice of RGB888 frames (any number of unique colours) and
//! produces:
//!  - a 256-entry palette (palette[0..N] are the chosen
//!    representatives; remaining slots stay black)
//!  - one `Vec<u8>` per frame — palette indices, row-major.
//!
//! Algorithm (classic median-cut, à la Heckbert 1982):
//!  1. Build a histogram of unique RGB triples across all frames.
//!  2. Place every unique colour into a single "box" (axis-aligned
//!     bounding box in 3-space, holding a list of pixels with their
//!     occurrence counts).
//!  3. Repeatedly split the box with the largest weighted volume:
//!     pick its longest channel, sort its colours by that channel,
//!     split at the median (by pixel count, not by colour count).
//!  4. Stop when we have `target_colours` boxes (≤ 256). Each box's
//!     representative is the count-weighted average of its colours.
//!  5. Build a nearest-neighbour lookup via squared-Euclidean
//!     distance in RGB space and remap every pixel.
//!
//! When the input has ≤ `target_colours` unique colours the
//! quantiser short-circuits to a perfect-mapping path that is
//! guaranteed bit-exact through the round-trip.

use std::collections::HashMap;

/// One bucket in the median-cut tree.
#[derive(Clone)]
struct Box3 {
    /// `(rgb, count)` for every colour in this box.
    colours: Vec<([u8; 3], u32)>,
}

impl Box3 {
    fn min_max(&self) -> ([u8; 3], [u8; 3]) {
        let mut lo = [u8::MAX; 3];
        let mut hi = [0u8; 3];
        for (rgb, _) in &self.colours {
            for c in 0..3 {
                if rgb[c] < lo[c] {
                    lo[c] = rgb[c];
                }
                if rgb[c] > hi[c] {
                    hi[c] = rgb[c];
                }
            }
        }
        (lo, hi)
    }

    /// Total pixel count in the box.
    fn weight(&self) -> u64 {
        self.colours.iter().map(|(_, c)| *c as u64).sum()
    }

    /// Channel (0=R, 1=G, 2=B) with the largest range.
    fn longest_channel(&self) -> usize {
        let (lo, hi) = self.min_max();
        let r = hi[0] - lo[0];
        let g = hi[1] - lo[1];
        let b = hi[2] - lo[2];
        if r >= g && r >= b {
            0
        } else if g >= b {
            1
        } else {
            2
        }
    }

    /// Score used to pick which box to split next: weighted by how
    /// much information would be lost if we kept this box collapsed
    /// to a single representative. Use `weight × longest_axis_range`
    /// — boxes with many pixels and wide colour spread split first.
    fn split_priority(&self) -> u64 {
        let (lo, hi) = self.min_max();
        let range = (hi[0] - lo[0]).max(hi[1] - lo[1]).max(hi[2] - lo[2]) as u64;
        self.weight() * (range + 1)
    }

    /// Split this box at the median pixel along its longest channel.
    /// Returns `None` if the box can't be split (single colour).
    fn split(self) -> Option<(Box3, Box3)> {
        if self.colours.len() < 2 {
            return None;
        }
        let ch = self.longest_channel();
        let mut sorted = self.colours;
        sorted.sort_unstable_by_key(|(rgb, _)| rgb[ch]);

        let total: u64 = sorted.iter().map(|(_, c)| *c as u64).sum();
        let half = total / 2;
        let mut accum = 0u64;
        let mut cut = 0;
        for (i, (_, c)) in sorted.iter().enumerate() {
            accum += *c as u64;
            if accum >= half {
                cut = i + 1;
                break;
            }
        }
        // Guard against degenerate splits (everything in one half).
        if cut == 0 {
            cut = 1;
        }
        if cut >= sorted.len() {
            cut = sorted.len() - 1;
        }
        let right = sorted.split_off(cut);
        Some((Box3 { colours: sorted }, Box3 { colours: right }))
    }

    /// Count-weighted average colour — used as the box's
    /// representative entry in the final palette.
    fn representative(&self) -> [u8; 3] {
        let mut sum = [0u64; 3];
        let mut total = 0u64;
        for (rgb, c) in &self.colours {
            for k in 0..3 {
                sum[k] += rgb[k] as u64 * *c as u64;
            }
            total += *c as u64;
        }
        if total == 0 {
            return [0, 0, 0];
        }
        [
            (sum[0] / total) as u8,
            (sum[1] / total) as u8,
            (sum[2] / total) as u8,
        ]
    }
}

/// Compute up to `target_colours` (≤ 256) representative palette
/// entries via median cut on the colour histogram of `frames`. Any
/// remaining palette slots are filled with `[0, 0, 0]`.
fn median_cut(frames: &[&[[u8; 3]]], target_colours: usize) -> Box<[[u8; 3]; 256]> {
    debug_assert!(target_colours > 0 && target_colours <= 256);

    let mut hist: HashMap<[u8; 3], u32> = HashMap::new();
    for frame in frames {
        for px in *frame {
            *hist.entry(*px).or_default() += 1;
        }
    }

    let mut boxes: Vec<Box3> = if hist.is_empty() {
        Vec::new()
    } else {
        vec![Box3 {
            colours: hist.into_iter().collect(),
        }]
    };

    while boxes.len() < target_colours {
        // Pick the highest-priority box that's still splittable.
        let mut best_idx: Option<usize> = None;
        let mut best_score = 0u64;
        for (i, b) in boxes.iter().enumerate() {
            if b.colours.len() < 2 {
                continue;
            }
            let s = b.split_priority();
            if best_idx.is_none() || s > best_score {
                best_score = s;
                best_idx = Some(i);
            }
        }
        let Some(idx) = best_idx else {
            break; // every remaining box is a single colour
        };
        let b = boxes.swap_remove(idx);
        // We already filtered for `colours.len() >= 2`, so `split`
        // is guaranteed to succeed.
        let (l, r) = b.split().expect("multi-colour box must split");
        boxes.push(l);
        boxes.push(r);
    }

    let mut palette = Box::new([[0u8; 3]; 256]);
    for (i, b) in boxes.iter().enumerate() {
        palette[i] = b.representative();
    }
    palette
}

/// Map every pixel in every frame to the palette index minimising
/// the squared-Euclidean distance in RGB space. Returns one
/// `Vec<u8>` per frame.
fn map_frames(
    frames: &[&[[u8; 3]]],
    palette: &[[u8; 3]; 256],
    palette_size: usize,
) -> Vec<Vec<u8>> {
    let mut cache: HashMap<[u8; 3], u8> = HashMap::with_capacity(palette_size * 4);
    let mut out = Vec::with_capacity(frames.len());
    for frame in frames {
        let mut buf = Vec::with_capacity(frame.len());
        for px in *frame {
            let idx = if let Some(&i) = cache.get(px) {
                i
            } else {
                let i = nearest_palette_index(*px, palette, palette_size);
                cache.insert(*px, i);
                i
            };
            buf.push(idx);
        }
        out.push(buf);
    }
    out
}

#[inline]
fn nearest_palette_index(px: [u8; 3], palette: &[[u8; 3]; 256], palette_size: usize) -> u8 {
    let mut best = 0u8;
    let mut best_dist = u32::MAX;
    let n = palette_size.min(256);
    for (i, p) in palette.iter().enumerate().take(n) {
        let dr = px[0] as i32 - p[0] as i32;
        let dg = px[1] as i32 - p[1] as i32;
        let db = px[2] as i32 - p[2] as i32;
        let d = (dr * dr + dg * dg + db * db) as u32;
        if d < best_dist {
            best_dist = d;
            best = i as u8;
            if d == 0 {
                return best; // exact match — can't beat it
            }
        }
    }
    best
}

/// Quantise RGB888 frames to a 256-entry palette + per-frame index
/// buffers. If the input has ≤ 256 unique colours, the function
/// short-circuits to a bit-exact mapping (no median cut, no
/// nearest-neighbour fall-back).
pub fn quantise_to_palette8(frames: &[&[[u8; 3]]]) -> (Box<[[u8; 3]; 256]>, Vec<Vec<u8>>) {
    // Fast path: collect unique colours; if ≤ 256 use them verbatim.
    let mut unique: HashMap<[u8; 3], u8> = HashMap::new();
    let mut palette = Box::new([[0u8; 3]; 256]);
    let mut over = false;
    for frame in frames {
        for &px in *frame {
            if !unique.contains_key(&px) {
                let next = unique.len();
                if next >= 256 {
                    over = true;
                    break;
                }
                palette[next] = px;
                unique.insert(px, next as u8);
            }
        }
        if over {
            break;
        }
    }
    if !over {
        // ≤ 256 unique colours — do a direct map for bit-exact output.
        let mut indexed = Vec::with_capacity(frames.len());
        for frame in frames {
            let mut buf = Vec::with_capacity(frame.len());
            for px in *frame {
                buf.push(unique[px]);
            }
            indexed.push(buf);
        }
        return (palette, indexed);
    }

    // > 256 unique colours: median-cut to 256 representatives, then
    // remap by nearest-neighbour.
    let palette = median_cut(frames, 256);
    let indexed = map_frames(frames, &palette, 256);
    (palette, indexed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fast_path_is_bit_exact_under_256_colours() {
        // 4 distinct colours — well under 256.
        let cols: [[u8; 3]; 4] = [[255, 0, 0], [0, 255, 0], [0, 0, 255], [128, 128, 128]];
        let frame: Vec<[u8; 3]> = (0..16).map(|i| cols[i as usize % 4]).collect();
        let frames = [frame.as_slice()];
        let (palette, indexed) = quantise_to_palette8(&frames);
        assert_eq!(indexed.len(), 1);
        for (i, &px) in frame.iter().enumerate() {
            let mapped = palette[indexed[0][i] as usize];
            assert_eq!(mapped, px, "fast path must be bit-exact");
        }
    }

    #[test]
    fn median_cut_handles_more_than_256_colours() {
        // 12 × 12 × 12 = 1728 distinct RGB triples — must collapse.
        let mut frame: Vec<[u8; 3]> = Vec::with_capacity(1728);
        for r in 0..12 {
            for g in 0..12 {
                for b in 0..12 {
                    frame.push([r * 20, g * 20, b * 20]);
                }
            }
        }
        let frames = [frame.as_slice()];
        let (palette, indexed) = quantise_to_palette8(&frames);
        assert_eq!(indexed.len(), 1);
        // Average per-pixel distance must be small (under ~30 LSB
        // is what median-cut on a uniform cube achieves with 256
        // entries).
        let mut total: u64 = 0;
        for (i, &px) in frame.iter().enumerate() {
            let p = palette[indexed[0][i] as usize];
            let dr = px[0] as i32 - p[0] as i32;
            let dg = px[1] as i32 - p[1] as i32;
            let db = px[2] as i32 - p[2] as i32;
            total += (dr * dr + dg * dg + db * db) as u64;
        }
        let mean_sq = total / frame.len() as u64;
        let mean = (mean_sq as f64).sqrt();
        assert!(
            mean < 25.0,
            "mean per-pixel distance {mean} too high; want < 25"
        );
    }

    #[test]
    fn empty_frames_returns_zero_palette() {
        let frames: Vec<&[[u8; 3]]> = Vec::new();
        let (palette, indexed) = quantise_to_palette8(&frames);
        assert!(indexed.is_empty());
        // Palette is all-zero; no UB.
        for slot in palette.iter() {
            assert_eq!(*slot, [0, 0, 0]);
        }
    }
}
