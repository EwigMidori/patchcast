/// Parses unified diff format into structured representations.
///
/// Handles both `git diff` output and standalone `.diff` / `.patch` files.
/// Each diff is broken into files, and each file into hunks containing
/// added, removed, and context lines.
use anyhow::{Context, Result};
use regex::Regex;
use std::path::Path;

/// A single line within a hunk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffLineKind {
    Context,
    Added,
    Removed,
}

/// A line of diff content with its kind, text, and original line number.
#[derive(Debug, Clone)]
pub struct DiffLine {
    pub kind: DiffLineKind,
    pub content: String,
    /// Line number in the old file (for Context/Removed) or new file (for Added).
    pub line_number: u32,
}

/// A contiguous hunk of changes within a file.
#[derive(Debug, Clone)]
pub struct Hunk {
    pub old_start: u32,
    pub old_count: u32,
    pub new_start: u32,
    pub new_count: u32,
    pub lines: Vec<DiffLine>,
}

/// All changes to a single file.
#[derive(Debug, Clone)]
pub struct FileDiff {
    pub old_path: String,
    pub new_path: String,
    pub hunks: Vec<Hunk>,
    pub lines_added: u32,
    pub lines_removed: u32,
}

impl FileDiff {
    /// Infer the language / file extension for syntax highlighting.
    pub fn extension(&self) -> &str {
        let path = if self.new_path != "/dev/null" {
            &self.new_path
        } else {
            &self.old_path
        };
        Path::new(path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("txt")
    }

    /// The display name for the file (prefers new path).
    pub fn display_name(&self) -> &str {
        if self.new_path != "/dev/null" {
            &self.new_path
        } else {
            &self.old_path
        }
    }
}

/// A complete parsed diff containing one or more file diffs.
#[derive(Debug, Clone)]
pub struct ParsedDiff {
    pub files: Vec<FileDiff>,
}

impl ParsedDiff {
    pub fn total_added(&self) -> u32 {
        self.files.iter().map(|f| f.lines_added).sum()
    }

