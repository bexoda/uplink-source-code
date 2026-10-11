//! A 1024x768 virtual screen, scaled to fit the window, with drawing and hit-testing that mirror
//! the original's immediate button system: later-drawn things are on top, a press only counts as a
//! click if it is released over the same button, and every button can carry a tooltip.

use eframe::egui::{
    self, epaint, vec2, Align2, Color32, FontFamily, FontId, Mesh, Painter, Pos2, Rect, Shape,
    Stroke,
};

pub const W: f32 = 1024.0;
pub const H: f32 = 768.0;

/// The three text sizes the game loads from `dungeon.ttf` (HELVETICA_10, _12, _18).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Size {
    Small,
    Medium,
    Large,
}

impl Size {
    fn px(self) -> f32 {
        match self {
            Size::Small => 9.0,
            Size::Medium => 11.0,
            Size::Large => 16.0,
        }
    }
}

pub fn uplink_family() -> FontFamily {
    FontFamily::Name("uplink".into())
}

/// FNV-1a over a name and index: stable ids for hit regions.
pub fn id(name: &str, n: u32) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for b in name.bytes().chain(n.to_le_bytes()) {
        h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
    }
    h
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Hit {
    pub hover: bool,
    /// Held down while the pointer is still over it.
    pub down: bool,
    /// Released over the same button it was pressed on.
    pub clicked: bool,
}

/// State that persists across frames.
#[derive(Default)]
pub struct Ecl {
    regions: Vec<(u64, Rect)>,
    hover: Option<u64>,
    pressed: Option<u64>,
    clicked: Option<u64>,
    tooltip_now: String,
    tooltip_shown: String,
}

impl Ecl {
    /// Call once per frame before drawing. `area` is where the 1024x768 screen is shown.
    pub fn begin<'a>(&'a mut self, ui: &egui::Ui, painter: Painter, area: Rect) -> Canvas<'a> {
        let scale = (area.width() / W).min(area.height() / H).max(0.01);
        let size = vec2(W, H) * scale;
        let origin = area.min + (area.size() - size) * 0.5;
        let mouse = ui.input(|i| i.pointer.hover_pos());

        // Resolve last frame's top-most region under the mouse.
        self.hover = mouse.and_then(|m| {
            self.regions
                .iter()
                .rev()
                .find(|(_, r)| r.contains(m))
                .map(|(id, _)| *id)
        });
        let (pressed_now, released_now) =
            ui.input(|i| (i.pointer.primary_pressed(), i.pointer.primary_released()));
        self.clicked = None;
        if pressed_now {
            self.pressed = self.hover;
        }
        if released_now {
            if self.pressed.is_some() && self.pressed == self.hover {
                self.clicked = self.pressed;
            }
            self.pressed = None;
        }
        self.tooltip_shown = std::mem::take(&mut self.tooltip_now);
        self.regions.clear();
        Canvas {
            ecl: self,
            painter,
            origin,
            scale,
        }
    }
}

pub struct Canvas<'a> {
    ecl: &'a mut Ecl,
    painter: Painter,
    origin: Pos2,
    scale: f32,
}

