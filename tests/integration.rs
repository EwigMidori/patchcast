use patchcast::diff_parser::parse_diff;
use patchcast::highlighter::Highlighter;
use patchcast::renderer::RenderConfig;
use patchcast::scene::{generate_scenes, total_frames, SceneConfig};

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

/// Full pipeline: parse -> highlight -> scenes -> frame count.
#[test]
fn test_full_pipeline() {
    let parsed = parse_diff(SAMPLE_DIFF).unwrap();
    assert_eq!(parsed.files.len(), 2);

    let highlighter = Highlighter::new("base16-ocean.dark").unwrap();
    let config = SceneConfig::default();
    let scenes = generate_scenes(&parsed.files, &highlighter, &config).unwrap();

    assert!(!scenes.is_empty(), "Should generate at least one scene");

    let frames = total_frames(&scenes);
    assert!(frames > 0, "Should have positive frame count");
    assert!(frames < 10000, "Frame count should be reasonable");
}

/// Verify scene count for a two-file diff.
/// Each file: title + reveal + deletion + addition + pause. No inter-file stitch.
#[test]
fn test_scene_count_two_files() {
    let parsed = parse_diff(SAMPLE_DIFF).unwrap();
    let highlighter = Highlighter::new("base16-ocean.dark").unwrap();
    let config = SceneConfig::default();
    let scenes = generate_scenes(&parsed.files, &highlighter, &config).unwrap();

    assert!(
        scenes.len() >= 8,
        "Expected at least 8 scenes for 2 files, got {}",
        scenes.len()
    );
    assert!(
        scenes.iter().all(|s| !s.is_transition()),
        "no file-to-file transition scenes"
    );
}

/// Test that render config defaults are sane.
#[test]
fn test_render_config() {
    let config = RenderConfig::default();
    assert!(config.width >= 640);
    assert!(config.height >= 480);
    assert!(config.fps >= 15);
    assert!(config.font_size >= 10);
}

/// Test highlighting works for multiple languages in a single pipeline.
#[test]
fn test_multi_language_highlighting() {
    let multi_lang_diff = r#"diff --git a/main.py b/main.py
--- a/main.py
+++ b/main.py
@@ -1,2 +1,3 @@
 import os
+import sys
 print("hello")
diff --git a/main.js b/main.js
--- a/main.js
+++ b/main.js
@@ -1,2 +1,3 @@
 const x = 1;
+const y = 2;
 console.log(x);
diff --git a/main.go b/main.go
--- a/main.go
+++ b/main.go
@@ -1,3 +1,4 @@
 package main
+import "fmt"
 func main() {
+    fmt.Println("hello")
 }
"#;

    let parsed = parse_diff(multi_lang_diff).unwrap();
    assert_eq!(parsed.files.len(), 3);
    assert_eq!(parsed.files[0].extension(), "py");
    assert_eq!(parsed.files[1].extension(), "js");
    assert_eq!(parsed.files[2].extension(), "go");

    let highlighter = Highlighter::new("base16-ocean.dark").unwrap();
    let config = SceneConfig::default();
    let scenes = generate_scenes(&parsed.files, &highlighter, &config).unwrap();
    assert!(!scenes.is_empty());
}

/// Test scene duration calculations with custom FPS.
#[test]
fn test_custom_fps_scene_durations() {
    let parsed = parse_diff(SAMPLE_DIFF).unwrap();
    let highlighter = Highlighter::new("base16-ocean.dark").unwrap();

    let config_30 = SceneConfig { fps: 30, ..Default::default() };
    let config_60 = SceneConfig { fps: 60, ..Default::default() };

    let scenes_30 = generate_scenes(&parsed.files, &highlighter, &config_30).unwrap();
    let scenes_60 = generate_scenes(&parsed.files, &highlighter, &config_60).unwrap();

    let frames_30 = total_frames(&scenes_30);
    let frames_60 = total_frames(&scenes_60);

    // 60fps should produce roughly double the frames for the same duration.
    // Not exactly 2x because code reveal frames depend on line count, not just fps.
    assert!(
        frames_60 > frames_30,
        "60fps ({}) should produce more frames than 30fps ({})",
        frames_60,
        frames_30
    );
}
