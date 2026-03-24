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
fn is_video(path: &str) -> bool {
    let lower = path.to_lowercase();
    lower.ends_with(".mp4")
        || lower.ends_with(".webm")
        || lower.ends_with(".mov")
        || lower.ends_with(".mkv")
        || lower.ends_with(".avi")
        || lower.ends_with(".ogv")
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

    // Videos always stay external
    if is_video(src) {
        return MediaEmbed::External(src.to_string());
    }

    // Not an image we can handle
    if !is_image(src) {
        return MediaEmbed::External(src.to_string());
    }

    // Resolve the local path
    let resolved = resolve_path(src, slide_dir);

    if !resolved.exists() {
        warn!("Image not found: {}", resolved.display());
        return MediaEmbed::NotFound(src.to_string());
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

/// Resolve a relative path against the slide directory
fn resolve_path(src: &str, slide_dir: Option<&Path>) -> PathBuf {
    let path = Path::new(src);

    // Already absolute
    if path.is_absolute() {
        return path.to_path_buf();
    }

    // Expand ~ home directory
    if src.starts_with('~') {
        if let Some(home) = dirs::home_dir() {
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

/// Generate a `<video>` tag for a video source.
/// Videos are always external - too large to embed.
pub fn video_tag(src: &str, attrs: &str) -> String {
    let mime = if src.ends_with(".webm") {
        "video/webm"
    } else if src.ends_with(".mov") {
        "video/quicktime"
    } else {
        "video/mp4"
    };

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
    fn test_video_passthrough() {
        let result = process_media_src("demo.mp4", None, ImageMode::Embed, None);
        assert!(matches!(result, MediaEmbed::External(url) if url == "demo.mp4"));
    }

    #[test]
    fn test_video_tag() {
        let tag = video_tag("demo.mp4", "controls loop");
        assert!(tag.contains("<video controls loop"));
        assert!(tag.contains("video/mp4"));
        assert!(tag.contains("demo.mp4"));
    }
}