    pub fn total_removed(&self) -> u32 {
        self.files.iter().map(|f| f.lines_removed).sum()
    }
}

/// Parse a unified diff string into structured data.
pub fn parse_diff(input: &str) -> Result<ParsedDiff> {
    let mut files: Vec<FileDiff> = Vec::new();
    let lines: Vec<&str> = input.lines().collect();
    let len = lines.len();
    let mut i = 0;

    let hunk_header_re =
        Regex::new(r"^@@\s+-(\d+)(?:,(\d+))?\s+\+(\d+)(?:,(\d+))?\s+@@")
            .context("Failed to compile hunk header regex")?;

    while i < len {
        // Look for diff --git line or --- / +++ pair.
        if lines[i].starts_with("diff --git ") {
            let (file_diff, next_i) =
                parse_git_diff_file(&lines, i, len, &hunk_header_re)?;
            files.push(file_diff);
            i = next_i;
        } else if lines[i].starts_with("--- ") && i + 1 < len && lines[i + 1].starts_with("+++ ") {
            // Standalone diff without "diff --git" header.
            let (file_diff, next_i) =
                parse_plain_diff_file(&lines, i, len, &hunk_header_re)?;
            files.push(file_diff);
            i = next_i;
        } else {
            i += 1;
        }
    }

    Ok(ParsedDiff { files })
}

/// Parse a file diff that starts with `diff --git a/... b/...`.
fn parse_git_diff_file(
    lines: &[&str],
    start: usize,
    len: usize,
    hunk_re: &Regex,
) -> Result<(FileDiff, usize)> {
    let diff_line = lines[start];
    let (old_path, new_path) = parse_diff_git_paths(diff_line);
    let mut i = start + 1;

    // Skip index, mode, and other header lines until we hit --- or @@ or next diff.
    while i < len
        && !lines[i].starts_with("--- ")
        && !lines[i].starts_with("@@ ")
        && !lines[i].starts_with("diff --git ")
    {
        i += 1;
    }

    // Parse --- and +++ if present.
    let mut real_old = old_path.clone();
    let mut real_new = new_path.clone();
    if i < len && lines[i].starts_with("--- ") {
        real_old = strip_ab_prefix(lines[i].trim_start_matches("--- "));
        i += 1;
    }
    if i < len && lines[i].starts_with("+++ ") {
        real_new = strip_ab_prefix(lines[i].trim_start_matches("+++ "));
        i += 1;
    }

    let (hunks, next_i) = parse_hunks(lines, i, len, hunk_re)?;

    let lines_added: u32 = hunks
        .iter()
        .flat_map(|h| &h.lines)
        .filter(|l| l.kind == DiffLineKind::Added)
        .count() as u32;
    let lines_removed: u32 = hunks
        .iter()
        .flat_map(|h| &h.lines)
        .filter(|l| l.kind == DiffLineKind::Removed)
        .count() as u32;

    Ok((
        FileDiff {
            old_path: real_old,
            new_path: real_new,
            hunks,
            lines_added,
            lines_removed,
        },
        next_i,
    ))
}

/// Parse a file diff that starts directly with `--- ` / `+++ `.
fn parse_plain_diff_file(
    lines: &[&str],
    start: usize,
    len: usize,
    hunk_re: &Regex,
) -> Result<(FileDiff, usize)> {
    let old_path = strip_ab_prefix(lines[start].trim_start_matches("--- "));
    let new_path = strip_ab_prefix(lines[start + 1].trim_start_matches("+++ "));
    let i = start + 2;

    let (hunks, next_i) = parse_hunks(lines, i, len, hunk_re)?;

    let lines_added: u32 = hunks
        .iter()
        .flat_map(|h| &h.lines)
        .filter(|l| l.kind == DiffLineKind::Added)
        .count() as u32;
    let lines_removed: u32 = hunks
        .iter()
        .flat_map(|h| &h.lines)
        .filter(|l| l.kind == DiffLineKind::Removed)
        .count() as u32;

    Ok((
        FileDiff {
            old_path,
            new_path,
            hunks,
            lines_added,
            lines_removed,
        },
        next_i,
    ))
}

/// Parse all hunks starting at position `i`.
fn parse_hunks(
    lines: &[&str],
    mut i: usize,
    len: usize,
    hunk_re: &Regex,
) -> Result<(Vec<Hunk>, usize)> {
    let mut hunks = Vec::new();

    while i < len {
        if lines[i].starts_with("diff --git ") || lines[i].starts_with("--- ") {
            break;
        }

        if let Some(caps) = hunk_re.captures(lines[i]) {
            let old_start: u32 = caps[1].parse().unwrap_or(1);
            let old_count: u32 = caps.get(2).map_or(1, |m| m.as_str().parse().unwrap_or(1));
            let new_start: u32 = caps[3].parse().unwrap_or(1);
            let new_count: u32 = caps.get(4).map_or(1, |m| m.as_str().parse().unwrap_or(1));

            i += 1;

            let mut hunk_lines = Vec::new();
            let mut old_line = old_start;
            let mut new_line = new_start;

            while i < len
                && !lines[i].starts_with("@@ ")
                && !lines[i].starts_with("diff --git ")
                && !lines[i].starts_with("--- ")
            {
                let line = lines[i];
                if let Some(content) = line.strip_prefix('+') {
                    hunk_lines.push(DiffLine {
                        kind: DiffLineKind::Added,
                        content: content.to_string(),
                        line_number: new_line,
                    });
                    new_line += 1;
                } else if let Some(content) = line.strip_prefix('-') {
                    hunk_lines.push(DiffLine {
                        kind: DiffLineKind::Removed,
                        content: content.to_string(),
                        line_number: old_line,
                    });
                    old_line += 1;
                } else if let Some(content) = line.strip_prefix(' ') {
                    hunk_lines.push(DiffLine {
                        kind: DiffLineKind::Context,
                        content: content.to_string(),
                        line_number: new_line,
                    });
                    old_line += 1;
                    new_line += 1;
                } else if line == "\\ No newline at end of file" {
                    // Skip this marker.
                    i += 1;
                    continue;
                } else {
                    // Treat bare lines as context (some diffs omit the leading space).
                    hunk_lines.push(DiffLine {
                        kind: DiffLineKind::Context,
                        content: line.to_string(),
                        line_number: new_line,
                    });
                    old_line += 1;
                    new_line += 1;
                }
                i += 1;
            }

            hunks.push(Hunk {
                old_start,
                old_count,
                new_start,
                new_count,
                lines: hunk_lines,
            });
        } else {
            i += 1;
        }
    }

    Ok((hunks, i))
}

/// Extract old and new paths from a `diff --git a/... b/...` line.
fn parse_diff_git_paths(line: &str) -> (String, String) {
    let rest = line.trim_start_matches("diff --git ");
    // Paths are `a/path b/path`. Try splitting on ` b/`.
    if let Some(idx) = rest.find(" b/") {
        let old = rest[..idx].to_string();
        let new = rest[idx + 1..].to_string();
        (strip_ab_prefix(&old), strip_ab_prefix(&new))
    } else {
        // Fallback: split on space.
        let parts: Vec<&str> = rest.splitn(2, ' ').collect();
        if parts.len() == 2 {
            (
                strip_ab_prefix(parts[0]),
                strip_ab_prefix(parts[1]),
            )
        } else {
            (rest.to_string(), rest.to_string())
        }
    }
}

/// Remove leading `a/` or `b/` prefix from diff paths.
fn strip_ab_prefix(path: &str) -> String {
    if path.starts_with("a/") || path.starts_with("b/") {
        path[2..].to_string()
    } else {
        path.to_string()
    }
}

/// Read diff from a file on disk.
pub fn parse_diff_file(path: &Path) -> Result<ParsedDiff> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read diff file: {}", path.display()))?;
    parse_diff(&content)
}

