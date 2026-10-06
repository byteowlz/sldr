//! Stamp the binary with its git commit and build date, so `sldr --version`
//! can tell a stale install from a fresh build (two builds of "0.9.1" can
//! differ by weeks of fixes). Outside a checkout the stamp says so.
use std::process::Command;

fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string()).filter(|s| !s.is_empty())
}

fn main() {
    let hash = git(&["rev-parse", "--short=9", "HEAD"]).unwrap_or_else(|| "unknown".into());
    let dirty = git(&["status", "--porcelain", "--untracked-files=no"]).map(|s| !s.is_empty()).unwrap_or(false);
    let date = git(&["log", "-1", "--format=%cs"]).unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=SLDR_GIT_HASH={hash}{}", if dirty { "-dirty" } else { "" });
    println!("cargo:rustc-env=SLDR_GIT_DATE={date}");
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/refs/heads");
}
