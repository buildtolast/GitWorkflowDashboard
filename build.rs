use std::fs;
use std::path::Path;
use std::process::Command;

fn main() {
    // ── Git commit hash (short): try env var first (Docker), then git command ──
    let git_hash = std::env::var("GWD_GIT_HASH").ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            Command::new("git")
                .args(["rev-parse", "--short", "HEAD"])
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .unwrap_or_else(|| "unknown".to_string())
        });

    // Build timestamp (UTC) via date command
    let built_at = Command::new("date")
        .args(["-u", "+%Y-%m-%d %H:%M:%S UTC"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    println!("cargo:rustc-env=GWD_GIT_HASH={}", git_hash);
    println!("cargo:rustc-env=GWD_BUILT_AT={}", built_at);

    // ── Assemble frontend from modular source files ──
    let fe = Path::new("src/frontend");
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set");
    let out_path = Path::new(&out_dir).join("index.html");

    // Shell fragments (contain <style>, </style><script>, </script> wrapper tags)
    let shell_head = fs::read_to_string(fe.join("shell_head.html"))
        .expect("Failed to read shell_head.html");
    let shell_mid = fs::read_to_string(fe.join("shell_mid.html"))
        .expect("Failed to read shell_mid.html");
    let shell_foot = fs::read_to_string(fe.join("shell_foot.html"))
        .expect("Failed to read shell_foot.html");

    // CSS files (sorted by numeric prefix)
    let css_files = [
        "01-variables.css",
        "02-layout.css",
        "03-sidebar.css",
        "04-main-content.css",
        "05-active-runs.css",
        "06-workflows.css",
        "07-pipeline.css",
        "08-tabs-misc.css",
        "09-analytics.css",
        "10-components.css",
        "11-accessibility.css",
        "12-responsive.css",
        "13-commandbar.css",
    ];

    // JS files (sorted by numeric prefix)
    let js_files = [
        "01-state.js",
        "02-utils.js",
        "03-websocket.js",
        "04-render.js",
        "05-analytics.js",
        "06-app.js",
        "07-ui.js",
        "08-init.js",
        "09-commandbar.js",
    ];

    // Concatenate: shell_head + CSS + shell_mid + JS + shell_foot
    let mut html = String::with_capacity(400_000);
    html.push_str(&shell_head);

    for name in &css_files {
        let path = fe.join("styles").join(name);
        let content = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("Failed to read {}: {}", path.display(), e));
        html.push_str(&content);
    }

    html.push_str(&shell_mid);

    for name in &js_files {
        let path = fe.join("js").join(name);
        let content = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("Failed to read {}: {}", path.display(), e));
        html.push_str(&content);
    }

    html.push_str(&shell_foot);

    fs::write(&out_path, &html)
        .unwrap_or_else(|e| panic!("Failed to write {}: {}", out_path.display(), e));

    // ── Re-run triggers ──
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs/heads/");

    // Watch all frontend source files
    println!("cargo:rerun-if-changed=src/frontend/shell_head.html");
    println!("cargo:rerun-if-changed=src/frontend/shell_mid.html");
    println!("cargo:rerun-if-changed=src/frontend/shell_foot.html");
    for name in &css_files {
        println!("cargo:rerun-if-changed=src/frontend/styles/{}", name);
    }
    for name in &js_files {
        println!("cargo:rerun-if-changed=src/frontend/js/{}", name);
    }
}
