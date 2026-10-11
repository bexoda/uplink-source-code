//! On-demand texture cache over the player's own Uplink archives.

use std::cell::RefCell;
use std::collections::HashMap;

use eframe::egui::{self, ColorImage, TextureHandle, TextureId, TextureOptions};
use gl_assets::Store;

#[derive(Clone, Copy, Debug)]
pub struct Sprite {
    pub id: TextureId,
    pub w: f32,
    pub h: f32,
}

pub struct Sprites {
    store: RefCell<Store>,
    ctx: egui::Context,
    /// The theme's background colour: what `_blend` sprites treat as see-through at their edges.
    background: [u8; 3],
    cache: RefCell<HashMap<String, Option<(TextureHandle, Sprite)>>>,
}

impl Sprites {
    pub fn new(ctx: egui::Context, store: Store, background: [u8; 3]) -> Sprites {
        Sprites {
            store: RefCell::new(store),
            ctx,
            background,
            cache: RefCell::new(HashMap::new()),
        }
    }

    /// A sprite by its path under `graphics/` (e.g. `"hud/status.tif"`). The original blits these
    /// opaquely, so alpha is forced to 255. Missing or undecodable sprites return `None` once, and are
    /// remembered so a bad file doesn't cost a decode attempt every frame.
    pub fn get(&self, name: &str) -> Option<Sprite> {
        self.load(name, false)
    }

    /// The original's `_blend` sprites: pixels matching the background colour that connect to the image's
    /// border become transparent (`Image::SetAlphaBorder`), so only the artwork shows.
    pub fn get_blend(&self, name: &str) -> Option<Sprite> {
        self.load(name, true)
    }

    fn load(&self, name: &str, blend: bool) -> Option<Sprite> {
        let key = format!("{}{name}", if blend { "b:" } else { "o:" });
        if let Some(hit) = self.cache.borrow().get(&key) {
            return hit.as_ref().map(|(_, s)| *s);
        }
        let loaded = self.store.borrow_mut().image(name).ok().map(|mut img| {
            img.pixels.chunks_exact_mut(4).for_each(|p| p[3] = 255);
            if blend {
                clear_border(
                    &mut img.pixels,
                    img.width as usize,
                    img.height as usize,
                    self.background,
                );
            }
            let color = ColorImage::from_rgba_unmultiplied(
                [img.width as usize, img.height as usize],
                &img.pixels,
            );
            let tex = self.ctx.load_texture(&key, color, TextureOptions::LINEAR);
            let s = Sprite {
                id: tex.id(),
                w: img.width as f32,
                h: img.height as f32,
            };
            (tex, s)
        });
        let out = loaded.as_ref().map(|(_, s)| *s);
        self.cache.borrow_mut().insert(key, loaded);
        out
    }
}

/// Flood fill from every edge pixel, clearing alpha on pixels equal to `bg`. Iterative, so a big
/// image can't overflow the stack.
fn clear_border(px: &mut [u8], w: usize, h: usize, bg: [u8; 3]) {
    let mut stack: Vec<(usize, usize)> = Vec::new();
    for x in 0..w {
        stack.push((x, 0));
        stack.push((x, h.saturating_sub(1)));
    }
    for y in 0..h {
        stack.push((0, y));
        stack.push((w.saturating_sub(1), y));
    }
    while let Some((x, y)) = stack.pop() {
        if x >= w || y >= h {
            continue;
        }
        let i = (y * w + x) * 4;
        if px[i + 3] == 0 || px[i] != bg[0] || px[i + 1] != bg[1] || px[i + 2] != bg[2] {
            continue;
        }
        px[i + 3] = 0;
        if x > 0 {
            stack.push((x - 1, y));
        }
        if y > 0 {
            stack.push((x, y - 1));
        }
        stack.push((x + 1, y));
        stack.push((x, y + 1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_background_connected_to_the_border_goes_clear() {
        // 5x5, white ring? No: black border, a black pixel enclosed by white in the middle, white cross.
        let mut px = vec![0u8; 5 * 5 * 4];
        for p in px.chunks_exact_mut(4) {
            p.copy_from_slice(&[0, 0, 0, 255]);
        }
        // A white square ring at (1..=3, 1..=3) with black centre (2,2).
        for y in 1..=3 {
            for x in 1..=3 {
                if (x, y) != (2, 2) {
                    let i = (y * 5 + x) * 4;
                    px[i..i + 4].copy_from_slice(&[255, 255, 255, 255]);
                }
            }
        }
        clear_border(&mut px, 5, 5, [0, 0, 0]);
        let alpha = |x: usize, y: usize| px[(y * 5 + x) * 4 + 3];
        assert_eq!(alpha(0, 0), 0, "outside the ring is cleared");
        assert_eq!(alpha(4, 2), 0);
        assert_eq!(alpha(1, 1), 255, "the artwork stays");
        assert_eq!(alpha(2, 2), 255, "an enclosed black pixel stays");
    }
}
