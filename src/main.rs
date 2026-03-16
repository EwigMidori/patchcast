//! patchcast CLI — Turn any git diff into an animated code walkthrough video.
use anyhow::{bail, Context, Result};
use clap::Parser;
use std::path::PathBuf;

use patchcast::diff_parser::{generate_git_diff, parse_diff};
use patchcast::highlighter::Highlighter;
use patchcast::renderer::{check_ffmpeg, render_to_video, RenderConfig};
use patchcast::scene::{generate_scenes, total_frames, SceneConfig};

/// Turn any git diff or PR into an animated code walkthrough video.
#[derive(Parser, Debug)]
#[command(name = "patchcast")]
#[command(version)]
#[command(about = "Turn any git diff into an animated code walkthrough video")]
#[command(long_about = "patchcast renders syntax-highlighted, animated code walkthroughs \
    from git diffs. Each file change becomes a scene with smooth transitions, \
    highlighted additions and deletions, and professional dark-theme styling.")]
struct Cli {
    /// Git diff spec (e.g. HEAD~1, main..feature-branch).
    #[arg(long, conflicts_with = "file")]
    diff: Option<String>,

    /// Path to a .diff or .patch file.
    #[arg(long, conflicts_with = "diff")]
    file: Option<PathBuf>,

    /// Output video file path.
    #[arg(short, long, default_value = "output.mp4")]
    output: PathBuf,

    /// Syntect theme name for syntax highlighting.
    #[arg(long, default_value = "base16-ocean.dark")]
    theme: String,

    /// Font size in pixels.
    #[arg(long, default_value = "14")]
    font_size: u32,

    /// Video frame rate.
    #[arg(long, default_value = "30")]
    fps: u32,

    /// Video width in pixels.
    #[arg(long, default_value = "1280")]
    width: u32,

    /// Video height in pixels.
    #[arg(long, default_value = "720")]
    height: u32,

    /// Glob pattern to include only matching files (e.g. "*.rs").
    #[arg(long)]
    include: Option<String>,

    /// List available syntax highlighting themes and exit.
    #[arg(long)]
    list_themes: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Handle --list-themes.
    if cli.list_themes {
        let highlighter = Highlighter::new(&cli.theme)?;
        let mut themes = highlighter.available_themes();
        themes.sort();
        println!("Available themes:");
        for t in themes {
            println!("  {t}");
        }
        return Ok(());
    }

    // Verify ffmpeg is available.
    check_ffmpeg()?;

    // Parse the diff.
    let diff_text = if let Some(diff_spec) = &cli.diff {
        let cwd = std::env::current_dir().context("Failed to get current directory")?;
        generate_git_diff(&cwd, diff_spec)?
    } else if let Some(file_path) = &cli.file {
        std::fs::read_to_string(file_path)
            .with_context(|| format!("Failed to read {}", file_path.display()))?
    } else {
        bail!("Either --diff or --file must be specified. Run with --help for usage.");
    };

    let mut parsed = parse_diff(&diff_text)?;

    if parsed.files.is_empty() {
        bail!("No file changes found in the diff.");
    }

    // Apply --include filter.
    if let Some(pattern) = &cli.include {
        parsed.files.retain(|f| {
            let name = f.display_name();
            glob_match::glob_match(pattern, name)
        });
        if parsed.files.is_empty() {
            bail!("No files matched the include pattern '{pattern}'.");
        }
    }

    // Print summary.
    println!("patchcast v{}", env!("CARGO_PKG_VERSION"));
    println!();
    println!(
        "  {} file(s), +{} -{} lines",
        parsed.files.len(),
        parsed.total_added(),
        parsed.total_removed()
    );
    for f in &parsed.files {
        println!(
            "    {} (+{} -{})",
            f.display_name(),
            f.lines_added,
            f.lines_removed
        );
    }
    println!();
    println!("  Resolution: {}x{} @ {} fps", cli.width, cli.height, cli.fps);
    println!("  Theme: {}", cli.theme);
    println!("  Output: {}", cli.output.display());
    println!();

    // Generate scenes.
    let highlighter = Highlighter::new(&cli.theme)?;
    let scene_config = SceneConfig {
        fps: cli.fps,
        ..Default::default()
    };
    let scenes = generate_scenes(&parsed.files, &highlighter, &scene_config)?;

    let frames = total_frames(&scenes);
    let duration_secs = frames as f32 / cli.fps as f32;
    println!(
        "  Rendering {} scenes, {} frames ({:.1}s)...",
        scenes.len(),
        frames,
        duration_secs
    );
    println!();

    // Render to video.
    let render_config = RenderConfig {
        width: cli.width,
        height: cli.height,
        fps: cli.fps,
        font_size: cli.font_size,
    };

    render_to_video(&scenes, &cli.output, &render_config)?;

    println!();
    println!("  Video saved to: {}", cli.output.display());
    println!();

    Ok(())
}
