//! Library media (ADR-0011): list every image/video the library holds and
//! which slides reference it; store an upload beside the slide that will
//! use it.
//!
//! Convention, not configuration: a slide's media lives in the `media/`
//! folder next to it and is referenced relatively (`media/x.png`), so the
//! reference survives copies, bundles and git moves. The library-level
//! `media/` folder holds deck-agnostic assets and is listed the same way.
//! Nothing here is stored beyond the files themselves.

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

use schemars::JsonSchema;
use serde::Serialize;

use crate::slide::{Slide, SlideCollection};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MediaKind {
    Image,
    Video,
    Other,
}

/// One file in a media folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct MediaFile {
    /// Path relative to the slide directory: `genai/media/x.png`, or
    /// `../media/logo.svg` for the library-level folder (which sits beside
    /// `slides/`). A slide in `genai/` references the latter as
    /// `../../media/logo.svg`.
    pub path: String,
    /// The slide folder this belongs to; empty for the library-level folder.
    pub folder: String,
    pub file_name: String,
    pub kind: MediaKind,
    pub bytes: u64,
    /// Slides (relative paths) whose body references this file.
    pub used_by: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, JsonSchema)]
pub struct MediaIndex {
    pub files: Vec<MediaFile>,
}

const IMG: [&str; 9] = [".png", ".jpg", ".jpeg", ".gif", ".bmp", ".tiff", ".tif", ".webp", ".svg"];
const VID: [&str; 6] = [".mp4", ".webm", ".mov", ".mkv", ".avi", ".ogv"];

pub fn media_kind(name: &str) -> MediaKind {
    let l = name.to_lowercase();
    if IMG.iter().any(|e| l.ends_with(e)) {
        MediaKind::Image
    } else if VID.iter().any(|e| l.ends_with(e)) {
        MediaKind::Video
    } else {
        MediaKind::Other
    }
}

/// Every file under `<slide_dir>/**/media/` plus `<library_media>/`, with the
/// slides that reference each. `library_media` may not exist.
pub fn list_media(slide_dir: &Path, library_media: &Path, slides: &SlideCollection) -> MediaIndex {
    // canonical absolute path → (path as listed, folder, file name)
    let mut files: BTreeMap<PathBuf, (String, String, String)> = BTreeMap::new();

    for folder in media_folders(slide_dir) {
        let rel_folder = folder
            .parent()
            .and_then(|p| p.strip_prefix(slide_dir).ok())
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();
        for entry in read_files(&folder) {
            let name = entry.file_name().unwrap_or_default().to_string_lossy().to_string();
            let rel = if rel_folder.is_empty() {
                format!("media/{name}")
            } else {
                format!("{rel_folder}/media/{name}")
            };
            files.insert(clean(&entry), (rel, rel_folder.clone(), name));
        }
    }
    if library_media.is_dir() {
        for entry in read_files(library_media) {
            let name = entry.file_name().unwrap_or_default().to_string_lossy().to_string();
            files.insert(clean(&entry), (format!("../media/{name}"), String::new(), name));
        }
    }

    // Who uses what: resolve every reference in every slide body.
    let mut used: BTreeMap<PathBuf, Vec<String>> = BTreeMap::new();
    for s in &slides.slides {
        let base = s.path.parent().unwrap_or(slide_dir);
        for r in references(&s.content) {
            let abs = clean(&base.join(&r));
            if files.contains_key(&abs) {
                let v = used.entry(abs).or_default();
                if !v.contains(&s.relative_path) {
                    v.push(s.relative_path.clone());
                }
            }
        }
    }

    let files = files
        .into_iter()
        .map(|(abs, (path, folder, file_name))| MediaFile {
            kind: media_kind(&file_name),
            bytes: std::fs::metadata(&abs).map_or(0, |m| m.len()),
            used_by: used.remove(&abs).unwrap_or_default(),
            path,
            folder,
            file_name,
        })
        .collect();
    MediaIndex { files }
}

/// Store `bytes` as `<slide folder>/media/<file_name>` for `slide` and return
/// the reference to write into its markdown (`media/<file_name>`). Refuses a
/// name that escapes the folder and never overwrites unless `overwrite`.
pub fn store_beside(
    slide: &Slide,
    file_name: &str,
    bytes: &[u8],
    overwrite: bool,
) -> std::io::Result<(PathBuf, String)> {
    let name = safe_file_name(file_name)
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid media file name"))?;
    let dir = slide
        .path
        .parent()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "slide has no parent folder"))?
        .join("media");
    std::fs::create_dir_all(&dir)?;
    let target = dir.join(&name);
    if target.exists() && !overwrite {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!("media/{name} already exists beside {}", slide.relative_path),
        ));
    }
    std::fs::write(&target, bytes)?;
    Ok((target, format!("media/{name}")))
}

/// A bare file name: no separators, no `..`, no leading dot, not empty.
pub fn safe_file_name(name: &str) -> Option<String> {
    let name = name.trim();
    if name.is_empty()
        || name.starts_with('.')
        || name.contains('/')
        || name.contains('\\')
        || name.contains("..")
        || name.chars().any(char::is_control)
    {
        return None;
    }
    Some(name.to_string())
}

/// Resolve a listed media `path` back to an absolute file, refusing anything
/// that escapes the slide directory or the library media folder.
pub fn resolve_listed(slide_dir: &Path, library_media: &Path, path: &str) -> Option<PathBuf> {
    let abs = clean(&slide_dir.join(path));
    let in_slides = abs.starts_with(clean(slide_dir))
        && abs.parent().is_some_and(|p| p.file_name().is_some_and(|n| n == "media"));
    let in_library = abs.parent().is_some_and(|p| p == clean(library_media));
    ((in_slides || in_library) && abs.is_file()).then_some(abs)
}

