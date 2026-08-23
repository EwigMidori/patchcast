/// Frame rendering and ffmpeg encoding.
///
/// Draws each animation frame as an image using the `image` crate,
/// then pipes frames to ffmpeg for MP4 encoding.
use anyhow::{bail, Context, Result};
use image::{ImageBuffer, Rgba, RgbaImage};
use indicatif::{ProgressBar, ProgressStyle};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::animation::{
    crossfade, fade_in_alpha, lines_revealed, pulse_highlight, scene_progress, slide_in_offset,
};
use crate::diff_parser::DiffLineKind;
use crate::font;
use crate::highlighter::HighlightedLine;
use crate::scene::{change_summary, language_name, Scene, SceneKind, SceneLine};
use crate::style::{Color, Layout, Theme};
use ab_glyph::FontRef;

/// Rendering configuration.
pub struct RenderConfig {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub font_size: u32,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 720,
            fps: 30,
            font_size: 14,
        }
    }
}

/// Render each scene to its own MP4. `output_path` is treated as a directory
/// (a `.mp4` suffix is stripped). Transitions are skipped.
pub fn render_to_video(
    scenes: &[Scene],
    output_path: &Path,
    config: &RenderConfig,
) -> Result<Vec<PathBuf>> {
    let theme = Theme::default();
    let layout = Layout::new(config.font_size);
    let font = font::load_font();

    let clips: Vec<&Scene> = scenes.iter().filter(|s| !s.is_transition()).collect();
    let total_frames: u32 = clips.iter().map(|s| s.duration_frames).sum();
    if total_frames == 0 {
        bail!("No frames to render — the diff may be empty");
    }

    let out_dir = output_dir(output_path);
    fs::create_dir_all(&out_dir)
        .with_context(|| format!("Failed to create {}", out_dir.display()))?;

    let pb = ProgressBar::new(total_frames as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos}/{len} frames ({eta} remaining)")
            .unwrap()
            .progress_chars("##-"),
    );

    let mut written = Vec::new();
    for (i, scene) in clips.iter().enumerate() {
        let clip_path = out_dir.join(format!("{:02}-{}.mp4", i + 1, scene.clip_stem()));
        encode_scene(scene, &clip_path, config, &theme, &layout, &font, &pb)?;
        written.push(clip_path);
    }

    pb.finish_with_message("Encoding complete");
    Ok(written)
}

fn output_dir(output_path: &Path) -> PathBuf {
    if output_path.extension().is_some_and(|e| e == "mp4") {
        output_path.with_extension("")
    } else {
        output_path.to_path_buf()
    }
}

fn spawn_ffmpeg(output_path: &Path, config: &RenderConfig) -> Result<std::process::Child> {
    Command::new("ffmpeg")
        .args([
            "-y",
            "-f", "rawvideo",
            "-pixel_format", "rgba",
            "-video_size", &format!("{}x{}", config.width, config.height),
            "-framerate", &config.fps.to_string(),
            "-i", "-",
            "-c:v", "libx264",
            "-pix_fmt", "yuv420p",
            "-preset", "medium",
            "-crf", "23",
            "-movflags", "+faststart",
        ])
        .arg(output_path.as_os_str())
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("Failed to spawn ffmpeg. Is ffmpeg installed and in PATH?")
}

fn encode_scene(
    scene: &Scene,
    clip_path: &Path,
    config: &RenderConfig,
    theme: &Theme,
    layout: &Layout,
    font: &FontRef<'static>,
    pb: &ProgressBar,
) -> Result<()> {
    let mut ffmpeg = spawn_ffmpeg(clip_path, config)?;
    {
        let stdin = ffmpeg
            .stdin
            .as_mut()
            .context("Failed to open ffmpeg stdin")?;
        for frame_idx in 0..scene.duration_frames {
            let frame = render_frame(
                &scene.kind,
                frame_idx,
                scene.duration_frames,
                config.width,
                config.height,
                theme,
                layout,
                font,
                &None,
            );
            stdin
                .write_all(frame.as_raw())
                .context("Failed to write frame to ffmpeg")?;
            pb.inc(1);
        }
    }
    let status = ffmpeg.wait().context("ffmpeg process failed")?;
    if !status.success() {
        bail!("ffmpeg exited with status: {} ({})", status, clip_path.display());
    }
    Ok(())
}

