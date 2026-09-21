//! Media embedding - convert local images to WebP and optionally base64-encode
//! for self-contained HTML output, or copy to an assets directory alongside
//! the output. Videos are always kept as external references.

use std::io::Cursor;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use base64::Engine;
use image::ImageFormat;
use tracing::{info, warn};

/// Maximum image dimension (width or height) before we downscale.
/// Presentations rarely need images larger than 1920px.
const MAX_IMAGE_DIMENSION: u32 = 1920;

/// How images should be handled in the output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ImageMode {
    /// Embed images as base64 data URIs (self-contained HTML, larger file)
    #[default]
    Embed,
    /// Copy images as WebP to an assets directory alongside the HTML (smaller HTML, external files)
    External,
}

/// Result of processing a media reference found in slide markdown.
pub enum MediaEmbed {
    /// An inlined base64 data URI (for images)
    DataUri(String),
    /// An external reference that should be kept as-is (URLs, videos, external images)
    External(String),
    /// A local image that was converted to WebP and copied to an assets dir
    AssetFile {
        /// Relative path to use in the HTML src attribute
        html_src: String,
        /// Absolute path where the file was written
        dest_path: PathBuf,
    },
    /// The file was not found or could not be processed
    NotFound(String),
}

/// Check if a path looks like a URL (not a local file)
fn is_url(path: &str) -> bool {
    path.starts_with("http://")
        || path.starts_with("https://")
        || path.starts_with("data:")
        || path.starts_with("//")
}

/// Check if a path points to a video file
pub fn is_video(path: &str) -> bool {
    let lower = path.to_lowercase();
    lower.ends_with(".mp4")
        || lower.ends_with(".webm")
        || lower.ends_with(".mov")
        || lower.ends_with(".mkv")
        || lower.ends_with(".avi")
        || lower.ends_with(".ogv")
}

/// A local (non-URL) image path the pipeline can embed or copy.
pub fn is_local_image(path: &str) -> bool {
    !is_url(path) && is_image(path)
}

/// Check if a path points to an image file we can process
fn is_image(path: &str) -> bool {
    let lower = path.to_lowercase();
    lower.ends_with(".png")
        || lower.ends_with(".jpg")
        || lower.ends_with(".jpeg")
        || lower.ends_with(".gif")
        || lower.ends_with(".bmp")
        || lower.ends_with(".tiff")
        || lower.ends_with(".tif")
        || lower.ends_with(".webp")
        || lower.ends_with(".svg")
}

