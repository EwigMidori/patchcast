# patchcast

**I Never Record Code Reviews Anymore -- My Rust Tool Turns PRs Into Videos**

Turn any git diff or pull request into an animated, syntax-highlighted code walkthrough video. Each file change becomes a scene with smooth transitions, highlighted additions and deletions, and a professional dark-theme aesthetic.

## The Problem

Code reviews are text-heavy walls of green and red. They work fine for experienced reviewers reading inline, but they fall apart when you need to:

- Present changes async to a team
- Onboard someone to a large PR
- Record a walkthrough for documentation
- Show non-engineers what changed

patchcast turns diffs into watchable videos. No recording software, no screen sharing, no narration required.

## Quick Start

```bash
# Install
cargo install --path .

# From a git commit
patchcast --diff HEAD~1 -o review.mp4

# From a branch comparison
patchcast --diff main..feature-branch -o pr_walkthrough.mp4

# From a diff file
git diff HEAD~3 > changes.diff
patchcast --file changes.diff -o review.mp4
```

## How It Works

For each file in the diff, patchcast generates this animation sequence:

```
+------------------+     +------------------+     +------------------+
|   TITLE CARD     |     |   CODE REVEAL    |     | DELETION FLASH   |
|                  |     |                  |     |                  |
|  src/auth.rs     | --> |  15: pub secret  | --> |  -old line       |
|  Rust  +6 -2     |     |  16: pub token   |     |  -old line       |
|                  |     |  17: +refresh    |     |   (red pulse)    |
+------------------+     +------------------+     +------------------+
        |                                                  |
        v                                                  v
+------------------+     +------------------+     +------------------+
| ADDITION SLIDE   |     |     PAUSE        |     |   CROSSFADE      |
|                  |     |                  |     |                  |
|  +new line       | --> |  (hold for       | --> |  (fade to next   |
|  +new line       |     |   readability)   |     |   file...)       |
|  (green slide-in)|     |                  |     |                  |
+------------------+     +------------------+     +------------------+
```

1. **Title card** (1s) -- filename, language, change stats (+N/-N)
2. **Code reveal** -- lines appear progressively with syntax highlighting
3. **Deletion highlight** -- removed lines pulse red, then fade
4. **Addition highlight** -- new lines slide in with green highlight
5. **Pause** (0.5s) -- hold the final state for readability
6. **Transition** (0.5s) -- smooth crossfade to the next file

## Customization

```bash
# Custom theme, resolution, and frame rate
patchcast --diff HEAD~1 \
  --theme "Solarized (dark)" \
  --font-size 18 \
  --fps 30 \
  --width 1920 --height 1080 \
  -o review.mp4

# Filter to specific file types
patchcast --diff HEAD~1 --include "*.rs" -o rust_changes.mp4

# List available syntax themes
patchcast --list-themes
```

### Options

| Flag | Default | Description |
|------|---------|-------------|
| `--diff` | - | Git revision spec (e.g. `HEAD~1`, `main..feature`) |
| `--file` | - | Path to a `.diff` or `.patch` file |
| `-o, --output` | `output.mp4` | Output video path |
| `--theme` | `base16-ocean.dark` | Syntax highlighting theme |
| `--font-size` | `14` | Font size in pixels |
| `--fps` | `30` | Video frame rate |
| `--width` | `1280` | Video width |
| `--height` | `720` | Video height |
| `--include` | - | Glob pattern to filter files |
| `--list-themes` | - | Print available themes and exit |

## Supported Inputs

- **Git commits**: `--diff HEAD~1`, `--diff HEAD~5`
- **Branch ranges**: `--diff main..feature-branch`
- **Diff files**: `--file changes.diff`, `--file pr.patch`
- **Piped from git**: `git diff main | patchcast --file /dev/stdin -o out.mp4`

## Syntax Highlighting

patchcast uses [syntect](https://github.com/trishume/syntect) for syntax highlighting, which bundles grammars for 30+ languages including:

Rust, Python, JavaScript, TypeScript, Go, Java, C, C++, Ruby, Swift, Kotlin, Scala, HTML, CSS, JSON, YAML, TOML, Markdown, Shell, SQL, and more.

## Visual Style

- Dark background (#1e1e2e -- Catppuccin Mocha inspired)
- Syntax highlighting via bundled themes
- Line numbers in a dimmed gutter
- Green left-border for additions (#a6e3a1)
- Red left-border for deletions (#f38ba8)
- File path header at the top of each scene
- Monospace grid rendering for clean terminal aesthetics

## Architecture

```
src/
  main.rs           CLI entry point (clap)
  lib.rs            Public API re-exports
  diff_parser.rs    Unified diff format -> structured hunks
  highlighter.rs    syntect-based syntax highlighting
  scene.rs          Diff -> animation scenes (title, reveal, highlight, transition)
  animation.rs      Timing, easing functions, interpolation
  renderer.rs       Frame rendering (image crate) + ffmpeg encoding
  style.rs          Colors, theme, layout constants
```

**Pipeline:** `diff input -> parse -> highlight -> scene generation -> frame rendering -> ffmpeg -> MP4`

## Prerequisites

- **Rust** 1.70+ (for building)
- **ffmpeg** (for video encoding) -- `brew install ffmpeg` / `apt install ffmpeg`

## Install

```bash
git clone https://github.com/LakshmiSravyaVedantham/patchcast
cd patchcast
cargo install --path .
```

## Development

```bash
cargo build          # Build
cargo test           # Run all tests
cargo test -- -v     # Verbose test output
cargo clippy         # Lint
cargo run -- --help  # Run without installing
```

## License

MIT