/// Render a single frame for a given scene.
fn render_frame(
    scene_kind: &SceneKind,
    frame: u32,
    total_frames: u32,
    width: u32,
    height: u32,
    theme: &Theme,
    layout: &Layout,
    font: &FontRef<'static>,
    prev_frame: &Option<RgbaImage>,
) -> RgbaImage {
    match scene_kind {
        SceneKind::TitleCard {
            filename,
            lines_added,
            lines_removed,
            extension,
        } => render_title_card(
            frame,
            total_frames,
            width,
            height,
            theme,
            layout,
            font,
            filename,
            *lines_added,
            *lines_removed,
            extension,
        ),
        SceneKind::CodeReveal {
            filename,
            lines,
            reveal_rate: _,
        } => render_code_reveal(
            frame,
            total_frames,
            width,
            height,
            theme,
            layout,
            font,
            filename,
            lines,
        ),
        SceneKind::DeletionHighlight {
            filename,
            lines,
            deletion_indices,
        } => render_deletion_highlight(
            frame,
            total_frames,
            width,
            height,
            theme,
            layout,
            font,
            filename,
            lines,
            deletion_indices,
        ),
        SceneKind::AdditionHighlight {
            filename,
            lines,
            addition_indices,
        } => render_addition_highlight(
            frame,
            total_frames,
            width,
            height,
            theme,
            layout,
            font,
            filename,
            lines,
            addition_indices,
        ),
        SceneKind::Pause {
            filename,
            lines,
        } => render_static_code(width, height, theme, layout, font, filename, lines),
        SceneKind::Transition => {
            render_transition(frame, total_frames, width, height, theme, prev_frame)
        }
    }
}

/// Fill an image with a solid color.
fn fill_background(img: &mut RgbaImage, color: Color) {
    let rgba = color.to_rgba();
    for pixel in img.pixels_mut() {
        *pixel = rgba;
    }
}

/// Draw a filled rectangle.
fn draw_rect(img: &mut RgbaImage, x: u32, y: u32, w: u32, h: u32, color: Color) {
    let img_w = img.width();
    let img_h = img.height();
    for dy in 0..h {
        for dx in 0..w {
            let px = x + dx;
            let py = y + dy;
            if px < img_w && py < img_h {
                blend_pixel(img, px, py, color);
            }
        }
    }
}

/// Alpha-blend a color onto a pixel.
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
    img.put_pixel(x, y, Rgba([r, g, b, 255]));
}

/// Draw a string of styled characters.
fn draw_styled_line(
    img: &mut RgbaImage,
    font: &FontRef<'static>,
    x_start: u32,
    y: u32,
    line: &HighlightedLine,
    layout: &Layout,
    alpha_mult: f32,
) {
    let mut x = x_start;
    for sc in &line.chars {
        if sc.ch == ' ' || sc.ch == '\t' {
            if sc.ch == '\t' {
                x += layout.char_width * 4;
            } else {
                x += layout.char_width;
            }
            continue;
        }
        let mut fg = sc.fg;
        if alpha_mult < 1.0 {
            fg.a = (fg.a as f32 * alpha_mult) as u8;
        }
        font::draw_glyph(img, font, x, y, sc.ch, fg, layout.char_height);
        x += layout.char_width;
    }
}

/// Draw plain text (not syntax highlighted).
fn draw_plain_text(
    img: &mut RgbaImage,
    font: &FontRef<'static>,
    x_start: u32,
    y: u32,
    text: &str,
    color: Color,
    layout: &Layout,
) {
    let line = HighlightedLine::plain(text, color);
    draw_styled_line(img, font, x_start, y, &line, layout, 1.0);
}

/// Draw the file header bar at the top.
fn draw_header(
    img: &mut RgbaImage,
    font: &FontRef<'static>,
    filename: &str,
    theme: &Theme,
    layout: &Layout,
) {
    let w = img.width();
    draw_rect(img, 0, 0, w, layout.header_height, theme.header_bg);
    let text_y = (layout.header_height - layout.char_height) / 2;
    draw_plain_text(img, font, layout.left_margin, text_y, filename, theme.header_text, layout);
}

/// Draw line numbers in the gutter.
fn draw_line_number(
    img: &mut RgbaImage,
    font: &FontRef<'static>,
    y: u32,
    line_num: u32,
    theme: &Theme,
    layout: &Layout,
) {
    let num_str = format!("{:>4}", line_num);
    let x = layout.left_margin;
    draw_plain_text(img, font, x, y, &num_str, theme.line_number, layout);
}

