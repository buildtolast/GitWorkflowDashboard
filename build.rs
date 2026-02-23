use std::process::Command;

fn main() {
    // Git commit hash (short): try env var first (Docker), then git command
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

    // Compile Slint UI files
    slint_build::compile("ui/main.slint").expect("Slint compilation failed");

    // Re-run if git HEAD changes (new commits) or UI files change
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs/heads/");
    println!("cargo:rerun-if-changed=ui/");
}
