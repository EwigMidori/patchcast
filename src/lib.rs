//! patchcast — Turn any git diff into an animated code walkthrough video.
//!
//! This crate provides the core functionality for parsing diffs,
//! syntax highlighting code, generating animation scenes, and
//! rendering frames to video via ffmpeg.

pub mod animation;
pub mod diff_parser;
pub mod highlighter;
pub mod renderer;
pub mod scene;
pub mod style;

/// Re-export the main types for convenience.
pub use diff_parser::{parse_diff, parse_diff_file, generate_git_diff, ParsedDiff, FileDiff};
pub use highlighter::Highlighter;
pub use renderer::{render_to_video, check_ffmpeg, RenderConfig};
pub use scene::{generate_scenes, total_frames, Scene, SceneConfig};