/// Draw the diff border indicator (green for added, red for removed).
fn draw_diff_border(
    img: &mut RgbaImage,
    y: u32,
    line_height: u32,
    kind: &DiffLineKind,
    theme: &Theme,
    layout: &Layout,
) {
    let border_x = layout.left_margin + layout.gutter_width;
    let color = match kind {
        DiffLineKind::Added => theme.added_border,
        DiffLineKind::Removed => theme.removed_border,
        DiffLineKind::Context => return,
    };
    draw_rect(img, border_x, y, layout.border_width, line_height, color);
}

/// Render a title card scene.
fn render_title_card(
    frame: u32,
    total_frames: u32,
    width: u32,
    height: u32,
    theme: &Theme,
    layout: &Layout,
    font: &FontRef<'static>,
    filename: &str,
    lines_added: u32,
    lines_removed: u32,
    extension: &str,
) -> RgbaImage {
    let mut img = ImageBuffer::new(width, height);
    fill_background(&mut img, theme.background);

    let alpha = fade_in_alpha(frame, total_frames);

    // Center the title card content vertically.
    let center_y = height / 2;
    let lang = language_name(extension);
    let summary = change_summary(lines_added, lines_removed);

    // Draw filename (large, centered).
    let filename_x = (width / 2).saturating_sub((filename.len() as u32 * layout.char_width) / 2);
    let filename_y = center_y.saturating_sub(layout.char_height * 2);
    let mut title_color = theme.title_accent;
    title_color.a = (255.0 * alpha) as u8;
    draw_plain_text(&mut img, font, filename_x, filename_y, filename, title_color, layout);

    // Draw language name.
    let lang_x = (width / 2).saturating_sub((lang.len() as u32 * layout.char_width) / 2);
    let lang_y = center_y;
    let mut lang_color = theme.text_dim;
    lang_color.a = (255.0 * alpha) as u8;
    draw_plain_text(&mut img, font, lang_x, lang_y, lang, lang_color, layout);

    // Draw change summary.
    let summary_x = (width / 2).saturating_sub((summary.len() as u32 * layout.char_width) / 2);
    let summary_y = center_y + layout.char_height * 2;
    let mut summary_color = theme.text;
    summary_color.a = (255.0 * alpha) as u8;
    draw_plain_text(&mut img, font, summary_x, summary_y, &summary, summary_color, layout);

    img
}

/// Render a code reveal scene (lines appear progressively).
fn render_code_reveal(
    frame: u32,
    total_frames: u32,
    width: u32,
    height: u32,
    theme: &Theme,
    layout: &Layout,
    font: &FontRef<'static>,
    filename: &str,
    lines: &[SceneLine],
) -> RgbaImage {
    let mut img = ImageBuffer::new(width, height);
    fill_background(&mut img, theme.background);
    draw_header(&mut img, font, filename, theme, layout);

    let visible = lines_revealed(frame, total_frames, lines.len());
    let code_y_start = layout.header_height + layout.top_margin;

    for (i, line) in lines.iter().enumerate() {
        if i >= visible {
            break;
        }
        let y = code_y_start + i as u32 * layout.line_height();
        if y + layout.char_height > height {
            break;
        }

        draw_line_number(&mut img, font, y, line.line_number, theme, layout);
        draw_diff_border(&mut img, y, layout.line_height(), &line.kind, theme, layout);
        draw_styled_line(&mut img, font, layout.code_x_start(), y, &line.highlighted, layout, 1.0);
    }

    img
}