impl Canvas<'_> {
    /// Tooltip of the button the mouse is over, for the bar along the bottom of the screen.
    pub fn tooltip_text(&self) -> String {
        self.ecl.tooltip_shown.clone()
    }

    fn rect(&self, x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::from_min_size(
            self.origin + vec2(x, y) * self.scale,
            vec2(w, h) * self.scale,
        )
    }

    fn pt(&self, x: f32, y: f32) -> Pos2 {
        self.origin + vec2(x, y) * self.scale
    }

    pub fn fill(&self, x: f32, y: f32, w: f32, h: f32, c: Color32) {
        self.painter.rect_filled(self.rect(x, y, w, h), 0.0, c);
    }

    /// Four corner colours in the order the original submits them: bottom-left, top-left,
    /// top-right, bottom-right. (So the gradient runs along the diagonals.)
    pub fn gradient(&self, x: f32, y: f32, w: f32, h: f32, c: [Color32; 4]) {
        let r = self.rect(x, y, w, h);
        let mut m = Mesh::default();
        let uv = epaint::WHITE_UV;
        m.vertices.push(epaint::Vertex {
            pos: r.left_bottom(),
            uv,
            color: c[0],
        });
        m.vertices.push(epaint::Vertex {
            pos: r.left_top(),
            uv,
            color: c[1],
        });
        m.vertices.push(epaint::Vertex {
            pos: r.right_top(),
            uv,
            color: c[2],
        });
        m.vertices.push(epaint::Vertex {
            pos: r.right_bottom(),
            uv,
            color: c[3],
        });
        m.indices.extend([0, 1, 2, 0, 2, 3]);
        self.painter.add(Shape::mesh(m));
    }

    /// `border_draw`: a one-pixel rectangle outline, `width - 1` by `height - 1`.
    pub fn border(&self, x: f32, y: f32, w: f32, h: f32, c: Color32) {
        let r = self.rect(x, y, (w - 1.0).max(0.0), (h - 1.0).max(0.0));
        self.painter.rect_stroke(
            r,
            0.0,
            Stroke::new(self.scale.max(1.0), c),
            egui::StrokeKind::Middle,
        );
    }

    pub fn line(&self, a: (f32, f32), b: (f32, f32), c: Color32, width: f32) {
        self.painter.line_segment(
            [self.pt(a.0, a.1), self.pt(b.0, b.1)],
            Stroke::new(width * self.scale.max(1.0), c),
        );
    }

    /// Dashed polyline, like the original's stippled connection lines. `phase` animates the dashes.
    pub fn dashed(
        &self,
        points: &[(f32, f32)],
        c: Color32,
        width: f32,
        dash: f32,
        gap: f32,
        phase: f32,
    ) {
        let pts: Vec<Pos2> = points.iter().map(|p| self.pt(p.0, p.1)).collect();
        // A negative or huge phase would draw dashes before the start of the line; wrap it into one period.
        let period = (dash + gap).max(1.0);
        let phase = phase.rem_euclid(period);
        let shapes = Shape::dashed_line_with_offset(
            &pts,
            Stroke::new(width * self.scale.max(1.0), c),
            &[dash * self.scale],
            &[gap * self.scale],
            phase * self.scale,
        );
        self.painter.extend(shapes);
    }

    /// Text with `y` as the baseline, like `GciDrawText`.
    pub fn text(&self, x: f32, y: f32, s: &str, size: Size, c: Color32) {
        let px = size.px();
        self.painter.text(
            self.pt(x, y - px * 0.80),
            Align2::LEFT_TOP,
            s,
            FontId::new(px * self.scale, uplink_family()),
            c,
        );
    }

    pub fn text_width(&self, s: &str, size: Size) -> f32 {
        let font = FontId::new(size.px() * self.scale, uplink_family());
        self.painter
            .layout_no_wrap(s.to_string(), font, Color32::WHITE)
            .size()
            .x
            / self.scale
    }

    /// Draws a sprite 1:1 (in virtual pixels), opaque unless `blend`.
    pub fn sprite(&self, tex: egui::TextureId, w: f32, h: f32, x: f32, y: f32, tint: Color32) {
        let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
        self.painter.image(tex, self.rect(x, y, w, h), uv, tint);
    }

    /// Register an interactive region (later calls are on top). Hover/click come from last frame's
    /// layout, which is imperceptible at 60 fps and keeps z-order simple.
    pub fn hit(
        &mut self,
        name: &str,
        n: u32,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        tooltip: &str,
    ) -> Hit {
        let id = id(name, n);
        let r = self.rect(x, y, w, h);
        self.ecl.regions.push((id, r));
        let hover = self.ecl.hover == Some(id);
        if hover && !tooltip.is_empty() {
            self.ecl.tooltip_now = tooltip.to_string();
        }
        Hit {
            hover,
            down: hover && self.ecl.pressed == Some(id),
            clicked: self.ecl.clicked == Some(id),
        }
    }

    /// Clip drawing to a virtual rectangle for the duration of `f` (the original's scissor test).
    pub fn clipped<R>(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        f: impl FnOnce(&mut Canvas) -> R,
    ) -> R {
        let clip = self.rect(x, y, w, h).intersect(self.painter.clip_rect());
        let clipped = self.painter.with_clip_rect(clip);
        let outer = std::mem::replace(&mut self.painter, clipped);
        let r = f(self);
        self.painter = outer;
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_stable_and_distinct() {
        assert_eq!(id("hud_speed", 1), id("hud_speed", 1));
        assert_ne!(id("hud_speed", 1), id("hud_speed", 2));
        assert_ne!(id("a", 0), id("b", 0));
    }
}
