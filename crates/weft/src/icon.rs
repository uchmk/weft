//! The app's icon, drawn at any size so no image file is carried: a dark
//! tile with three warp threads (grey, up and down) and three weft threads
//! (teal, across) woven over and under each other, the weft on top where
//! the row and the column add up to an even number. A thin gap around the
//! thread on top shows which one passes under.

const TILE: [u8; 3] = [0x16, 0x18, 0x1d];
const RIM: [u8; 3] = [0x2e, 0x33, 0x3d];
const WEFT: [u8; 3] = [0x6f, 0xd0, 0xd0];
const WARP: [u8; 3] = [0x7a, 0x80, 0x90];
/// Where the threads run, as fractions of the side.
const LINES: [f32; 3] = [0.32, 0.5, 0.68];
/// Half a thread's width, the gap around the one on top, and where the threads end.
const HALF: f32 = 0.055;
const GAP: f32 = 0.025;
const SPAN: (f32, f32) = (0.18, 0.82);
/// Samples per pixel along each side, for the edges.
const SUB: usize = 4;

/// The icon as RGBA, `size` x `size`.
pub fn pixels(size: u32) -> Vec<u8> {
    let n = size as usize;
    let s = size as f32;
    let mut out = vec![0u8; n * n * 4];
    for y in 0..n {
        for x in 0..n {
            let (mut sum, mut hit) = ([0u32; 3], 0u32);
            for sy in 0..SUB {
                for sx in 0..SUB {
                    let fx = (x as f32 + (sx as f32 + 0.5) / SUB as f32) / s;
                    let fy = (y as f32 + (sy as f32 + 0.5) / SUB as f32) / s;
                    if let Some(c) = at(fx, fy, s) {
                        for (a, b) in sum.iter_mut().zip(c) {
                            *a += u32::from(b);
                        }
                        hit += 1;
                    }
                }
            }
            if hit == 0 {
                continue;
            }
            let alpha = (hit * 255 / (SUB * SUB) as u32) as u8;
            let px = [(sum[0] / hit) as u8, (sum[1] / hit) as u8, (sum[2] / hit) as u8, alpha];
            out[(y * n + x) * 4..][..4].copy_from_slice(&px);
        }
    }
    out
}

/// The colour at (`fx`, `fy`) (fractions of the side), or `None` outside the tile's rounded corners.
fn at(fx: f32, fy: f32, size: f32) -> Option<[u8; 3]> {
    let r = 0.2;
    let (cx, cy) = (fx.clamp(r, 1.0 - r), fy.clamp(r, 1.0 - r));
    let d = ((fx - cx).powi(2) + (fy - cy).powi(2)).sqrt();
    if d > r {
        return None;
    }
    // A rim at least a pixel wide, so the tile stands off a dark bar.
    if d > r - (1.0 / size).max(0.03) {
        return Some(RIM);
    }
    let along = |v: f32| (SPAN.0..=SPAN.1).contains(&v);
    // The thread whose middle is within `w` of `v`, by its index.
    let near = |v: f32, w: f32| LINES.iter().position(|l| (v - l).abs() <= w);
    let weft_on_top = |i: usize, j: usize| (i + j).is_multiple_of(2);
    let warp = if along(fy) { near(fx, HALF) } else { None };
    let weft = if along(fx) { near(fy, HALF) } else { None };
    Some(match (warp, weft) {
        (Some(i), Some(j)) => {
            if weft_on_top(i, j) {
                WEFT
            } else {
                WARP
            }
        }
        (Some(i), None) => match near(fy, HALF + GAP).filter(|_| along(fx)) {
            Some(j) if weft_on_top(i, j) => TILE,
            _ => WARP,
        },
        (None, Some(j)) => match near(fx, HALF + GAP).filter(|_| along(fy)) {
            Some(i) if !weft_on_top(i, j) => TILE,
            _ => WEFT,
        },
        (None, None) => TILE,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_is_square_rgba_with_clear_corners() {
        let p = pixels(32);
        assert_eq!(p.len(), 32 * 32 * 4);
        assert_eq!(p[3], 0);
        assert_eq!(p[(16 * 32 + 16) * 4 + 3], 255);
    }

    /// The middle crossing has the weft on top; the one beside it, the warp.
    #[test]
    fn the_threads_go_over_and_under_in_turn() {
        assert_eq!(at(0.5, 0.5, 128.0), Some(WEFT));
        assert_eq!(at(0.32, 0.5, 128.0), Some(WARP));
        assert_eq!(at(0.5, 0.32, 128.0), Some(WARP));
        assert_eq!(at(0.32, 0.32, 128.0), Some(WEFT));
        // Beside the middle crossing, the warp stops short of the weft on top of it.
        assert_eq!(at(0.5, 0.5 - HALF - GAP / 2.0, 128.0), Some(TILE));
        assert_eq!(at(0.0, 0.0, 128.0), None);
    }
}
