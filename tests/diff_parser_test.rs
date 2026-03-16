use patchcast::diff_parser::{parse_diff, DiffLineKind};

const MULTI_FILE_DIFF: &str = r#"diff --git a/src/auth.rs b/src/auth.rs
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
fn test_multi_file_parsing() {
    let parsed = parse_diff(MULTI_FILE_DIFF).unwrap();
    assert_eq!(parsed.files.len(), 2);
}

#[test]
fn test_first_file_details() {
    let parsed = parse_diff(MULTI_FILE_DIFF).unwrap();
    let auth = &parsed.files[0];
    assert_eq!(auth.old_path, "src/auth.rs");
    assert_eq!(auth.new_path, "src/auth.rs");
    assert_eq!(auth.lines_added, 7);
    assert_eq!(auth.lines_removed, 2);
    assert_eq!(auth.extension(), "rs");
}

#[test]
fn test_second_file_details() {
    let parsed = parse_diff(MULTI_FILE_DIFF).unwrap();
    let routes = &parsed.files[1];
    assert_eq!(routes.new_path, "src/api/routes.rs");
    assert_eq!(routes.lines_added, 4);
    assert_eq!(routes.lines_removed, 2);
}

#[test]
fn test_hunk_structure() {
    let parsed = parse_diff(MULTI_FILE_DIFF).unwrap();
    let auth = &parsed.files[0];
    assert_eq!(auth.hunks.len(), 1);

    let hunk = &auth.hunks[0];
    assert_eq!(hunk.old_start, 15);
    assert_eq!(hunk.old_count, 8);
    assert_eq!(hunk.new_start, 15);
    assert_eq!(hunk.new_count, 12);
}

#[test]
fn test_line_kinds() {
    let parsed = parse_diff(MULTI_FILE_DIFF).unwrap();
    let hunk = &parsed.files[0].hunks[0];

    let context_count = hunk.lines.iter().filter(|l| l.kind == DiffLineKind::Context).count();
    let added_count = hunk.lines.iter().filter(|l| l.kind == DiffLineKind::Added).count();
    let removed_count = hunk.lines.iter().filter(|l| l.kind == DiffLineKind::Removed).count();

    assert_eq!(added_count, 7);
    assert_eq!(removed_count, 2);
    assert!(context_count >= 3, "Should have context lines");
}

#[test]
fn test_total_counts() {
    let parsed = parse_diff(MULTI_FILE_DIFF).unwrap();
    assert_eq!(parsed.total_added(), 11);
    assert_eq!(parsed.total_removed(), 4);
}

#[test]
fn test_empty_diff() {
    let parsed = parse_diff("").unwrap();
    assert!(parsed.files.is_empty());
}

#[test]
fn test_single_addition() {
    let diff = r#"diff --git a/new.txt b/new.txt
--- /dev/null
+++ b/new.txt
@@ -0,0 +1,3 @@
+line one
+line two
+line three
"#;
    let parsed = parse_diff(diff).unwrap();
    assert_eq!(parsed.files.len(), 1);
    assert_eq!(parsed.files[0].lines_added, 3);
    assert_eq!(parsed.files[0].lines_removed, 0);
}

#[test]
fn test_single_deletion() {
    let diff = r#"diff --git a/old.txt b/old.txt
--- a/old.txt
+++ /dev/null
@@ -1,2 +0,0 @@
-removed line one
-removed line two
"#;
    let parsed = parse_diff(diff).unwrap();
    assert_eq!(parsed.files.len(), 1);
    assert_eq!(parsed.files[0].lines_added, 0);
    assert_eq!(parsed.files[0].lines_removed, 2);
}

#[test]
fn test_python_diff() {
    let diff = r#"diff --git a/app.py b/app.py
--- a/app.py
+++ b/app.py
@@ -1,4 +1,5 @@
 import os
+import sys

 def main():
-    print("hello")
+    print("hello world")
"#;
    let parsed = parse_diff(diff).unwrap();
    assert_eq!(parsed.files.len(), 1);
    assert_eq!(parsed.files[0].extension(), "py");
    assert_eq!(parsed.files[0].lines_added, 2);
    assert_eq!(parsed.files[0].lines_removed, 1);
}
