//! Compile-time provenance only; informational invocations never run git.
use std::process::Command;

fn main() {
    let revision = Command::new("git")
        .args(["describe", "--always", "--dirty", "--abbrev=9"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|revision| revision.trim().to_string())
        .filter(|revision| !revision.is_empty())
        .unwrap_or_else(|| "unknown (source archive or git unavailable)".into());
    println!("cargo:rustc-env=SLDR_GIT_HASH={revision}");
    println!(
        "cargo:rustc-env=SLDR_BUILD_TARGET={}",
        std::env::var("TARGET").unwrap_or_default()
    );
    // Watch server sources as well as commit/index changes. No runtime paths or
    // private user state are consulted, and no wall-clock timestamp is embedded.
    println!("cargo:rerun-if-changed=.");
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/index");
    println!("cargo:rerun-if-changed=../../.git/refs");
    println!("cargo:rerun-if-changed=../../.git/packed-refs");
}