/// Render deletion highlight scene (removed lines pulse red).
fn render_deletion_highlight(
    frame: u32,
    total_frames: u32,
    width: u32,
    height: u32,
    theme: &Theme,
    layout: &Layout,
    font: &FontRef<'static>,
    filename: &str,
    lines: &[SceneLine],
    deletion_indices: &[usize],
) -> RgbaImage {
    let mut img = ImageBuffer::new(width, height);
    fill_background(&mut img, theme.background);
    draw_header(&mut img, font, filename, theme, layout);

    let pulse_alpha = pulse_highlight(frame, total_frames, 0.2, 0.6);
    let code_y_start = layout.header_height + layout.top_margin;

    for (i, line) in lines.iter().enumerate() {
        let y = code_y_start + i as u32 * layout.line_height();
        if y + layout.char_height > height {
            break;
        }

        let is_deletion = deletion_indices.contains(&i);

        if is_deletion {
            // Draw red background highlight.
            let bg = Color::with_alpha(
                theme.removed_border.r,
                theme.removed_border.g,
                theme.removed_border.b,
                (255.0 * pulse_alpha) as u8,
            );
            draw_rect(&mut img, layout.left_margin, y, width - layout.left_margin * 2, layout.line_height(), bg);
        }

        draw_line_number(&mut img, font, y, line.line_number, theme, layout);
        draw_diff_border(&mut img, y, layout.line_height(), &line.kind, theme, layout);
        draw_styled_line(&mut img, font, layout.code_x_start(), y, &line.highlighted, layout, 1.0);
    }

    img
}

/// Render addition highlight scene (new lines slide in with green highlight).
fn render_addition_highlight(
    frame: u32,
    total_frames: u32,
    width: u32,
    height: u32,
    theme: &Theme,
    layout: &Layout,
    font: &FontRef<'static>,
    filename: &str,
    lines: &[SceneLine],
    addition_indices: &[usize],
) -> RgbaImage {
    let mut img = ImageBuffer::new(width, height);
    fill_background(&mut img, theme.background);
    draw_header(&mut img, font, filename, theme, layout);

    let progress = scene_progress(frame, total_frames);
    let slide_offset = slide_in_offset(frame, total_frames, 50.0) as u32;
    let highlight_alpha = if progress < 0.5 {
        0.4
    } else {
        // Fade the highlight out in the second half.
        0.4 * (1.0 - (progress - 0.5) * 2.0).max(0.0)
    };

    let code_y_start = layout.header_height + layout.top_margin;

    for (i, line) in lines.iter().enumerate() {
        let y = code_y_start + i as u32 * layout.line_height();
        if y + layout.char_height > height {
            break;
        }

        let is_addition = addition_indices.contains(&i);
        let x_offset = if is_addition { slide_offset } else { 0 };

        if is_addition && highlight_alpha > 0.01 {
            let bg = Color::with_alpha(
                theme.added_border.r,
                theme.added_border.g,
                theme.added_border.b,
                (255.0 * highlight_alpha) as u8,
            );
            draw_rect(&mut img, layout.left_margin + x_offset, y, width - layout.left_margin * 2, layout.line_height(), bg);
        }

        draw_line_number(&mut img, font, y, line.line_number, theme, layout);
        draw_diff_border(&mut img, y, layout.line_height(), &line.kind, theme, layout);
        draw_styled_line(
            &mut img,
            font,
            layout.code_x_start() + x_offset,
            y,
            &line.highlighted,
            layout,
            1.0,
        );
    }

    img
}

/// Render a static code view (for pause scenes).
fn render_static_code(
    width: u32,
    height: u32,
    theme: &Theme,
    layout: &Layout,
    font: &FontRef<'static>,
    filename: &str,
    lines: &[SceneLine],
) -> RgbaImage {
    let mut img = ImageBuffer::new(width, height);
    fill_background(&mut img, theme.background);
    draw_header(&mut img, font, filename, theme, layout);

    let code_y_start = layout.header_height + layout.top_margin;

    for (i, line) in lines.iter().enumerate() {
        let y = code_y_start + i as u32 * layout.line_height();
        if y + layout.char_height > height {
            break;
        }

        draw_line_number(&mut img, font, y, line.line_number, theme, layout);
        draw_diff_border(&mut img, y, layout.line_height(), &line.kind, theme, layout);
        draw_styled_line(&mut img, font, layout.code_x_start(), y, &line.highlighted, layout, 1.0);
    }

    img
}

