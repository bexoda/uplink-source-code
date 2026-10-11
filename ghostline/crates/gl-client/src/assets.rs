//! Original Uplink artwork, loaded at runtime from the player's own install. Entirely optional:
//! without it the client draws its built-in vector look. Nothing here is copied into the repo or build.

use std::collections::HashMap;
use std::sync::Arc;

use eframe::egui::{
    self, ColorImage, FontData, FontDefinitions, FontFamily, TextureHandle, TextureOptions,
};
use gl_assets::Store;

/// Family used for headings (the game's display face, mostly capitals).
pub fn heading_family() -> FontFamily {
    FontFamily::Name("battle".into())
}

/// HUD icons we use as section markers, keyed by the name we ask for them under.
const ICONS: [(&str, &str); 6] = [
    ("mission", "hud/mission.tif"),
    ("software", "hud/software.tif"),
    ("analyser", "hud/analyser.tif"),
    ("finance", "hud/finance.tif"),
    ("status", "hud/status.tif"),
    ("cpu", "gateway/cpu.tif"),
];

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

pub struct Assets {
    pub map: TextureHandle,
    pub mask: LandMask,
    pub icons: HashMap<&'static str, TextureHandle>,
    pub granted: Option<TextureHandle>,
    pub denied: Option<TextureHandle>,
    pub fonts: bool,
}

fn texture(
    ctx: &egui::Context,
    store: &mut Store,
    name: &str,
    key: &str,
) -> Result<TextureHandle, String> {
    let img = store.image(name).map_err(|e| format!("{name}: {e}"))?;
    let color =
        ColorImage::from_rgba_unmultiplied([img.width as usize, img.height as usize], &img.pixels);
    Ok(ctx.load_texture(key, color, TextureOptions::LINEAR))
}

/// Install the game's own fonts. Returns false (and changes nothing) if they can't be read.
fn install_fonts(ctx: &egui::Context, store: &mut Store) -> bool {
    let (Ok(body), Ok(display)) = (store.font("dungeon.TTF"), store.font("BATTLE3.TTF")) else {
        return false;
    };
    let mut fonts = FontDefinitions::default();
    fonts
        .font_data
        .insert("dungeon".into(), Arc::new(FontData::from_owned(body)));
    fonts
        .font_data
        .insert("battle".into(), Arc::new(FontData::from_owned(display)));
    // Keep egui's built-in fonts behind ours so glyphs the originals lack (₡, arrows) still draw.
    // The built-in monospace face is in the chain too: it has the credit sign (₡) that the originals lack.
    let mut fallback = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    fallback.extend(
        fonts
            .families
            .get(&FontFamily::Monospace)
            .cloned()
            .unwrap_or_default(),
    );
    fallback.dedup();
    let mut body_family = vec!["dungeon".to_string()];
    body_family.extend(fallback.iter().cloned());
    fonts.families.insert(FontFamily::Proportional, body_family);
    let mut display_family = vec!["battle".to_string()];
    display_family.extend(fallback);
    fonts.families.insert(heading_family(), display_family);
    ctx.set_fonts(fonts);
    true
}

/// Everything or nothing for the map; individual icons and banners are best-effort.
/// Errors come back as text for the UI to show once.
pub fn load(ctx: &egui::Context, store: &mut Store) -> Result<Assets, String> {
    let fonts = install_fonts(ctx, store);
    let map = texture(ctx, store, "worldmaplarge.tif", "uplink_map")?;
    let mask_img = store
        .image("worldmaplarge_mask.tif")
        .map_err(|e| e.to_string())?;
    let mask = LandMask::from_rgba(
        mask_img.width as usize,
        mask_img.height as usize,
        &mask_img.pixels,
    );
    let icons = ICONS
        .iter()
        .filter_map(|(k, f)| texture(ctx, store, f, k).ok().map(|t| (*k, t)))
        .collect();
    Ok(Assets {
        map,
        mask,
        icons,
        granted: texture(ctx, store, "accessgranted.tif", "granted").ok(),
        denied: texture(ctx, store, "accessdenied.tif", "denied").ok(),
        fonts,
    })
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
