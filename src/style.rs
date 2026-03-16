/// Colors, theme configuration, and layout constants for patchcast rendering.

/// RGBA color representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub const fn with_alpha(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Linearly interpolate between two colors by factor t in [0.0, 1.0].
    pub fn lerp(self, other: Color, t: f32) -> Color {
        let t = t.clamp(0.0, 1.0);
        let inv = 1.0 - t;
        Color {
            r: (self.r as f32 * inv + other.r as f32 * t) as u8,
            g: (self.g as f32 * inv + other.g as f32 * t) as u8,
            b: (self.b as f32 * inv + other.b as f32 * t) as u8,
            a: (self.a as f32 * inv + other.a as f32 * t) as u8,
        }
    }

    pub fn to_rgba(self) -> image::Rgba<u8> {
        image::Rgba([self.r, self.g, self.b, self.a])
    }
}

/// Catppuccin Mocha-inspired dark theme.
pub struct Theme {
    pub background: Color,
    pub surface: Color,
    pub text: Color,
    pub text_dim: Color,
    pub line_number: Color,
    pub added_border: Color,
    pub added_bg: Color,
    pub removed_border: Color,
    pub removed_bg: Color,
    pub header_bg: Color,
    pub header_text: Color,
    pub title_accent: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            background: Color::new(30, 30, 46),      // #1e1e2e
            surface: Color::new(49, 50, 68),          // #313244
            text: Color::new(205, 214, 244),          // #cdd6f4
            text_dim: Color::new(108, 112, 134),      // #6c7086
            line_number: Color::new(88, 91, 112),     // #585b70
            added_border: Color::new(166, 227, 161),  // #a6e3a1
            added_bg: Color::with_alpha(166, 227, 161, 30),  // semi-transparent green
            removed_border: Color::new(243, 139, 168),// #f38ba8
            removed_bg: Color::with_alpha(243, 139, 168, 30),// semi-transparent red
            header_bg: Color::new(24, 24, 37),        // #181825
            header_text: Color::new(137, 180, 250),   // #89b4fa
            title_accent: Color::new(203, 166, 247),  // #cba6f7
        }
    }
}

/// Layout constants for rendering.
pub struct Layout {
    pub char_width: u32,
    pub char_height: u32,
    pub line_padding: u32,
    pub gutter_width: u32,
    pub left_margin: u32,
    pub top_margin: u32,
    pub header_height: u32,
    pub border_width: u32,
}

impl Layout {
    pub fn new(font_size: u32) -> Self {
        // Scale character dimensions proportionally to font size.
        // Base: font_size=14 -> char 8x16
        let char_width = (font_size as f32 * 0.57).round() as u32;
        let char_height = (font_size as f32 * 1.14).round() as u32;
        let line_padding = char_height / 4;
        let gutter_width = char_width * 5 + 8; // 5 digit line numbers + padding
        let left_margin = 12;
        let top_margin = 8;
        let header_height = char_height * 2 + 16;
        let border_width = 3;

        Self {
            char_width,
            char_height,
            line_padding,
            gutter_width,
            left_margin,
            top_margin,
            header_height,
            border_width,
        }
    }

    /// Total height per line of code (character height + padding).
    pub fn line_height(&self) -> u32 {
        self.char_height + self.line_padding
    }

    /// X offset where code text starts (after gutter and margin).
    pub fn code_x_start(&self) -> u32 {
        self.left_margin + self.gutter_width + self.border_width + 8
    }

    /// How many lines of code fit in the viewport.
    pub fn visible_lines(&self, viewport_height: u32) -> u32 {
        let available = viewport_height.saturating_sub(self.header_height + self.top_margin * 2);
        available / self.line_height()
    }
}

/// Easing functions for smooth animation transitions.
pub fn ease_in_out_cubic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
    }
}

pub fn ease_out_quad(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t) * (1.0 - t)
}

pub fn ease_in_quad(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_lerp() {
        let black = Color::new(0, 0, 0);
        let white = Color::new(255, 255, 255);

        let mid = black.lerp(white, 0.5);
        assert!((mid.r as i32 - 127).abs() <= 1);
        assert!((mid.g as i32 - 127).abs() <= 1);

        let start = black.lerp(white, 0.0);
        assert_eq!(start.r, 0);

        let end = black.lerp(white, 1.0);
        assert_eq!(end.r, 255);
    }

    #[test]
    fn test_layout_dimensions() {
        let layout = Layout::new(14);
        assert!(layout.char_width > 0);
        assert!(layout.char_height > 0);
        assert!(layout.line_height() > layout.char_height);
        assert!(layout.code_x_start() > layout.gutter_width);
    }

    #[test]
    fn test_visible_lines() {
        let layout = Layout::new(14);
        let lines = layout.visible_lines(720);
        assert!(lines > 10, "Should fit at least 10 lines in 720p");
        assert!(lines < 100, "Should not exceed 100 lines in 720p");
    }

    #[test]
    fn test_easing_boundaries() {
        assert!((ease_in_out_cubic(0.0)).abs() < 0.001);
        assert!((ease_in_out_cubic(1.0) - 1.0).abs() < 0.001);
        assert!((ease_out_quad(0.0)).abs() < 0.001);
        assert!((ease_out_quad(1.0) - 1.0).abs() < 0.001);
        assert!((ease_in_quad(0.0)).abs() < 0.001);
        assert!((ease_in_quad(1.0) - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_easing_monotonic() {
        let mut prev = 0.0f32;
        for i in 0..=100 {
            let t = i as f32 / 100.0;
            let v = ease_in_out_cubic(t);
            assert!(v >= prev - 0.001, "Easing should be monotonically increasing");
            prev = v;
        }
    }
}