/// Generate diff from a git repository using libgit2.
pub fn generate_git_diff(repo_path: &Path, diff_spec: &str) -> Result<String> {
    let repo = git2::Repository::discover(repo_path)
        .with_context(|| format!("Failed to open git repository at {}", repo_path.display()))?;

    let diff_text = if diff_spec.contains("..") {
        // Range diff: e.g. "main..feature"
        let parts: Vec<&str> = diff_spec.splitn(2, "..").collect();
        let old_obj = repo
            .revparse_single(parts[0])
            .with_context(|| format!("Failed to parse revision: {}", parts[0]))?;
        let new_obj = repo
            .revparse_single(parts[1])
            .with_context(|| format!("Failed to parse revision: {}", parts[1]))?;

        let old_tree = old_obj
            .peel_to_tree()
            .context("Failed to peel old revision to tree")?;
        let new_tree = new_obj
            .peel_to_tree()
            .context("Failed to peel new revision to tree")?;

        let diff = repo
            .diff_tree_to_tree(Some(&old_tree), Some(&new_tree), None)
            .context("Failed to generate diff between trees")?;

        let mut output = String::new();
        diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
            let origin = line.origin();
            match origin {
                '+' | '-' | ' ' => {
                    output.push(origin);
                }
                _ => {}
            }
            if let Ok(s) = std::str::from_utf8(line.content()) {
                output.push_str(s);
            }
            true
        })
        .context("Failed to format diff")?;

        output
    } else {
        // Single revision diff: e.g. "HEAD~1"
        let obj = repo
            .revparse_single(diff_spec)
            .with_context(|| format!("Failed to parse revision: {}", diff_spec))?;
        let commit = obj
            .peel_to_commit()
            .context("Failed to peel to commit")?;

        let tree = commit.tree().context("Failed to get commit tree")?;
        let parent_tree = if commit.parent_count() > 0 {
            Some(
                commit
                    .parent(0)
                    .context("Failed to get parent commit")?
                    .tree()
                    .context("Failed to get parent tree")?,
            )
        } else {
            None
        };

        let diff = repo
            .diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None)
            .context("Failed to generate diff")?;

        let mut output = String::new();
        diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
            let origin = line.origin();
            match origin {
                '+' | '-' | ' ' => {
                    output.push(origin);
                }
                _ => {}
            }
            if let Ok(s) = std::str::from_utf8(line.content()) {
                output.push_str(s);
            }
            true
        })
        .context("Failed to format diff")?;

        output
    };

    Ok(diff_text)
}

