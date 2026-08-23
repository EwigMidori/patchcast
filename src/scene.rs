/// Scene generation for the animated walkthrough.
///
/// Each file diff is converted into a sequence of scenes (title card,
/// code reveal, deletions, additions, pause, transition) that the
/// renderer draws frame by frame.
use crate::diff_parser::{DiffLineKind, FileDiff, Hunk};
use crate::highlighter::{HighlightedLine, Highlighter};
use anyhow::Result;

/// The kind of visual scene in the animation.
#[derive(Debug, Clone)]
pub enum SceneKind {
    /// Title card showing filename + change stats.
    TitleCard {
        filename: String,
        lines_added: u32,
        lines_removed: u32,
        extension: String,
    },
    /// Show code with syntax highlighting; lines appear progressively.
    CodeReveal {
        filename: String,
        lines: Vec<SceneLine>,
        /// How many lines to reveal per frame tick.
        reveal_rate: usize,
    },
    /// Highlight removed lines (flash red).
    DeletionHighlight {
        filename: String,
        lines: Vec<SceneLine>,
        /// Indices of lines that are deletions (into `lines`).
        deletion_indices: Vec<usize>,
    },
    /// Highlight added lines (slide in green).
    AdditionHighlight {
        filename: String,
        lines: Vec<SceneLine>,
        /// Indices of lines that are additions (into `lines`).
        addition_indices: Vec<usize>,
    },
    /// Pause on the final state for readability.
    Pause {
        filename: String,
        lines: Vec<SceneLine>,
    },
    /// Crossfade transition between files.
    Transition,
}

/// A single line in a scene, with its highlighting and diff metadata.
#[derive(Debug, Clone)]
pub struct SceneLine {
    pub highlighted: HighlightedLine,
    pub kind: DiffLineKind,
    pub line_number: u32,
}

/// A complete scene with its kind and duration in frames.
#[derive(Debug, Clone)]
pub struct Scene {
    pub kind: SceneKind,
    pub duration_frames: u32,
}

impl Scene {
    /// File stem for a standalone clip, e.g. `title-kernel_src_lib.rs`.
    pub fn clip_stem(&self) -> String {
        let (kind, file) = match &self.kind {
            SceneKind::TitleCard { filename, .. } => ("title", filename.as_str()),
            SceneKind::CodeReveal { filename, .. } => ("reveal", filename.as_str()),
            SceneKind::DeletionHighlight { filename, .. } => ("deletion", filename.as_str()),
            SceneKind::AdditionHighlight { filename, .. } => ("addition", filename.as_str()),
            SceneKind::Pause { filename, .. } => ("pause", filename.as_str()),
            SceneKind::Transition => return "transition".to_string(),
        };
        format!("{kind}-{}", sanitize_filename(file))
    }

    pub fn is_transition(&self) -> bool {
        matches!(self.kind, SceneKind::Transition)
    }
}

fn sanitize_filename(name: &str) -> String {
    name.replace(['/', '\\', ' '], "_")
}

/// Configuration for scene generation.
pub struct SceneConfig {
    pub fps: u32,
    pub title_duration_secs: f32,
    pub deletion_duration_secs: f32,
    pub addition_duration_secs: f32,
    pub pause_duration_secs: f32,
    pub transition_duration_secs: f32,
}

impl Default for SceneConfig {
    fn default() -> Self {
        Self {
            fps: 30,
            title_duration_secs: 1.0,
            deletion_duration_secs: 1.0,
            addition_duration_secs: 1.0,
            pause_duration_secs: 0.5,
            transition_duration_secs: 0.5,
        }
    }
}

impl SceneConfig {
    fn frames(&self, secs: f32) -> u32 {
        (secs * self.fps as f32).round() as u32
    }
}

/// Generate all scenes for a complete diff.
pub fn generate_scenes(
    files: &[FileDiff],
    highlighter: &Highlighter,
    config: &SceneConfig,
) -> Result<Vec<Scene>> {
    let mut scenes = Vec::new();

    for file in files {
        scenes.extend(generate_file_scenes(file, highlighter, config)?);
    }

    Ok(scenes)
}

