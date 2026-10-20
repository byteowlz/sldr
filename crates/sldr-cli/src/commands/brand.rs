//! Resolve a flavor's branding (background artwork, logos) into ready
//! PNG/JPEG images for the native PPTX writer. The writer stays pure; this is
//! where files are found and converted.
//!
//! Raster files are decoded and re-encoded directly. Anything the browser
//! draws — an SVG, a CSS gradient background — is rendered with the same
//! headless Chrome the rest of export uses, so PowerPoint shows what the
//! HTML deck shows. Without a browser those pieces are reported, not faked.

use std::io::Cursor;
use std::path::Path;

use anyhow::{bail, Context, Result};
use base64::Engine;
use image::{DynamicImage, GenericImageView, ImageFormat};
use sldr_core::flavor::{Flavor, LogoPlacement};
use sldr_pptx::{Brand, BrandImage, BrandLogo};

const SLIDE_W: u32 = 1920;
const SLIDE_H: u32 = 1080;

pub fn resolve(flavor: &Flavor, browser: Option<&Path>) -> (Brand, Vec<String>) {
    let mut warnings = Vec::new();
    let assets = flavor.source_dir.as_ref().map(|d| d.join("assets"));
    let mut brand = Brand::default();

    match background(flavor, assets.as_deref(), browser) {
        Ok(bg) => brand.background = bg,
        Err(e) => warnings.push(format!("background: {e:#}")),
    }
    for (index, logo) in flavor.logos.iter().enumerate() {
        match resolve_logo(logo, index, assets.as_deref(), browser) {
            Ok(l) => brand.logos.push(l),
            Err(e) => warnings.push(format!("logo {}: {e:#}", logo.file)),
        }
    }
    (brand, warnings)
}

fn background(flavor: &Flavor, assets: Option<&Path>, browser: Option<&Path>) -> Result<Option<BrandImage>> {
    let kind = flavor.background.background_type.as_deref().unwrap_or("color");
    let Some(value) = flavor.background.value.as_deref() else { return Ok(None) };
    let base = flavor.colors.background.clone().unwrap_or_else(|| "#000".into());
    match kind {
        "image" | "svg" => {
            let path = assets.context("flavor has no assets folder")?.join(value.trim_start_matches('/'));
            let bytes = std::fs::read(&path).with_context(|| format!("cannot read {}", path.display()))?;
            if is_svg(value) {
                let html = format!(
                    "<html><body style=\"margin:0;background:{base}\"><img src=\"data:image/svg+xml;base64,{}\" style=\"position:fixed;inset:0;width:100%;height:100%;object-fit:cover\"></body></html>",
                    b64(&bytes)
                );
                let img = screenshot(browser.context("an SVG background needs Chrome/Chromium to render")?, &html, SLIDE_W, SLIDE_H, false)?;
                Ok(Some(encode(img, false)?))
            } else {
                let img = image::load_from_memory(&bytes).with_context(|| format!("cannot decode {}", path.display()))?;
                let cover = img.resize_to_fill(SLIDE_W, SLIDE_H, image::imageops::FilterType::Lanczos3);
                Ok(Some(encode(cover, false)?))
            }
        }
        "gradient" => {
            let html = format!(
                "<html><body style=\"margin:0\"><div style=\"position:fixed;inset:0;background:{base};background:{value}\"></div></body></html>"
            );
            let img = screenshot(browser.context("a gradient background needs Chrome/Chromium to render")?, &html, SLIDE_W, SLIDE_H, false)?;
            Ok(Some(encode(img, false)?))
        }
        _ => Ok(None),
    }
}