/// Render a crossfade transition between scenes.
fn render_transition(
    frame: u32,
    total_frames: u32,
    width: u32,
    height: u32,
    theme: &Theme,
    prev_frame: &Option<RgbaImage>,
) -> RgbaImage {
    let (old_alpha, _new_alpha) = crossfade(frame, total_frames);

    if let Some(prev) = prev_frame {
        // Blend previous frame with background.
        let mut img = ImageBuffer::new(width, height);
        let bg = theme.background.to_rgba();

        for y in 0..height {
            for x in 0..width {
                if x < prev.width() && y < prev.height() {
                    let old_pixel = prev.get_pixel(x, y);
                    let r = (old_pixel[0] as f32 * old_alpha + bg[0] as f32 * (1.0 - old_alpha)) as u8;
                    let g = (old_pixel[1] as f32 * old_alpha + bg[1] as f32 * (1.0 - old_alpha)) as u8;
                    let b = (old_pixel[2] as f32 * old_alpha + bg[2] as f32 * (1.0 - old_alpha)) as u8;
                    img.put_pixel(x, y, Rgba([r, g, b, 255]));
                } else {
                    img.put_pixel(x, y, bg);
                }
            }
        }
        img
    } else {
        let mut img = ImageBuffer::new(width, height);
        fill_background(&mut img, theme.background);
        img
    }
}

/// Check that ffmpeg is available.
pub fn check_ffmpeg() -> Result<()> {
    let output = Command::new("ffmpeg")
        .arg("-version")
        .output()
        .context("ffmpeg not found. Please install ffmpeg: https://ffmpeg.org/download.html")?;

    if !output.status.success() {
        bail!("ffmpeg returned an error when checking version");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fill_background() {
        let mut img = ImageBuffer::new(10, 10);
        let color = Color::new(30, 30, 46);
        fill_background(&mut img, color);
        let pixel = img.get_pixel(5, 5);
        assert_eq!(pixel[0], 30);
        assert_eq!(pixel[1], 30);
        assert_eq!(pixel[2], 46);
        assert_eq!(pixel[3], 255);
    }

    #[test]
    fn test_draw_rect() {
        let mut img = ImageBuffer::new(100, 100);
        fill_background(&mut img, Color::new(0, 0, 0));
        draw_rect(&mut img, 10, 10, 20, 20, Color::new(255, 0, 0));
        let pixel = img.get_pixel(20, 20);
        assert_eq!(pixel[0], 255);
        assert_eq!(pixel[1], 0);
        assert_eq!(pixel[2], 0);
    }

    #[test]
    fn test_draw_rect_bounds() {
        // Should not panic when drawing outside bounds.
        let mut img = ImageBuffer::new(10, 10);
        fill_background(&mut img, Color::new(0, 0, 0));
        draw_rect(&mut img, 5, 5, 100, 100, Color::new(255, 255, 255));
    }

    #[test]
    fn test_blend_pixel_alpha() {
        let mut img: RgbaImage = ImageBuffer::new(10, 10);
        fill_background(&mut img, Color::new(0, 0, 0));
        blend_pixel(&mut img, 5, 5, Color::with_alpha(255, 0, 0, 128));
        let pixel = img.get_pixel(5, 5);
        // Red channel should be approximately half of 255.
        assert!(pixel[0] > 100 && pixel[0] < 200);
        assert!(pixel[1] < 10);
    }

    #[test]
    fn test_render_config_default() {
        let config = RenderConfig::default();
        assert_eq!(config.width, 1280);
        assert_eq!(config.height, 720);
        assert_eq!(config.fps, 30);
    }

    #[test]
    fn test_check_ffmpeg() {
        // This test depends on ffmpeg being installed, so we just check it doesn't panic.
        let result = check_ffmpeg();
        // Either succeeds or gives a clear error.
        if result.is_err() {
            let err = result.unwrap_err();
            assert!(
                format!("{:?}", err).contains("ffmpeg"),
                "Error should mention ffmpeg"
            );
        }
    }

    #[test]
    fn test_title_card_renders() {
        let theme = Theme::default();
        let layout = Layout::new(14);
        let font = font::load_font();
        let img = render_title_card(
            15, 30, 640, 480, &theme, &layout, &font,
            "src/main.rs", 5, 2, "rs",
        );
        assert_eq!(img.width(), 640);
        assert_eq!(img.height(), 480);
    }

    #[test]
    fn test_static_code_renders() {
        let theme = Theme::default();
        let layout = Layout::new(14);
        let lines = vec![
            SceneLine {
                highlighted: HighlightedLine::plain("fn main() {}", Color::new(200, 200, 200)),
                kind: DiffLineKind::Context,
                line_number: 1,
            },
        ];
        let font = font::load_font();
        let img = render_static_code(640, 480, &theme, &layout, &font, "test.rs", &lines);
        assert_eq!(img.width(), 640);
        assert_eq!(img.height(), 480);
    }
}
