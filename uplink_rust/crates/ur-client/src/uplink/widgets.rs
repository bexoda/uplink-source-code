//! The original's button looks, ported from `opengl_interface.cpp`.

use eframe::egui::Color32;

use super::canvas::{Canvas, Hit, Size};
use super::Res;

/// `button_draw`: a diagonal gradient (Normal / Highlighted / Clicked), caption centred.
pub fn button_box(
    c: &mut Canvas,
    res: &Res,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    caption: &str,
    hit: Hit,
) {
    button_box_state(c, res, x, y, w, h, caption, hit.hover, hit.down);
}

pub fn button_box_state(
    c: &mut Canvas,
    res: &Res,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    caption: &str,
    highlighted: bool,
    clicked: bool,
) {
    let t = &res.theme;
    let (a, b) = if clicked {
        (t.c("ButtonClickedA"), t.c("ButtonClickedB"))
    } else if highlighted {
        (t.c("ButtonHighlightedA"), t.c("ButtonHighlightedB"))
    } else {
        (t.c("ButtonNormalA"), t.c("ButtonNormalB"))
    };
    c.gradient(x, y, w, h, [a, b, a, b]);
    let tx = x + w / 2.0 - c.text_width(caption, Size::Small) / 2.0;
    c.text(
        tx,
        y + h / 2.0 + 2.0,
        caption,
        Size::Small,
        t.c("DefaultText"),
    );
}

/// `imagebutton_draw`: pick the clicked / highlighted / standard image. `base` is the standard
/// image path; the others follow the game's `_h` / `_c` naming.
pub fn image_button(
    c: &mut Canvas,
    res: &Res,
    x: f32,
    y: f32,
    base: &str,
    hit: Hit,
    force_clicked: bool,
) {
    let stem = base.strip_suffix(".tif").unwrap_or(base);
    let name = if hit.down || force_clicked {
        format!("{stem}_c.tif")
    } else if hit.hover {
        format!("{stem}_h.tif")
    } else {
        base.to_string()
    };
    let sprite = res.sprites.get(&name).or_else(|| res.sprites.get(base));
    if let Some(s) = sprite {
        c.sprite(s.id, s.w, s.h, x, y, Color32::WHITE);
    }
}

/// Draws a plain sprite (no states).
pub fn picture(c: &mut Canvas, res: &Res, x: f32, y: f32, name: &str) -> Option<(f32, f32)> {
    res.sprites.get(name).map(|s| {
        c.sprite(s.id, s.w, s.h, x, y, Color32::WHITE);
        (s.w, s.h)
    })
}

/// A sprite stretched to a box, like `Image::Scale`.
pub fn picture_scaled(c: &mut Canvas, res: &Res, x: f32, y: f32, w: f32, h: f32, name: &str) {
    if let Some(s) = res.sprites.get(name) {
        c.sprite(s.id, w, h, x, y, Color32::WHITE);
    }
}

/// The vertical panel gradient used behind local screens.
pub fn panel(c: &mut Canvas, res: &Res, x: f32, y: f32, w: f32, h: f32) {
    let t = &res.theme;
    let (a, b) = (t.c("PanelBackgroundA"), t.c("PanelBackgroundB"));
    c.gradient(x, y, w, h, [a, b, a, b]);
    c.border(x, y, w, h, t.c("PanelBorder"));
}

/// Like `image_button`, for the original's `_blend` sprites (see-through edges).
pub fn image_button_blend(
    c: &mut Canvas,
    res: &Res,
    x: f32,
    y: f32,
    base: &str,
    hit: Hit,
    force_clicked: bool,
) {
    let stem = base.strip_suffix(".tif").unwrap_or(base);
    let name = if hit.down || force_clicked {
        format!("{stem}_c.tif")
    } else if hit.hover {
        format!("{stem}_h.tif")
    } else {
        base.to_string()
    };
    if let Some(s) = res
        .sprites
        .get_blend(&name)
        .or_else(|| res.sprites.get_blend(base))
    {
        c.sprite(s.id, s.w, s.h, x, y, Color32::WHITE);
    }
}