fn resolve_logo(logo: &LogoPlacement, index: usize, assets: Option<&Path>, browser: Option<&Path>) -> Result<BrandLogo> {
    let path = assets.context("flavor has no assets folder")?.join(logo.file.trim_start_matches('/'));
    let bytes = std::fs::read(&path).with_context(|| format!("cannot read {}", path.display()))?;
    let w = css_len_pct(&logo.width, SLIDE_W).context("logo width must be %, px or vw")?;
    let image = if is_svg(&logo.file) {
        // Render at 4K width for a crisp picture, then trim to the drawing.
        let px = ((w / 100.0) * 3840.0).round().clamp(64.0, 3840.0) as u32;
        let html = format!(
            "<html><body style=\"margin:0;background:transparent\"><img src=\"data:image/svg+xml;base64,{}\" style=\"display:block;width:{px}px\"></body></html>",
            b64(&bytes)
        );
        let shot = screenshot(browser.context("an SVG logo needs Chrome/Chromium to render")?, &html, 3840, 2160, true)?;
        let (trimmed, _, _) = super::export::trim_transparent(&shot.to_rgba8());
        encode(DynamicImage::ImageRgba8(trimmed), true)?
    } else {
        let img = image::load_from_memory(&bytes).with_context(|| format!("cannot decode {}", path.display()))?;
        match image::guess_format(&bytes) {
            Ok(ImageFormat::Png) => BrandImage { width_px: img.width(), height_px: img.height(), bytes, ext: "png".into() },
            Ok(ImageFormat::Jpeg) => BrandImage { width_px: img.width(), height_px: img.height(), bytes, ext: "jpeg".into() },
            _ => encode(img, true)?,
        }
    };
    let h = w * (image.height_px as f64 / image.width_px.max(1) as f64) * (SLIDE_W as f64 / SLIDE_H as f64);
    let (x, y) = match (&logo.x, &logo.y) {
        (Some(x), Some(y)) => (
            css_len_pct(x, SLIDE_W).context("logo x must be %, px or vw")?,
            css_len_pct(y, SLIDE_H).context("logo y must be %, px or vh")?,
        ),
        _ => preset(&logo.position, w, h),
    };
    Ok(BrandLogo { image, x, y, w, opacity: logo.opacity, layouts: logo.layouts.clone(), index })
}

/// The HTML renderer's position presets (3% inset), in slide percent.
fn preset(position: &str, w: f64, h: f64) -> (f64, f64) {
    match position {
        "top-left" => (3.0, 3.0),
        "top-center" => (50.0 - w / 2.0, 3.0),
        "bottom-left" => (3.0, 97.0 - h),
        "bottom-center" => (50.0 - w / 2.0, 97.0 - h),
        "bottom-right" => (97.0 - w, 97.0 - h),
        _ => (97.0 - w, 3.0),
    }
}

/// `21.2%` → 21.2; `120px` → percent of `axis_px`; `8vw`/`8vh` → 8.
fn css_len_pct(v: &str, axis_px: u32) -> Option<f64> {
    let v = v.trim();
    if let Some(p) = v.strip_suffix('%') {
        return p.trim().parse().ok();
    }
    if let Some(p) = v.strip_suffix("px") {
        return p.trim().parse::<f64>().ok().map(|px| px / axis_px as f64 * 100.0);
    }
    if let Some(p) = v.strip_suffix("vw").or_else(|| v.strip_suffix("vh")) {
        return p.trim().parse().ok();
    }
    None
}

fn is_svg(name: &str) -> bool {
    name.to_ascii_lowercase().ends_with(".svg")
}

fn b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// PNG when transparency matters, else JPEG (photographic backgrounds).
fn encode(img: DynamicImage, alpha: bool) -> Result<BrandImage> {
    let (w, h) = img.dimensions();
    let mut buf = Vec::new();
    if alpha {
        img.write_to(&mut Cursor::new(&mut buf), ImageFormat::Png)?;
        Ok(BrandImage { bytes: buf, ext: "png".into(), width_px: w, height_px: h })
    } else {
        let rgb = DynamicImage::ImageRgb8(img.to_rgb8());
        let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, 88);
        enc.encode_image(&rgb)?;
        Ok(BrandImage { bytes: buf, ext: "jpeg".into(), width_px: w, height_px: h })
    }
}

/// Screenshot `html` at exactly `w`×`h` CSS px. Screenshot and dump-DOM
/// modes have different viewport behavior; never compensate one using the other.
/// `transparent` keeps an alpha background.
pub fn screenshot(browser: &Path, html: &str, w: u32, h: u32, transparent: bool) -> Result<DynamicImage> {
    let dir = tempfile::tempdir()?;
    let page = dir.path().join("page.html");
    let png = dir.path().join("shot.png");
    std::fs::write(&page, capture_canvas(html, w, h))?;
    let mut cmd = std::process::Command::new(browser);
    cmd.args(["--headless=new", "--no-sandbox", "--disable-gpu", "--hide-scrollbars", "--force-device-scale-factor=1", "--virtual-time-budget=4000"])
        .arg(format!("--window-size={w},{h}"))
        .arg(format!("--screenshot={}", png.display()));
    if transparent {
        cmd.arg("--default-background-color=00000000");
    }
    let status = cmd
        .arg(format!("file://{}", page.display()))
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .context("failed to launch browser")?;
    if !status.success() || !png.exists() {
        bail!("browser did not produce a screenshot");
    }
    open_capture(&png, w, h)
}

