//! Font embedding — make decks truly self-contained (ADR-0006 podium test).
//!
//! A flavor's `font_imports` are Google-Fonts `css2` stylesheet URLs. Left
//! as `<link>` tags they are a network dependency: the deck won't render
//! correctly offline, and online they load asynchronously, flashing
//! fallback text that reflows when the web font arrives. Instead, at build
//! time we fetch the stylesheet and its woff2 files and inline them as
//! `@font-face` rules with base64 data URIs.
//!
//! Build-time network is fine (ADR-0006 only requires the *presentation* be
//! offline). Results are cached under the user cache dir, so subsequent
//! builds — including fully offline ones — embed from cache. If a font
//! cannot be fetched and isn't cached, the caller falls back to a `<link>`
//! so the deck still works online, exactly as before.

use std::hash::{Hash, Hasher};
use std::io::Read;
use std::path::PathBuf;
use std::time::Duration;

use base64::Engine;

// A modern desktop UA so Google Fonts returns woff2 `src` URLs.
const UA: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 \
                  (KHTML, like Gecko) Chrome/124.0 Safari/537.36";

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(6))
        .timeout(Duration::from_secs(20))
        .build()
}

fn cache_dir() -> Option<PathBuf> {
    let d = dirs::cache_dir()?.join("sldr").join("fonts");
    std::fs::create_dir_all(&d).ok()?;
    Some(d)
}

fn cache_name(url: &str, ext: &str) -> String {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    url.hash(&mut h);
    format!("{:016x}.{ext}", h.finish())
}

fn fetch_text(ag: &ureq::Agent, url: &str) -> Option<String> {
    let dir = cache_dir()?;
    let path = dir.join(cache_name(url, "css"));
    if let Ok(s) = std::fs::read_to_string(&path) {
        return Some(s);
    }
    let body = ag.get(url).set("User-Agent", UA).call().ok()?.into_string().ok()?;
    let _ = std::fs::write(&path, &body);
    Some(body)
}

fn fetch_bytes(ag: &ureq::Agent, url: &str) -> Option<Vec<u8>> {
    let dir = cache_dir()?;
    let path = dir.join(cache_name(url, "bin"));
    if let Ok(b) = std::fs::read(&path) {
        return Some(b);
    }
    let mut buf = Vec::new();
    ag.get(url)
        .set("User-Agent", UA)
        .call()
        .ok()?
        .into_reader()
        .read_to_end(&mut buf)
        .ok()?;
    let _ = std::fs::write(&path, &buf);
    Some(buf)
}

/// Embed a *local* font stylesheet a flavor ships in its own directory
/// (`font_imports = ["assets/fonts.css"]`). The CSS is read from disk and
/// any `url(...)` pointing to a local font file is inlined as base64 — so
/// the flavor carries its own fonts with zero network dependency, at build
/// or presentation time. A stylesheet that already uses `data:` URIs is
/// returned as-is. Returns None if the file (or a referenced font) is
/// missing, so the caller can warn and move on.
pub fn embed_local_font_css(flavor_dir: &std::path::Path, rel: &str) -> Option<String> {
    let css_path = flavor_dir.join(rel);
    let css = std::fs::read_to_string(&css_path).ok()?;
    let base = css_path.parent()?.to_path_buf();

    let mut out = String::with_capacity(css.len() * 2);
    let mut rest = css.as_str();
    while let Some(i) = rest.find("url(") {
        out.push_str(&rest[..i + 4]);
        let after = &rest[i + 4..];
        let Some(end) = after.find(')') else {
            out.push_str(after);
            rest = "";
            break;
        };
        let raw = after[..end].trim_matches(|c| c == '\'' || c == '"' || c == ' ');
        if raw.starts_with("data:") || raw.starts_with("http") {
            out.push_str(raw);
        } else {
            let font_path = base.join(raw);
            let bytes = std::fs::read(&font_path).ok()?;
            let mime = if raw.ends_with("woff2") {
                "font/woff2"
            } else if raw.ends_with("woff") {
                "font/woff"
            } else {
                "font/ttf"
            };
            out.push_str("data:");
            out.push_str(mime);
            out.push_str(";base64,");
            out.push_str(&base64::engine::general_purpose::STANDARD.encode(&bytes));
        }
        rest = &after[end..];
    }
    out.push_str(rest);
    Some(out)
}

/// Fetch a Google-Fonts stylesheet and inline its woff2/woff/ttf `src`
/// urls as base64 data URIs. Returns the rewritten CSS, or None when the
/// stylesheet (or any of its fonts) can't be obtained — the caller then
/// falls back to a plain `<link>`.
pub fn embed_font_css(url: &str) -> Option<String> {
    let ag = agent();
    let css = fetch_text(&ag, url)?;

    let mut out = String::with_capacity(css.len() * 2);
    let mut rest = css.as_str();
    let mut embedded_any = false;
    while let Some(i) = rest.find("url(") {
        out.push_str(&rest[..i + 4]);
        let after = &rest[i + 4..];
        let Some(end) = after.find(')') else {
            out.push_str(after);
            rest = "";
            break;
        };
        let raw = after[..end].trim_matches(|c| c == '\'' || c == '"' || c == ' ');
        if raw.starts_with("http") {
            match fetch_bytes(&ag, raw) {
                Some(bytes) => {
                    let mime = if raw.ends_with("woff2") {
                        "font/woff2"
                    } else if raw.ends_with("woff") {
                        "font/woff"
                    } else {
                        "font/ttf"
                    };
                    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
                    out.push_str("data:");
                    out.push_str(mime);
                    out.push_str(";base64,");
                    out.push_str(&b64);
                    embedded_any = true;
                }
                // A font we couldn't fetch: bail so the caller keeps the
                // <link> rather than ship a half-embedded stylesheet.
                None => return None,
            }
        } else {
            out.push_str(raw);
        }
        rest = &after[end..];
    }
    out.push_str(rest);
    embedded_any.then_some(out)
}
