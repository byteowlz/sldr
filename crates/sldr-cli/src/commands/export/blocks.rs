//! Freeform PPTX projection. Source blocks remain authoritative; browser work
//! lives in this CLI adapter, never in the pure writer or canonical formats.
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use sldr_core::{config::Config, flavor::Flavor, slide::Slide};
use sldr_pptx::{Disposition, Report, ZoneContent};
use sldr_renderer::{LayoutDef, Zone, ZoneRep};

pub(super) struct RasterContext<'a> {
    pub config: &'a Config,
    pub flavor: &'a Flavor,
    pub lang: Option<&'a str>,
    pub default_lang: &'a str,
    pub browser: Option<&'a Path>,
}

pub(super) struct BlockExport {
    pub layout: LayoutDef,
    pub fields: Vec<(String, ZoneContent)>,
    pub report: Report,
}

/// Whether a markdown fragment is exactly one image and nothing else.
fn is_single_image(md: &str) -> bool {
    let t = md.trim();
    t.starts_with("![") && t.ends_with(')') && t.matches("![").count() == 1 && !t.contains('\n')
}

fn local_svg(md: &str, slide: &Path) -> Option<PathBuf> {
    if !is_single_image(md) {
        return None;
    }
    let src = super::extract_img_src(md)?;
    if ["http://", "https://", "data:"]
        .iter()
        .any(|scheme| src.starts_with(scheme))
    {
        return None;
    }
    if !Path::new(&src)
        .extension()?
        .to_str()?
        .eq_ignore_ascii_case("svg")
    {
        return None;
    }
    Some(slide.parent()?.join(src))
}

pub(super) fn needs_browser(
    slide: &Slide,
    layout: &LayoutDef,
    lang: Option<&str>,
    default_lang: &str,
) -> bool {
    if !layout.blocks {
        return false;
    }
    let selected = sldr_core::lang::select_language(&slide.content, lang, default_lang);
    sldr_renderer::split_blocks(&selected.content)
        .unwrap_or_default()
        .iter()
        .any(|block| local_svg(&block.markdown, &slide.path).is_some())
}

pub(super) fn prepare(
    slide: &Slide,
    base: &LayoutDef,
    raster: &RasterContext<'_>,
    index: usize,
) -> BlockExport {
    let mut out = BlockExport {
        layout: base.clone(),
        fields: Vec::new(),
        report: Report::default(),
    };
    let selected =
        sldr_core::lang::select_language(&slide.content, raster.lang, raster.default_lang);
    for (i, block) in sldr_renderer::split_blocks(&selected.content)
        .unwrap_or_default()
        .iter()
        .enumerate()
    {
        let name = format!("block{}", i + 1);
        let image = is_single_image(&block.markdown);
        // A missing/unreadable image is still a picture binding, not successfully
        // converted native text. Let the writer report unresolved picture input.
        let zone = Zone {
            name: name.clone(),
            ph: None,
            idx: None,
            rep: if image {
                ZoneRep::Picture
            } else {
                ZoneRep::PlaceholderText
            },
            x: block.x,
            y: block.y,
            w: block.w,
            h: block.h,
        };
        let media = if let Some(path) = local_svg(&block.markdown, &slide.path) {
            let result = svg_picture(raster, slide, &zone, &path, i + 1);
            let (disposition, remedy) = if result.is_ok() {
                (Disposition::Baked, "SVG block is a PNG of its isolated render, not editable vector geometry; original markdown and SVG source are unchanged")
            } else {
                (Disposition::Unsupported, "SVG block could not be decoded/rasterized; check the local asset and installed Chrome/Chromium. Original source is unchanged")
            };
            let part = format!("ppt/slides/slide{}.xml", index + 1);
            out.report
                .record(Some(&part), &part, &name, "svg_raster", disposition, remedy);
            if let Err(error) = &result {
                println!("  warning: slide '{}', {name}: {error:#}", slide.name);
            }
            result.ok()
        } else {
            image
                .then(|| super::resolve_media(&block.markdown, &slide.path))
                .flatten()
        };
        out.layout.zones.push(zone);
        out.fields.push((
            name,
            media.unwrap_or_else(|| ZoneContent::Markdown(block.markdown.clone())),
        ));
    }
    out
}

fn svg_picture(
    raster: &RasterContext<'_>,
    slide: &Slide,
    zone: &Zone,
    path: &Path,
    index: usize,
) -> Result<ZoneContent> {
    let browser = raster
        .browser
        .context("SVG pictures need installed Chrome/Chromium")?;
    let bytes = std::fs::read(path).context("local SVG is unreadable")?;
    super::super::brand::check_svg(browser, &bytes)?;
    // Render just this block: neighboring native text/images must not be baked
    // into its transparent crop. Keep its layout, placement and flavor semantics.
    let png = super::raster_slide(
        slide,
        raster.flavor,
        raster.lang,
        raster.default_lang,
        raster.config,
        browser,
        super::RasterView::Block(index),
    )?;
    super::crop_regions(&png, &[zone])?
        .into_iter()
        .next()
        .map(|(_, content)| content)
        .context("SVG block crop is empty")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn svg_binding_is_local_single_image_only() {
        let source = Path::new("library/slide.md");
        assert_eq!(
            local_svg("![art](media/art.SVG)", source),
            Some(PathBuf::from("library/media/art.SVG"))
        );
        for md in [
            "![](https://example.invalid/art.svg)",
            "![](data:image/svg+xml,a.svg)",
            "![](art.png)",
            "![](art.svg)\ncaption",
        ] {
            assert!(local_svg(md, source).is_none(), "{md}");
        }
    }

    #[test]
    fn unavailable_svg_remains_unsupported_picture_and_text_stays_native() -> Result<()> {
        let slide = Slide::from_str("probe", "probe.md", "---\nlayout: freeform\n---\n::block x=5 y=5 w=40 h=40::\n![](missing.svg)\n::block x=50 y=5 w=40 h=40::\nEditable neighbor\n");
        let registry = sldr_renderer::LayoutRegistry::builtin();
        let flavor = Flavor::default();
        let config = Config::default();
        let ctx = RasterContext {
            config: &config,
            flavor: &flavor,
            lang: None,
            default_lang: "en",
            browser: None,
        };
        let result = prepare(&slide, registry.resolve("freeform")?, &ctx, 0);
        assert_eq!(
            result
                .layout
                .zones
                .iter()
                .find(|z| z.name == "block1")
                .map(|z| z.rep),
            Some(ZoneRep::Picture)
        );
        assert_eq!(
            result
                .layout
                .zones
                .iter()
                .find(|z| z.name == "block2")
                .map(|z| z.rep),
            Some(ZoneRep::PlaceholderText)
        );
        assert!(
            matches!(&result.fields[1].1, ZoneContent::Markdown(text) if text.contains("Editable neighbor"))
        );
        assert!(result
            .report
            .findings
            .iter()
            .any(|f| f.element == "block1" && f.disposition == Disposition::Unsupported));
        Ok(())
    }
}