/// Fix the export canvas independently of Chrome's pre-capture CSS viewport.
/// Some builds keep `vh` at the dump-DOM height even when the screenshot surface
/// has the requested height. Never resize the window and crop away authored content.
pub(crate) fn capture_canvas(html: &str, w: u32, h: u32) -> String {
    let style = format!("<style>html,body{{width:{w}px!important;height:{h}px!important;margin:0!important}}\
        .sldr-deck,.sldr-deck[data-aspect-lock]{{width:{w}px!important;height:{h}px!important;\
        position:relative!important;inset:auto!important;margin:0!important}}</style>");
    let at = html.find("</head>").or_else(|| html.find("<body")).unwrap_or(0);
    format!("{}{style}{}", &html[..at], &html[at..])
}

/// Refuse unexpected capture geometry rather than silently cutting off content.
pub(crate) fn open_capture(path: &Path, w: u32, h: u32) -> Result<DynamicImage> {
    let img = image::open(path).context("screenshot unreadable")?;
    if img.dimensions() != (w, h) {
        bail!("Screenshot dimensions {:?} do not match requested {w}×{h}; refusing to crop slide content", img.dimensions());
    }
    Ok(img)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    fn fake_browser(root: &Path, w: u32, h: u32) -> std::path::PathBuf {
        use std::os::unix::fs::PermissionsExt;
        image::RgbaImage::from_pixel(w, h, image::Rgba([17, 33, 55, 255])).save(root.join("capture.png")).unwrap();
        let script = root.join("chrome");
        std::fs::write(&script, format!(r#"#!/usr/bin/env bash
for arg in "$@"; do
  if [ "$arg" = --dump-dom ]; then echo VP993VP; exit 0; fi
done
printf '%s\n' "$@" > '{dir}/args'
for arg in "$@"; do
  case "$arg" in --screenshot=*) cp '{dir}/capture.png' "${{arg#--screenshot=}}";; esac
done
"#, dir = root.display())).unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        script
    }

    #[test]
    #[cfg(unix)]
    fn screenshot_uses_capture_dimensions_not_dump_dom_shortfall() {
        let dir = tempfile::tempdir().unwrap();
        let browser = fake_browser(dir.path(), 960, 540);
        let img = screenshot(&browser, "<html></html>", 960, 540, false).unwrap();
        assert_eq!(img.dimensions(), (960, 540));
        let args = std::fs::read_to_string(dir.path().join("args")).unwrap();
        assert!(args.lines().any(|a| a == "--window-size=960,540"), "{args}");
    }

    #[test]
    #[cfg(unix)]
    fn screenshot_rejects_unexpected_capture_dimensions_instead_of_cropping() {
        let dir = tempfile::tempdir().unwrap();
        let browser = fake_browser(dir.path(), 960, 627);
        let result = screenshot(&browser, "<html></html>", 960, 540, false);
        assert!(result.is_err(), "unexpected capture dimensions were accepted");
        let error = result.err().unwrap();
        assert!(error.to_string().contains("dimensions"), "{error}");
    }

    #[test]
    fn capture_canvas_keeps_doctype_and_precedes_body_layout() {
        let html = capture_canvas("<!doctype html><html><body>sample</body></html>", 960, 540);
        assert!(html.starts_with("<!doctype html>"));
        assert!(html.contains("height:540px!important"));
        assert!(html.find("<style>").unwrap() < html.find("<body>").unwrap());
    }

    #[test]
    fn lengths_to_percent() {
        assert_eq!(css_len_pct("21.2%", 1920), Some(21.2));
        assert_eq!(css_len_pct("192px", 1920), Some(10.0));
        assert_eq!(css_len_pct("8vw", 1920), Some(8.0));
        assert_eq!(css_len_pct("auto", 1920), None);
    }

    #[test]
    fn presets_inset_three_percent() {
        assert_eq!(preset("top-left", 10.0, 5.0), (3.0, 3.0));
        assert_eq!(preset("bottom-right", 10.0, 5.0), (87.0, 92.0));
        assert_eq!(preset("unknown", 10.0, 5.0), (87.0, 3.0));
    }
}