/// Process a media source path, returning a data URI, asset file, or external reference.
///
/// - URLs are always passed through as-is
/// - Videos are always passed through as external references
/// - Local images are converted to WebP and either:
///   - `Embed` mode: base64-encoded as data URIs (self-contained)
///   - `External` mode: copied to `assets_dir` as WebP files
/// - SVGs are base64-encoded (tiny) or copied depending on mode
pub fn process_media_src(
    src: &str,
    slide_dir: Option<&Path>,
    mode: ImageMode,
    assets_dir: Option<&Path>,
) -> MediaEmbed {
    // URLs pass through regardless of mode
    if is_url(src) {
        return MediaEmbed::External(src.to_string());
    }

    // Not an image or video we can handle
    if !is_image(src) && !is_video(src) {
        return MediaEmbed::External(src.to_string());
    }

    // Resolve the local path
    let resolved = resolve_path(src, slide_dir);

    if !resolved.exists() {
        warn!("Media not found: {}", resolved.display());
        return MediaEmbed::NotFound(src.to_string());
    }

    // Videos: never transcoded. Directory output copies the file next to
    // the HTML (browser-native streaming/seeking over file://, ADR-0006);
    // single-file output inlines it as a data URI like everything else —
    // the build warns when that pushes the file past the playback ceiling.
    if is_video(src) {
        return match (mode, assets_dir) {
            (ImageMode::External, Some(assets)) => match copy_media_file(&resolved, assets) {
                Ok((html_src, dest_path)) => {
                    info!("Copied video: {} -> {}", src, dest_path.display());
                    MediaEmbed::AssetFile {
                        html_src,
                        dest_path,
                    }
                }
                Err(e) => {
                    warn!("Failed to copy video {}: {}", resolved.display(), e);
                    MediaEmbed::NotFound(src.to_string())
                }
            },
            _ => match std::fs::read(&resolved) {
                Ok(bytes) => {
                    let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
                    info!("Embedded video: {} ({} bytes)", src, bytes.len());
                    MediaEmbed::DataUri(format!("data:{};base64,{encoded}", video_mime(src)))
                }
                Err(e) => {
                    warn!("Failed to read video {}: {}", resolved.display(), e);
                    MediaEmbed::NotFound(src.to_string())
                }
            },
        };
    }

    // SVGs - always embed (they're tiny as text)
    if src.to_lowercase().ends_with(".svg") {
        return match std::fs::read_to_string(&resolved) {
            Ok(svg_content) => {
                let encoded =
                    base64::engine::general_purpose::STANDARD.encode(svg_content.as_bytes());
                let data_uri = format!("data:image/svg+xml;base64,{encoded}");
                info!("Embedded SVG: {} ({} bytes)", src, encoded.len());
                MediaEmbed::DataUri(data_uri)
            }
            Err(e) => {
                warn!("Failed to read SVG {}: {}", resolved.display(), e);
                MediaEmbed::NotFound(src.to_string())
            }
        };
    }

    // Raster images: convert to WebP, then embed or copy
    match mode {
        ImageMode::Embed => match embed_image_as_webp(&resolved) {
            Ok(data_uri) => {
                info!("Embedded image as WebP: {}", src);
                MediaEmbed::DataUri(data_uri)
            }
            Err(e) => {
                warn!("Failed to embed image {}: {}", resolved.display(), e);
                MediaEmbed::NotFound(src.to_string())
            }
        },
        ImageMode::External => {
            let Some(assets) = assets_dir else {
                warn!("External image mode but no assets_dir set, falling back to embed");
                return match embed_image_as_webp(&resolved) {
                    Ok(data_uri) => MediaEmbed::DataUri(data_uri),
                    Err(e) => {
                        warn!("Failed to embed image {}: {}", resolved.display(), e);
                        MediaEmbed::NotFound(src.to_string())
                    }
                };
            };

            match copy_image_as_webp(&resolved, assets) {
                Ok((html_src, dest_path)) => {
                    info!("Copied image as WebP: {} -> {}", src, dest_path.display());
                    MediaEmbed::AssetFile {
                        html_src,
                        dest_path,
                    }
                }
                Err(e) => {
                    warn!("Failed to copy image {}: {}", resolved.display(), e);
                    MediaEmbed::NotFound(src.to_string())
                }
            }
        }
    }
}

/// Pixel dimensions of a local image as emitted (after the max-dimension
/// downscale), so `<img width height>` matches the file and the browser knows
/// the aspect ratio before the bytes arrive. `None` for URLs, videos, and
/// anything unreadable — the tag is simply emitted without the attributes.
pub fn image_dimensions(src: &str, slide_dir: Option<&Path>) -> Option<(u32, u32)> {
    if is_url(src) || !is_image(src) {
        return None;
    }
    let resolved = resolve_path(src, slide_dir);
    if src.to_lowercase().ends_with(".svg") {
        return svg_dimensions(&std::fs::read_to_string(&resolved).ok()?);
    }
    let (w, h) = image::image_dimensions(&resolved).ok()?;
    if w == 0 || h == 0 {
        return None;
    }
    // Mirror the downscale in `load_and_convert_to_webp` (fit inside the box).
    if w > MAX_IMAGE_DIMENSION || h > MAX_IMAGE_DIMENSION {
        let ratio = (MAX_IMAGE_DIMENSION as f64 / w as f64).min(MAX_IMAGE_DIMENSION as f64 / h as f64);
        return Some((
            ((w as f64 * ratio).round() as u32).max(1),
            ((h as f64 * ratio).round() as u32).max(1),
        ));
    }
    Some((w, h))
}

/// Intrinsic size of an SVG from its `viewBox` (preferred) or `width`/`height`
/// attributes on the root element. Only the aspect ratio matters downstream.
fn svg_dimensions(svg: &str) -> Option<(u32, u32)> {
    let open = svg.find("<svg")?;
    let end = svg[open..].find('>')? + open;
    let root = &svg[open..end];
    let attr = |name: &str| -> Option<String> {
        let needle = format!(" {name}=");
        let start = root.find(&needle)? + needle.len();
        let quote = root[start..].chars().next()?;
        if quote != '"' && quote != '\'' {
            return None;
        }
        let inner = &root[start + 1..];
        let close = inner.find(quote)?;
        Some(inner[..close].to_string())
    };
    let to_px = |v: &str| -> Option<f64> {
        let v = v.trim();
        if v.ends_with('%') {
            return None;
        }
        v.trim_end_matches(|c: char| c.is_ascii_alphabetic()).trim().parse::<f64>().ok()
    };
    let round = |w: f64, h: f64| -> Option<(u32, u32)> {
        if w > 0.0 && h > 0.0 {
            Some(((w.round() as u32).max(1), (h.round() as u32).max(1)))
        } else {
            None
        }
    };
    if let Some(vb) = attr("viewBox") {
        let parts: Vec<f64> = vb
            .split(|c: char| c == ',' || c.is_whitespace())
            .filter(|s| !s.is_empty())
            .filter_map(|s| s.parse().ok())
            .collect();
        if parts.len() == 4 {
            if let Some(d) = round(parts[2], parts[3]) {
                return Some(d);
            }
        }
    }
    round(to_px(&attr("width")?)?, to_px(&attr("height")?)?)
}

