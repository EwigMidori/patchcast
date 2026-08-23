/// Bundled monospace font for glyph rasterization.
use ab_glyph::{Font, FontRef, PxScale, ScaleFont, point};
use image::RgbaImage;

use crate::style::Color;

const FONT_TTF: &[u8] = include_bytes!("../assets/JetBrainsMono-Regular.ttf");

/// Load the bundled JetBrains Mono Regular face.
pub fn load_font() -> FontRef<'static> {
    FontRef::try_from_slice(FONT_TTF).expect("bundled JetBrains Mono TTF is valid")
}

/// Pixel scale matching a cell of `char_h` pixels tall.
pub fn cell_scale(char_h: u32) -> PxScale {
    PxScale::from(char_h as f32)
}

/// Rasterize one glyph into `img` at cell origin `(x, y)`.
pub fn draw_glyph(
    img: &mut RgbaImage,
    font: &FontRef<'static>,
    x: u32,
    y: u32,
    ch: char,
    fg: Color,
    char_h: u32,
) {
    let scale = cell_scale(char_h);
    let scaled = font.as_scaled(scale);
    let mut glyph = font.glyph_id(ch).with_scale(scale);
    glyph.position = point(x as f32, y as f32 + scaled.ascent());

    let Some(outlined) = font.outline_glyph(glyph) else {
        return;
    };
    let bounds = outlined.px_bounds();
    outlined.draw(|gx, gy, coverage| {
        if coverage <= 0.0 {
            return;
        }
        let px = bounds.min.x as i32 + gx as i32;
        let py = bounds.min.y as i32 + gy as i32;
        if px < 0 || py < 0 {
            return;
        }
        let mut color = fg;
        color.a = (fg.a as f32 * coverage.clamp(0.0, 1.0)) as u8;
        blend_pixel(img, px as u32, py as u32, color);
    });
}

fn blend_pixel(img: &mut RgbaImage, x: u32, y: u32, color: Color) {
    if x >= img.width() || y >= img.height() {
        return;
    }
    if color.a == 255 {
        img.put_pixel(x, y, color.to_rgba());
        return;
    }
    let existing = img.get_pixel(x, y);
    let alpha = color.a as f32 / 255.0;
    let inv = 1.0 - alpha;
    let r = (color.r as f32 * alpha + existing[0] as f32 * inv) as u8;
    let g = (color.g as f32 * alpha + existing[1] as f32 * inv) as u8;
    let b = (color.b as f32 * alpha + existing[2] as f32 * inv) as u8;
    img.put_pixel(x, y, image::Rgba([r, g, b, 255]));
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::ImageBuffer;

    #[test]
    fn glyphs_differ_by_character() {
        let font = load_font();
        let mut a: RgbaImage = ImageBuffer::from_pixel(32, 32, image::Rgba([0, 0, 0, 255]));
        let mut i: RgbaImage = ImageBuffer::from_pixel(32, 32, image::Rgba([0, 0, 0, 255]));
        let fg = Color::new(255, 255, 255);
        draw_glyph(&mut a, &font, 2, 2, 'A', fg, 20);
        draw_glyph(&mut i, &font, 2, 2, 'i', fg, 20);
        assert_ne!(a.as_raw(), i.as_raw());
        let lit = a.pixels().filter(|p| p[0] > 20).count();
        assert!(lit > 10, "glyph A should light more than a handful of pixels, got {lit}");
    }
}