/// Generate scenes for a single file diff.
fn generate_file_scenes(
    file: &FileDiff,
    highlighter: &Highlighter,
    config: &SceneConfig,
) -> Result<Vec<Scene>> {
    let mut scenes = Vec::new();
    let ext = file.extension();

    // 1. Title card.
    scenes.push(Scene {
        kind: SceneKind::TitleCard {
            filename: file.display_name().to_string(),
            lines_added: file.lines_added,
            lines_removed: file.lines_removed,
            extension: ext.to_string(),
        },
        duration_frames: config.frames(config.title_duration_secs),
    });

    // Build highlighted lines for all hunks.
    let scene_lines = highlight_hunks(&file.hunks, ext, highlighter)?;

    let deletion_indices: Vec<usize> = scene_lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.kind == DiffLineKind::Removed)
        .map(|(i, _)| i)
        .collect();

    let addition_indices: Vec<usize> = scene_lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.kind == DiffLineKind::Added)
        .map(|(i, _)| i)
        .collect();

    // 2. Code reveal (context + all lines).
    let reveal_lines_count = scene_lines.len();
    let reveal_frames = if reveal_lines_count > 0 {
        // ~2 lines per frame at 30fps, minimum 15 frames.
        let calculated = (reveal_lines_count as f32 / 2.0).ceil() as u32;
        calculated.max(15)
    } else {
        15
    };

    scenes.push(Scene {
        kind: SceneKind::CodeReveal {
            filename: file.display_name().to_string(),
            lines: scene_lines.clone(),
            reveal_rate: 2,
        },
        duration_frames: reveal_frames,
    });

    // 3. Deletion highlight.
    if !deletion_indices.is_empty() {
        scenes.push(Scene {
            kind: SceneKind::DeletionHighlight {
                filename: file.display_name().to_string(),
                lines: scene_lines.clone(),
                deletion_indices,
            },
            duration_frames: config.frames(config.deletion_duration_secs),
        });
    }

    // 4. Addition highlight.
    if !addition_indices.is_empty() {
        // Build the "after" view: context + additions only (deletions removed).
        let after_lines: Vec<SceneLine> = scene_lines
            .iter()
            .filter(|l| l.kind != DiffLineKind::Removed)
            .cloned()
            .collect();

        let after_addition_indices: Vec<usize> = after_lines
            .iter()
            .enumerate()
            .filter(|(_, l)| l.kind == DiffLineKind::Added)
            .map(|(i, _)| i)
            .collect();

        scenes.push(Scene {
            kind: SceneKind::AdditionHighlight {
                filename: file.display_name().to_string(),
                lines: after_lines.clone(),
                addition_indices: after_addition_indices,
            },
            duration_frames: config.frames(config.addition_duration_secs),
        });

        // 5. Pause on final state.
        scenes.push(Scene {
            kind: SceneKind::Pause {
                filename: file.display_name().to_string(),
                lines: after_lines,
            },
            duration_frames: config.frames(config.pause_duration_secs),
        });
    } else {
        // No additions — pause on the current state.
        scenes.push(Scene {
            kind: SceneKind::Pause {
                filename: file.display_name().to_string(),
                lines: scene_lines,
            },
            duration_frames: config.frames(config.pause_duration_secs),
        });
    }

    Ok(scenes)
}

/// Syntax-highlight all lines from all hunks in a file.
fn highlight_hunks(
    hunks: &[Hunk],
    extension: &str,
    highlighter: &Highlighter,
) -> Result<Vec<SceneLine>> {
    let mut scene_lines = Vec::new();

    for hunk in hunks {
        for diff_line in &hunk.lines {
            let highlighted = highlighter.highlight_single(&diff_line.content, extension)?;
            scene_lines.push(SceneLine {
                highlighted,
                kind: diff_line.kind.clone(),
                line_number: diff_line.line_number,
            });
        }
    }

    Ok(scene_lines)
}

/// Calculate the total number of frames for a sequence of scenes.
pub fn total_frames(scenes: &[Scene]) -> u32 {
    scenes.iter().map(|s| s.duration_frames).sum()
}

/// Get the change summary line for a title card (e.g. "+6 -2").
pub fn change_summary(added: u32, removed: u32) -> String {
    let mut parts = Vec::new();
    if added > 0 {
        parts.push(format!("+{added}"));
    }
    if removed > 0 {
        parts.push(format!("-{removed}"));
    }
    if parts.is_empty() {
        "no changes".to_string()
    } else {
        parts.join(" ")
    }
}

