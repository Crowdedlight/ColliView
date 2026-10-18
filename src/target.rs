//! The printable centring target, `calibration/centring_target.pdf`.
//!
//! Black and white 5 mm squares that never repeat. Each square is drawn at random, and
//! the squares are grouped in 4 × 4 blocks that are either mostly black or mostly white.
//! The blocks give the pattern structure at a coarse scale, so the match between the
//! 0° and 180° views is unique at every pyramid level.
//!
//! Std-only: `examples/make_target.rs` includes this file to write the PDF, and the
//! simulator renders the same pattern.

/// A4 portrait.
pub const PAGE_MM: (f32, f32) = (210.0, 297.0);
pub const CELL_MM: f32 = 5.0;
/// Top-left corner of the pattern on the page.
pub const ORIGIN_MM: (f32, f32) = (10.0, 10.0);
pub const COLS: i32 = 38;
pub const ROWS: i32 = 55;
/// Squares per block side.
const BLOCK: i32 = 4;

/// An axis-aligned rectangle in mm, from the page's top-left corner, y down.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// Whether square (`col`, `row`) is black. Defined for any index, so the simulator can
/// render past the page edge.
pub fn is_black(col: i32, row: i32) -> bool {
    let dense = unit(hash(col.div_euclid(BLOCK), row.div_euclid(BLOCK), 0xb10c)) < 0.5;
    unit(hash(col, row, 0xce11)) < if dense { 0.8 } else { 0.2 }
}

/// Reflectance at a point on the page (mm): 0 black, 1 white.
pub fn value(x_mm: f32, y_mm: f32) -> f32 {
    let col = ((x_mm - ORIGIN_MM.0) / CELL_MM).floor() as i32;
    let row = ((y_mm - ORIGIN_MM.1) / CELL_MM).floor() as i32;
    if is_black(col, row) { 0.0 } else { 1.0 }
}

/// The black squares on the page, merged into one rectangle per horizontal run.
pub fn black_runs() -> Vec<Rect> {
    let mut out = Vec::new();
    for row in 0..ROWS {
        let mut col = 0;
        while col < COLS {
            if !is_black(col, row) {
                col += 1;
                continue;
            }
            let start = col;
            while col < COLS && is_black(col, row) {
                col += 1;
            }
            out.push(Rect {
                x: ORIGIN_MM.0 + start as f32 * CELL_MM,
                y: ORIGIN_MM.1 + row as f32 * CELL_MM,
                w: (col - start) as f32 * CELL_MM,
                h: CELL_MM,
            });
        }
    }
    out
}

/// SplitMix64 finaliser over the cell index.
fn hash(a: i32, b: i32, salt: u64) -> u64 {
    let mut x = (a as i64 as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15)
        ^ (b as i64 as u64).wrapping_mul(0xc2b2_ae3d_27d4_eb4f)
        ^ salt.wrapping_mul(0x1656_67b1_9e37_79f9);
    x ^= x >> 30;
    x = x.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}

/// Uniform in [0, 1).
fn unit(h: u64) -> f32 {
    (h >> 40) as f32 / (1u64 << 24) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn about_half_black_and_runs_cover_the_black_squares() {
        let black = (0..ROWS).flat_map(|r| (0..COLS).map(move |c| (c, r))).filter(|&(c, r)| is_black(c, r)).count();
        let total = (ROWS * COLS) as usize;
        assert!(black > total * 2 / 5 && black < total * 3 / 5, "{black} of {total}");
        let area: f32 = black_runs().iter().map(|r| r.w * r.h).sum();
        assert!((area - black as f32 * CELL_MM * CELL_MM).abs() < 1e-3);
        let (x, y) = (ORIGIN_MM.0 + 2.5, ORIGIN_MM.1 + 2.5);
        assert_eq!(value(x, y) == 0.0, is_black(0, 0));
    }
}