#[cfg(test)]
mod tests {
    use super::*;

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
diff --git a/src/api/routes.rs b/src/api/routes.rs
index 2345678..bcdefgh 100644
--- a/src/api/routes.rs
+++ b/src/api/routes.rs
@@ -1,5 +1,6 @@
 use crate::auth;
+use crate::middleware::RateLimit;

-pub fn setup_routes(app: &mut App) {
+pub fn setup_routes(app: &mut App, rate_limit: RateLimit) {
     app.get("/health", health_check);
-    app.post("/login", auth::login);
+    app.post("/login", rate_limit.wrap(auth::login));
+    app.post("/refresh", rate_limit.wrap(auth::refresh_token));
 }
"#;

    #[test]
    fn test_parse_sample_diff() {
        let parsed = parse_diff(SAMPLE_DIFF).unwrap();
        assert_eq!(parsed.files.len(), 2, "Should parse two files");

        let auth = &parsed.files[0];
        assert_eq!(auth.new_path, "src/auth.rs");
        assert_eq!(auth.hunks.len(), 1);
        assert_eq!(auth.lines_added, 7);
        assert_eq!(auth.lines_removed, 2);

        let routes = &parsed.files[1];
        assert_eq!(routes.new_path, "src/api/routes.rs");
        assert_eq!(routes.hunks.len(), 1);
        assert_eq!(routes.lines_added, 4);
        assert_eq!(routes.lines_removed, 2);
    }

    #[test]
    fn test_total_counts() {
        let parsed = parse_diff(SAMPLE_DIFF).unwrap();
        assert_eq!(parsed.total_added(), 11);
        assert_eq!(parsed.total_removed(), 4);
    }

    #[test]
    fn test_file_extension() {
        let parsed = parse_diff(SAMPLE_DIFF).unwrap();
        assert_eq!(parsed.files[0].extension(), "rs");
        assert_eq!(parsed.files[1].extension(), "rs");
    }

    #[test]
    fn test_hunk_line_numbers() {
        let parsed = parse_diff(SAMPLE_DIFF).unwrap();
        let hunk = &parsed.files[0].hunks[0];
        assert_eq!(hunk.old_start, 15);
        assert_eq!(hunk.new_start, 15);

        // First line is context.
        assert_eq!(hunk.lines[0].kind, DiffLineKind::Context);
        assert_eq!(hunk.lines[0].line_number, 15);
    }

    #[test]
    fn test_display_name() {
        let parsed = parse_diff(SAMPLE_DIFF).unwrap();
        assert_eq!(parsed.files[0].display_name(), "src/auth.rs");
    }

    #[test]
    fn test_empty_diff() {
        let parsed = parse_diff("").unwrap();
        assert_eq!(parsed.files.len(), 0);
    }

    #[test]
    fn test_strip_ab_prefix() {
        assert_eq!(strip_ab_prefix("a/src/main.rs"), "src/main.rs");
        assert_eq!(strip_ab_prefix("b/src/main.rs"), "src/main.rs");
        assert_eq!(strip_ab_prefix("src/main.rs"), "src/main.rs");
    }

    #[test]
    fn test_plain_diff_without_git_header() {
        let plain = "--- a/old.py\n+++ b/new.py\n@@ -1,3 +1,3 @@\n context\n-old line\n+new line\n context end\n";
        let parsed = parse_diff(plain).unwrap();
        assert_eq!(parsed.files.len(), 1);
        assert_eq!(parsed.files[0].new_path, "new.py");
        assert_eq!(parsed.files[0].lines_added, 1);
        assert_eq!(parsed.files[0].lines_removed, 1);
    }
}
