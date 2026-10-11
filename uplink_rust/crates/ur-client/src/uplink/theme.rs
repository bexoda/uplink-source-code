//! The game's colour theme, parsed from `graphics/theme.txt` in the player's own `patch.dat`.
//! Same format the original reads: header lines, then `Name  r  g  b` with 0..1 floats.

use std::collections::HashMap;

use eframe::egui::Color32;

#[derive(Clone, Debug, Default)]
pub struct Theme {
    colours: HashMap<String, [f32; 3]>,
}

impl Theme {
    pub fn parse(text: &str) -> Theme {
        let mut colours = HashMap::new();
        for line in text.lines() {
            let line = line.trim();
            if line.len() < 2 || line.starts_with(';') {
                continue;
            }
            let parts: Vec<&str> = line.split_whitespace().collect();
            // Header lines ("NAME: ...") have non-numeric fields and are skipped naturally.
            if let [name, r, g, b] = parts[..] {
                if let (Ok(r), Ok(g), Ok(b)) =
                    (r.parse::<f32>(), g.parse::<f32>(), b.parse::<f32>())
                {
                    colours.insert(name.to_string(), [r, g, b]);
                }
            }
        }
        Theme { colours }
    }

    pub fn len(&self) -> usize {
        self.colours.len()
    }

    /// A named colour; unknown names are black, as in the original.
    pub fn c(&self, name: &str) -> Color32 {
        self.rgba(name, 1.0)
    }

    pub fn rgba(&self, name: &str, alpha: f32) -> Color32 {
        let [r, g, b] = self.colours.get(name).copied().unwrap_or([0.0; 3]);
        let u = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
        Color32::from_rgba_unmultiplied(u(r), u(g), u(b), u(alpha))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "NAME:       Uplink Blue\nAUTHOR:     X\nCOMMENT:    y\n\n; COLOURNAME  RED GREEN BLUE\n\nBackground 0.0 0.0 0.0\nButtonNormalA 0.0 0.0 0.7\nPanelBorder 0.31 0.54 0.84\n";

    #[test]
    fn parses_colours_and_skips_headers_and_comments() {
        let t = Theme::parse(SAMPLE);
        assert_eq!(t.len(), 3);
        assert_eq!(t.c("ButtonNormalA"), Color32::from_rgb(0, 0, 179));
        assert_eq!(t.c("PanelBorder"), Color32::from_rgb(79, 138, 214));
        assert_eq!(t.c("Nope"), Color32::from_rgb(0, 0, 0));
    }

    #[test]
    fn garbage_is_harmless() {
        assert_eq!(Theme::parse("").len(), 0);
        assert_eq!(Theme::parse("a b c d\nx 1 2\n; 1 2 3 4").len(), 0);
    }
}