/// Map a file extension to a human-friendly language name.
pub fn language_name(ext: &str) -> &str {
    match ext {
        "rs" => "Rust",
        "py" => "Python",
        "js" => "JavaScript",
        "ts" => "TypeScript",
        "tsx" => "TypeScript (React)",
        "jsx" => "JavaScript (React)",
        "go" => "Go",
        "rb" => "Ruby",
        "java" => "Java",
        "c" => "C",
        "cpp" | "cc" | "cxx" => "C++",
        "h" | "hpp" => "C/C++ Header",
        "cs" => "C#",
        "swift" => "Swift",
        "kt" => "Kotlin",
        "scala" => "Scala",
        "html" => "HTML",
        "css" => "CSS",
        "scss" => "SCSS",
        "json" => "JSON",
        "yaml" | "yml" => "YAML",
        "toml" => "TOML",
        "md" => "Markdown",
        "sh" | "bash" | "zsh" => "Shell",
        "sql" => "SQL",
        "dockerfile" => "Dockerfile",
        _ => ext,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff_parser::parse_diff;

    const SAMPLE_DIFF: &str = r#"diff --git a/src/auth.rs b/src/auth.rs
index 1234567..abcdefg 100644
--- a/src/auth.rs
+++ b/src/auth.rs
@@ -15,8 +15,12 @@ pub struct AuthConfig {
     pub secret_key: String,
     pub token_expiry: u64,
+    pub refresh_enabled: bool,
+    pub max_sessions: usize,
 }

-pub fn validate_token(token: &str) -> bool {
-    token.len() > 0
+pub fn validate_token(token: &str, config: &AuthConfig) -> Result<Claims, AuthError> {
+    if token.is_empty() {
+        return Err(AuthError::EmptyToken);
+    }
+    decode_and_verify(token, &config.secret_key)
 }
"#;

    #[test]
    fn test_generate_scenes_count() {
        let parsed = parse_diff(SAMPLE_DIFF).unwrap();
        let highlighter = Highlighter::new("base16-ocean.dark").unwrap();
        let config = SceneConfig::default();
        let scenes = generate_scenes(&parsed.files, &highlighter, &config).unwrap();

        // For one file: title + code_reveal + deletion + addition + pause = 5 scenes.
        // (No transition because only one file in this subset.)
        assert!(
            scenes.len() >= 4,
            "Expected at least 4 scenes per file, got {}",
            scenes.len()
        );
    }

    #[test]
    fn test_total_frames_positive() {
        let parsed = parse_diff(SAMPLE_DIFF).unwrap();
        let highlighter = Highlighter::new("base16-ocean.dark").unwrap();
        let config = SceneConfig::default();
        let scenes = generate_scenes(&parsed.files, &highlighter, &config).unwrap();
        let frames = total_frames(&scenes);
        assert!(frames > 0, "Total frames should be positive");
    }

    #[test]
    fn test_change_summary() {
        assert_eq!(change_summary(6, 2), "+6 -2");
        assert_eq!(change_summary(0, 3), "-3");
        assert_eq!(change_summary(5, 0), "+5");
        assert_eq!(change_summary(0, 0), "no changes");
    }

    #[test]
    fn test_language_name() {
        assert_eq!(language_name("rs"), "Rust");
        assert_eq!(language_name("py"), "Python");
        assert_eq!(language_name("js"), "JavaScript");
        assert_eq!(language_name("unknown"), "unknown");
    }

    #[test]
    fn test_scene_config_frames() {
        let config = SceneConfig {
            fps: 30,
            ..Default::default()
        };
        assert_eq!(config.frames(1.0), 30);
        assert_eq!(config.frames(0.5), 15);
        assert_eq!(config.frames(2.0), 60);
    }

    #[test]
    fn test_multi_file_has_no_transitions() {
        let multi_diff = r#"diff --git a/a.rs b/a.rs
--- a/a.rs
+++ b/a.rs
@@ -1,1 +1,1 @@
-old
+new
diff --git a/b.rs b/b.rs
--- a/b.rs
+++ b/b.rs
@@ -1,1 +1,1 @@
-old2
+new2
"#;
        let parsed = parse_diff(multi_diff).unwrap();
        let highlighter = Highlighter::new("base16-ocean.dark").unwrap();
        let config = SceneConfig::default();
        let scenes = generate_scenes(&parsed.files, &highlighter, &config).unwrap();

        assert!(
            scenes.iter().all(|s| !s.is_transition()),
            "each file stays its own clip; no crossfade between files"
        );
    }
}
