/// Syntax highlighting via `syntect`.
///
/// Takes source code and produces colored character data suitable for
/// the frame renderer.
use anyhow::{Context, Result};
use syntect::highlighting::{ThemeSet, Style as SyntectStyle};
use syntect::parsing::SyntaxSet;
use syntect::easy::HighlightLines;

use crate::style::Color;

/// A single character with its syntax-highlighted color.
#[derive(Debug, Clone)]
pub struct StyledChar {
    pub ch: char,
    pub fg: Color,
}

/// A fully highlighted line of code.
#[derive(Debug, Clone)]
pub struct HighlightedLine {
    pub chars: Vec<StyledChar>,
}

impl HighlightedLine {
    pub fn plain(text: &str, fg: Color) -> Self {
        Self {
            chars: text.chars().map(|ch| StyledChar { ch, fg }).collect(),
        }
    }
}

/// Highlighter with loaded syntax definitions and themes.
pub struct Highlighter {
    syntax_set: SyntaxSet,
    theme_set: ThemeSet,
    theme_name: String,
}

impl Highlighter {
    /// Create a new highlighter with the specified theme.
    pub fn new(theme_name: &str) -> Result<Self> {
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let theme_set = ThemeSet::load_defaults();

        // Validate the theme exists; fall back to a known bundled theme.
        let resolved_theme = if theme_set.themes.contains_key(theme_name) {
            theme_name.to_string()
        } else {
            // Try common alternatives.
            let fallbacks = [
                "base16-ocean.dark",
                "base16-eighties.dark",
                "Solarized (dark)",
                "InspiredGitHub",
            ];
            let mut found = "base16-ocean.dark".to_string();
            for fb in &fallbacks {
                if theme_set.themes.contains_key(*fb) {
                    found = fb.to_string();
                    break;
                }
            }
            found
        };

        Ok(Self {
            syntax_set,
            theme_set,
            theme_name: resolved_theme,
        })
    }

    /// List available theme names.
    pub fn available_themes(&self) -> Vec<&str> {
        self.theme_set.themes.keys().map(|s| s.as_str()).collect()
    }

    /// Highlight a slice of lines for the given file extension.
    pub fn highlight_lines(
        &self,
        lines: &[&str],
        extension: &str,
    ) -> Result<Vec<HighlightedLine>> {
        let syntax = self
            .syntax_set
            .find_syntax_by_extension(extension)
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());

        let theme = &self.theme_set.themes[&self.theme_name];
        let mut h = HighlightLines::new(syntax, theme);
        let mut result = Vec::with_capacity(lines.len());

        for line in lines {
            let ranges = h
                .highlight_line(line, &self.syntax_set)
                .context("Syntax highlighting failed")?;

            let chars = ranges
                .iter()
                .flat_map(|(style, text)| {
                    text.chars().map(move |ch| StyledChar {
                        ch,
                        fg: syntect_to_color(style),
                    })
                })
                .collect();

            result.push(HighlightedLine { chars });
        }

        Ok(result)
    }

    /// Highlight a single line.
    pub fn highlight_single(
        &self,
        text: &str,
        extension: &str,
    ) -> Result<HighlightedLine> {
        let lines = self.highlight_lines(&[text], extension)?;
        Ok(lines.into_iter().next().unwrap_or_else(|| {
            HighlightedLine::plain(text, Color::new(205, 214, 244))
        }))
    }
}

/// Convert a syntect Style to our Color.
fn syntect_to_color(style: &SyntectStyle) -> Color {
    Color::new(
        style.foreground.r,
        style.foreground.g,
        style.foreground.b,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_highlighter_creation() {
        let h = Highlighter::new("base16-ocean.dark").unwrap();
        let themes = h.available_themes();
        assert!(!themes.is_empty(), "Should have at least one theme");
    }

    #[test]
    fn test_highlighter_fallback_theme() {
        // Nonexistent theme should fall back gracefully.
        let h = Highlighter::new("nonexistent-theme-12345").unwrap();
        let themes = h.available_themes();
        assert!(!themes.is_empty());
    }

    #[test]
    fn test_highlight_rust_code() {
        let h = Highlighter::new("base16-ocean.dark").unwrap();
        let lines = h
            .highlight_lines(&["fn main() {", "    println!(\"hello\");", "}"], "rs")
            .unwrap();
        assert_eq!(lines.len(), 3);
        // Each line should have characters.
        assert!(!lines[0].chars.is_empty());
        assert!(!lines[1].chars.is_empty());
    }

    #[test]
    fn test_highlight_python_code() {
        let h = Highlighter::new("base16-ocean.dark").unwrap();
        let lines = h
            .highlight_lines(&["def hello():", "    print('hi')"], "py")
            .unwrap();
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn test_highlight_unknown_extension() {
        let h = Highlighter::new("base16-ocean.dark").unwrap();
        // Should fall back to plain text, not error.
        let lines = h
            .highlight_lines(&["some random text"], "zzzzz")
            .unwrap();
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn test_highlight_single() {
        let h = Highlighter::new("base16-ocean.dark").unwrap();
        let line = h.highlight_single("let x = 42;", "rs").unwrap();
        assert!(!line.chars.is_empty());
    }

    #[test]
    fn test_styled_char_colors_valid() {
        let h = Highlighter::new("base16-ocean.dark").unwrap();
        let lines = h.highlight_lines(&["fn foo() {}"], "rs").unwrap();
        for sc in &lines[0].chars {
            // Colors should be non-zero for syntax-highlighted code.
            // (At minimum the alpha channel is implied 255.)
            assert_eq!(sc.fg.a, 255);
        }
    }

    #[test]
    fn test_plain_line() {
        let fg = Color::new(200, 200, 200);
        let line = HighlightedLine::plain("hello", fg);
        assert_eq!(line.chars.len(), 5);
        assert_eq!(line.chars[0].ch, 'h');
        assert_eq!(line.chars[0].fg.r, 200);
    }
}