/// Resolve a relative path against the slide directory
fn resolve_path(src: &str, slide_dir: Option<&Path>) -> PathBuf {
    let path = Path::new(src);

    // Already absolute
    if path.is_absolute() {
        return path.to_path_buf();
    }

    // Expand ~ home directory
    if src.starts_with('~') {
        if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
            return home.join(&src[2..]);
        }
    }

    // Resolve relative to slide directory
    if let Some(dir) = slide_dir {
        let resolved = dir.join(path);
        if resolved.exists() {
            return resolved;
        }
    }

    // Fall back to current directory
    path.to_path_buf()
}

/// Load an image, downscale if necessary, and convert to WebP bytes.
fn load_and_convert_to_webp(path: &Path) -> Result<(Vec<u8>, u64)> {
    let original_size = std::fs::metadata(path)?.len();

    // If the source is already a small WebP, just read it directly
    let is_already_webp = path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("webp"));

    let img = image::open(path).context("Failed to open image")?;

    if is_already_webp && img.width() <= MAX_IMAGE_DIMENSION && img.height() <= MAX_IMAGE_DIMENSION
    {
        let bytes = std::fs::read(path)?;
        return Ok((bytes, original_size));
    }

    // Downscale if either dimension exceeds the limit
    let img = if img.width() > MAX_IMAGE_DIMENSION || img.height() > MAX_IMAGE_DIMENSION {
        info!(
            "Downscaling {}x{} -> max {}px",
            img.width(),
            img.height(),
            MAX_IMAGE_DIMENSION
        );
        img.resize(
            MAX_IMAGE_DIMENSION,
            MAX_IMAGE_DIMENSION,
            image::imageops::FilterType::Lanczos3,
        )
    } else {
        img
    };

    // Encode to WebP
    let mut webp_buf = Cursor::new(Vec::new());
    img.write_to(&mut webp_buf, ImageFormat::WebP)
        .context("Failed to encode as WebP")?;

    let webp_bytes = webp_buf.into_inner();

    info!(
        "Image: {} -> WebP: {} bytes (was {} bytes, {:.0}% reduction)",
        path.display(),
        webp_bytes.len(),
        original_size,
        if original_size > 0 {
            (1.0 - webp_bytes.len() as f64 / original_size as f64) * 100.0
        } else {
            0.0
        }
    );

    Ok((webp_bytes, original_size))
}

/// Load an image, convert to WebP, and return a base64 data URI.
fn embed_image_as_webp(path: &Path) -> Result<String> {
    let (webp_bytes, _) = load_and_convert_to_webp(path)?;
    let encoded = base64::engine::general_purpose::STANDARD.encode(&webp_bytes);
    Ok(format!("data:image/webp;base64,{encoded}"))
}

/// Load an image, convert to WebP, and copy it to the assets directory.
/// Returns (relative html src path, absolute destination path).
fn copy_image_as_webp(path: &Path, assets_dir: &Path) -> Result<(String, PathBuf)> {
    let (webp_bytes, _) = load_and_convert_to_webp(path)?;

    // Create assets dir if needed
    std::fs::create_dir_all(assets_dir)?;

    // Derive output filename: original stem + .webp
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("image");
    let filename = format!("{stem}.webp");
    let dest_path = assets_dir.join(&filename);

    std::fs::write(&dest_path, &webp_bytes)?;

    // HTML src is relative: assets/<filename>
    let html_src = format!("assets/{filename}");

    Ok((html_src, dest_path))
}