/// Local image/video references in a slide body: `![alt](src)`, `<img src>`,
/// `<video src>`, `<source src>`. URLs and data URIs are skipped.
pub fn references(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = body;
    while let Some(i) = rest.find("](") {
        let after = &rest[i + 2..];
        if let Some(end) = after.find(')') {
            let inner = after[..end].trim();
            let src = inner.split_whitespace().next().unwrap_or("");
            // Only markdown *images* count: look back for `![`.
            let before = &rest[..i];
            if before.rfind("![").is_some_and(|b| !before[b..].contains(']')) {
                push_local(&mut out, src);
            }
            rest = &after[end..];
        } else {
            break;
        }
    }
    for needle in ["<img", "<video", "<source"] {
        let mut r = body;
        while let Some(i) = r.find(needle) {
            let tag = &r[i..];
            if let Some(s) = tag.find("src=\"") {
                let v = &tag[s + 5..];
                if let Some(e) = v.find('"') {
                    push_local(&mut out, &v[..e]);
                }
            }
            r = &tag[needle.len()..];
        }
    }
    out
}

fn push_local(out: &mut Vec<String>, src: &str) {
    let src = src.trim();
    if src.is_empty()
        || src.starts_with("http://")
        || src.starts_with("https://")
        || src.starts_with("data:")
        || src.starts_with("//")
        || src.starts_with('/')
    {
        return;
    }
    if !out.iter().any(|s| s == src) {
        out.push(src.to_string());
    }
}

fn media_folders(slide_dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![slide_dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for e in entries.flatten() {
            let p = e.path();
            if !p.is_dir() {
                continue;
            }
            if p.file_name().is_some_and(|n| n == "media") {
                out.push(p);
            } else if !p.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.')) {
                stack.push(p);
            }
        }
    }
    out.sort();
    out
}

fn read_files(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|it| it.flatten().map(|e| e.path()).filter(|p| p.is_file()).collect())
        .unwrap_or_default();
    v.sort();
    v
}

/// Lexically normalise `..` and `.` without touching the filesystem, so a
/// reference like `../shared/media/x.png` compares equal to its target.
fn clean(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp() -> PathBuf {
        let d = std::env::temp_dir().join(format!("sldr-media-{}-{}", std::process::id(), rand_suffix()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }
    fn rand_suffix() -> u128 {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    }

    #[test]
    fn references_finds_markdown_and_html_locals_only() {
        let body = "![a](media/x.png)\n[link](not-image.md)\n<img src=\"media/y.jpg\">\n![u](https://x/y.png)\n<video src=\"../media/clip.mp4\"></video>\n![t](media/x.png \"title\")";
        assert_eq!(references(body), vec!["media/x.png", "media/y.jpg", "../media/clip.mp4"]);
    }

    #[test]
    fn list_index_and_usage_across_folders() {
        let root = tmp();
        let slides = root.join("slides");
        let lib_media = root.join("media");
        std::fs::create_dir_all(slides.join("genai/media")).unwrap();
        std::fs::create_dir_all(&lib_media).unwrap();
        std::fs::write(slides.join("genai/media/x.png"), b"1234").unwrap();
        std::fs::write(slides.join("genai/media/unused.png"), b"1").unwrap();
        std::fs::write(lib_media.join("logo.svg"), b"<svg/>").unwrap();
        std::fs::write(slides.join("genai/a.md"), "![a](media/x.png)\n![l](../../media/logo.svg)\n").unwrap();
        std::fs::write(slides.join("genai/b.md"), "![a](./media/x.png)\n").unwrap();
        let coll = SlideCollection::load_from_dir(&slides).unwrap();

        let idx = list_media(&slides, &lib_media, &coll);
        let by: BTreeMap<&str, &MediaFile> = idx.files.iter().map(|f| (f.path.as_str(), f)).collect();
        let x = by["genai/media/x.png"];
        assert_eq!(x.kind, MediaKind::Image);
        assert_eq!(x.bytes, 4);
        assert_eq!(x.used_by, vec!["genai/a.md", "genai/b.md"]);
        assert_eq!(x.folder, "genai");
        assert!(by["genai/media/unused.png"].used_by.is_empty());
        let logo = by["../media/logo.svg"];
        assert_eq!(logo.used_by, vec!["genai/a.md"]);
        assert_eq!(logo.folder, "");

        assert!(resolve_listed(&slides, &lib_media, "genai/media/x.png").is_some());
        assert!(resolve_listed(&slides, &lib_media, "../media/logo.svg").is_some());
        assert!(resolve_listed(&slides, &lib_media, "../../etc/passwd").is_none());
        assert!(resolve_listed(&slides, &lib_media, "genai/a.md").is_none(), "not in a media folder");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn store_beside_writes_next_to_slide_and_refuses_overwrite() {
        let root = tmp();
        let slides = root.join("slides");
        std::fs::create_dir_all(slides.join("t")).unwrap();
        std::fs::write(slides.join("t/s.md"), "x").unwrap();
        let coll = SlideCollection::load_from_dir(&slides).unwrap();
        let s = coll.find("t/s").unwrap();
        let (path, reference) = store_beside(s, "pic.png", b"abc", false).unwrap();
        assert_eq!(reference, "media/pic.png");
        assert_eq!(path, slides.join("t/media/pic.png"));
        assert!(store_beside(s, "pic.png", b"abc", false).is_err());
        assert!(store_beside(s, "pic.png", b"abcd", true).is_ok());
        assert!(store_beside(s, "../evil.png", b"x", false).is_err());
        assert!(store_beside(s, ".hidden", b"x", false).is_err());
        let _ = std::fs::remove_dir_all(&root);
    }
}
