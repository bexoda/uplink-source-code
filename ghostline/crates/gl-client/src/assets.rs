//! Placing servers on the world map: land only, and stable per name.

/// Which pixels of the world map are land. Servers are placed on land only.
pub struct LandMask {
    pub width: usize,
    pub height: usize,
    land: Vec<bool>,
}

impl LandMask {
    pub fn from_rgba(width: usize, height: usize, rgba: &[u8]) -> LandMask {
        let land = rgba
            .chunks_exact(4)
            .take(width * height)
            .map(|p| p[0] > 127)
            .collect();
        LandMask {
            width,
            height,
            land,
        }
    }

    pub fn is_land(&self, x: usize, y: usize) -> bool {
        x < self.width
            && y < self.height
            && self.land.get(y * self.width + x).copied().unwrap_or(false)
    }
}

/// Stable hash so a server always lands in the same spot, across runs and platforms.
fn fnv(s: &str) -> u64 {
    s.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// Places each name on land, spaced apart. Returns positions normalised to 0..1.
/// Deterministic: the same names always give the same layout. Names are placed in order,
/// so an earlier name never moves when a later one is added.
pub fn place_on_land(names: &[String], mask: &LandMask, min_gap: f32) -> Vec<(f32, f32)> {
    if mask.width == 0 || mask.height == 0 {
        return vec![(0.5, 0.5); names.len()];
    }
    let mut placed: Vec<(f32, f32)> = Vec::with_capacity(names.len());
    for name in names {
        let mut state = fnv(name);
        let mut next = || {
            // xorshift64*
            state ^= state >> 12;
            state ^= state << 25;
            state ^= state >> 27;
            state.wrapping_mul(0x2545_F491_4F6C_DD1D)
        };
        let mut best: Option<(f32, f32)> = None;
        // Prefer a spot that is land and far from the others; relax the gap if the map is crowded.
        for gap in [min_gap, min_gap * 0.6, 0.0] {
            for _ in 0..600 {
                let (x, y) = (
                    (next() >> 33) as usize % mask.width,
                    (next() >> 33) as usize % mask.height,
                );
                let p = (x as f32 / mask.width as f32, y as f32 / mask.height as f32);
                if mask.is_land(x, y)
                    && placed
                        .iter()
                        .all(|q| ((p.0 - q.0).powi(2) + (p.1 - q.1).powi(2)).sqrt() >= gap)
                {
                    best = Some(p);
                    break;
                }
            }
            if best.is_some() {
                break;
            }
        }
        // A mask with no land at all would be a broken asset; fall back to the centre rather than panic.
        let p = best.unwrap_or((0.5, 0.5));
        placed.push(p);
    }
    placed
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Left half sea, right half land.
    fn half_mask() -> LandMask {
        let (w, h) = (64, 64);
        let rgba: Vec<u8> = (0..w * h)
            .flat_map(|i| {
                if i % w >= w / 2 {
                    [255, 255, 255, 255]
                } else {
                    [0, 0, 0, 255]
                }
            })
            .collect();
        LandMask::from_rgba(w, h, &rgba)
    }

    fn names(n: usize) -> Vec<String> {
        (0..n).map(|i| format!("corp-{i}")).collect()
    }

    #[test]
    fn nodes_land_on_land_and_are_spread_out() {
        let m = half_mask();
        let pts = place_on_land(&names(10), &m, 0.12);
        assert_eq!(pts.len(), 10);
        for &(x, y) in &pts {
            assert!(
                m.is_land((x * 64.0) as usize, (y * 64.0) as usize),
                "({x},{y}) is in the sea"
            );
        }
        let mut min = f32::MAX;
        for i in 0..pts.len() {
            for j in 0..i {
                min =
                    min.min(((pts[i].0 - pts[j].0).powi(2) + (pts[i].1 - pts[j].1).powi(2)).sqrt());
            }
        }
        assert!(min > 0.07, "nodes overlap: closest pair {min}");
    }

    #[test]
    fn layout_is_deterministic_and_stable_when_names_are_added() {
        let m = half_mask();
        let a = place_on_land(&names(6), &m, 0.1);
        assert_eq!(a, place_on_land(&names(6), &m, 0.1));
        let b = place_on_land(&names(9), &m, 0.1);
        assert_eq!(&a[..], &b[..6], "existing servers must not jump around");
    }

    #[test]
    fn broken_masks_degrade_instead_of_panicking() {
        let all_sea = LandMask::from_rgba(8, 8, &[0u8; 8 * 8 * 4]);
        assert_eq!(place_on_land(&names(2), &all_sea, 0.2), vec![(0.5, 0.5); 2]);
        let empty = LandMask::from_rgba(0, 0, &[]);
        assert_eq!(place_on_land(&names(1), &empty, 0.2).len(), 1);
    }
}