/// Copy a media file verbatim into the assets directory.
/// Returns (relative html src path, absolute destination path).
fn copy_media_file(path: &Path, assets_dir: &Path) -> Result<(String, PathBuf)> {
    std::fs::create_dir_all(assets_dir)?;
    let filename = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("media")
        .to_string();
    let dest_path = assets_dir.join(&filename);
    // Skip the copy when the destination is already this exact file (rebuilds
    // of large videos would otherwise dominate build time).
    let same = std::fs::metadata(&dest_path)
        .ok()
        .zip(std::fs::metadata(path).ok())
        .is_some_and(|(d, s)| d.len() == s.len() && d.modified().ok() >= s.modified().ok());
    if !same {
        std::fs::copy(path, &dest_path).context("Failed to copy media file")?;
    }
    Ok((format!("assets/{filename}"), dest_path))
}

/// MIME type for a video source, by extension (data-URI srcs carry it
/// inline, so only the original path is consulted).
pub fn video_mime(src: &str) -> &'static str {
    let lower = src.to_lowercase();
    if lower.ends_with(".webm") {
        "video/webm"
    } else if lower.ends_with(".mov") {
        "video/quicktime"
    } else if lower.ends_with(".ogv") {
        "video/ogg"
    } else {
        "video/mp4"
    }
}

/// Generate a `<video>` tag for a video source. `mime` is derived from the
/// original (pre-pipeline) path via [`video_mime`].
pub fn video_tag(src: &str, mime: &str, attrs: &str) -> String {
    format!(
        r#"<video {attrs} playsinline>
  <source src="{src}" type="{mime}">
  Your browser does not support the video tag.
</video>"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_url() {
        assert!(is_url("https://example.com/img.png"));
        assert!(is_url("http://example.com/img.png"));
        assert!(is_url("data:image/png;base64,abc"));
        assert!(!is_url("./local/image.png"));
        assert!(!is_url("/absolute/path.jpg"));
    }

    #[test]
    fn test_is_video() {
        assert!(is_video("clip.mp4"));
        assert!(is_video("recording.webm"));
        assert!(is_video("movie.MOV"));
        assert!(!is_video("photo.png"));
        assert!(!is_video("slide.md"));
    }

    #[test]
    fn test_is_image() {
        assert!(is_image("photo.png"));
        assert!(is_image("pic.JPG"));
        assert!(is_image("icon.svg"));
        assert!(is_image("shot.webp"));
        assert!(!is_image("clip.mp4"));
        assert!(!is_image("slide.md"));
    }

    #[test]
    fn test_url_passthrough() {
        let result = process_media_src("https://example.com/img.png", None, ImageMode::Embed, None);
        assert!(matches!(result, MediaEmbed::External(url) if url == "https://example.com/img.png"));
    }

    #[test]
    fn test_video_url_passthrough() {
        let result = process_media_src("https://example.com/demo.mp4", None, ImageMode::Embed, None);
        assert!(matches!(result, MediaEmbed::External(url) if url == "https://example.com/demo.mp4"));
    }

    #[test]
    fn test_missing_video_is_not_found() {
        let result = process_media_src("does-not-exist.mp4", None, ImageMode::External, None);
        assert!(matches!(result, MediaEmbed::NotFound(src) if src == "does-not-exist.mp4"));
    }

    #[test]
    fn test_video_copied_to_assets() {
        let dir = std::env::temp_dir().join(format!("sldr-video-{}", std::process::id()));
        let src_dir = dir.join("slides");
        let assets = dir.join("assets");
        std::fs::create_dir_all(&src_dir).unwrap();
        std::fs::write(src_dir.join("clip.mp4"), b"not really mp4").unwrap();

        let result =
            process_media_src("clip.mp4", Some(&src_dir), ImageMode::External, Some(&assets));
        match result {
            MediaEmbed::AssetFile {
                html_src,
                dest_path,
            } => {
                assert_eq!(html_src, "assets/clip.mp4");
                assert_eq!(std::fs::read(dest_path).unwrap(), b"not really mp4");
            }
            _ => panic!("expected AssetFile"),
        }

        let result = process_media_src("clip.mp4", Some(&src_dir), ImageMode::Embed, None);
        assert!(matches!(result, MediaEmbed::DataUri(uri) if uri.starts_with("data:video/mp4;base64,")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_video_tag() {
        let tag = video_tag("demo.mp4", video_mime("demo.MP4"), "controls loop");
        assert!(tag.contains("<video controls loop"));
        assert!(tag.contains("video/mp4"));
        assert!(tag.contains("demo.mp4"));
        assert_eq!(video_mime("a.webm"), "video/webm");
    }
}
