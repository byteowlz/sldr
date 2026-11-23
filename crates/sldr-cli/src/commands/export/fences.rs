//! Graphic fences in native PPTX export. A body segment that is exactly one
//! ```svg / ```html fence is a figure in HTML, not code: native text would
//! print its markup. Its zone becomes a picture of the real render instead,
//! reported as baked. The fence source stays authoritative and untouched.
use std::ops::Range;

use anyhow::{Context, Result};
use sldr_core::{media::fence_renders_markup, slide::Slide};
use sldr_pptx::{Disposition, Report, ZoneContent};
use sldr_renderer::{LayoutDef, Zone, ZoneRep};

use super::blocks::RasterContext;

/// Comment markers placed around the fence body of the probe render only.
/// Comments change neither layout nor selector matching.
const OPEN: &str = "<!--sldr-fence-->";
const CLOSE: &str = "<!--/sldr-fence-->";

/// Injected into the probe render: everything except the marked fence is
/// invisible (never removed, so the fence keeps its real layout), which makes
/// the painted pixels of the transparent capture exactly the fence.
pub(super) const ISOLATE: &str = "<style>body :not([data-sldr-fence],[data-sldr-fence] *){visibility:hidden!important}\
[data-sldr-fence=\"root\"]{visibility:visible}</style>\
<script>document.addEventListener('DOMContentLoaded',function(){\
var w=document.createTreeWalker(document.body,NodeFilter.SHOW_ELEMENT|NodeFilter.SHOW_TEXT|NodeFilter.SHOW_COMMENT),n,open=false;\
while((n=w.nextNode())){\
if(n.nodeType===8){if(n.data==='sldr-fence')open=true;else if(n.data==='/sldr-fence')open=false;continue;}\
if(!open)continue;\
var inside=n.parentElement&&n.parentElement.hasAttribute('data-sldr-fence');\
if(n.nodeType===1){n.setAttribute('data-sldr-fence',inside?'':'root');}\
else if(!inside&&n.data.trim()){var s=document.createElement('span');s.setAttribute('data-sldr-fence','root');n.parentNode.insertBefore(s,n);s.appendChild(n);}\
}});</script>";

/// What a slide's graphic fences contribute to its export.
#[derive(Default)]
pub(super) struct FenceExport {
    /// `(zone, picture)` for every fence that was pictured.
    pub fields: Vec<(String, ZoneContent)>,
    pub report: Report,
}

/// One ```svg / ```html fence that is a whole body segment.
#[derive(Debug, PartialEq, Eq)]
struct Fence<'a> {
    /// The fence language, as written (`svg` or `html`).
    kind: &'a str,
    /// The segment without surrounding whitespace, as it stands in the body.
    text: &'a str,
    /// Byte range of the fence body within `text`.
    body: Range<usize>,
    /// The body names a remote URL. Export never fetches it.
    remote: bool,
}

impl Fence<'_> {
    /// Account for a fence exported as a picture: the region is baked, and a
    /// remote reference is a separate, explicit gap in that picture.
    fn record(&self, report: &mut Report, slide: usize, zone: &str) {
        let part = format!("ppt/slides/slide{}.xml", slide + 1);
        report.record(Some(&part), &part, zone, "region_raster", Disposition::Baked, &format!(
            "This region is a ```{} fence: it is a picture of the real render, not editable geometry or text; the fence source is unchanged, neighboring zones stay editable text, and import leaves the picture alone",
            self.kind
        ));
        if self.remote {
            report.record(Some(&part), &part, zone, "remote_reference", Disposition::Unsupported,
                "The fence names remote URLs. Export never fetches them, so whatever they would load is missing from the picture; use local media or inline the resource");
        }
    }
}

/// Whether markup names a remote URL other than an XML namespace.
fn names_remote(body: &str) -> bool {
    let lower = body.to_ascii_lowercase();
    ["http://", "https://"].iter().any(|scheme| {
        lower.match_indices(scheme).any(|(at, _)| {
            let before = lower[..at].trim_end_matches(['"', '\'']);
            let attribute = before
                .strip_suffix('=')
                .and_then(|b| b.split_whitespace().last());
            !attribute.is_some_and(|name| name.starts_with("xmlns"))
        })
    })
}

/// The fence when `segment` is exactly one ```svg / ```html fence. Trailing
/// speaker notes are not part of the visible body. Anything else beside the
/// fence keeps the segment native text: a region picture would bake that
/// text too.
fn graphic(segment: &str) -> Option<Fence<'_>> {
    let text = segment.trim();
    let (mark, width, info) = opener(text.lines().next()?)?;
    if !fence_renders_markup(info) {
        return None;
    }
    let kind = info.trim().split(['{', ' ']).next()?;
    let start = text.find('\n')? + 1;
    let mut at = start;
    let (end, after) = text[start..].split_inclusive('\n').find_map(|line| {
        let here = at;
        at += line.len();
        let fence = line.trim();
        (fence.len() >= width && fence.chars().all(|c| c == mark)).then_some((here, at))
    })?;
    let tail = text[after..].trim();
    let notes = tail.starts_with("<!-- notes -->")
        || (tail.starts_with("<!-- notes:")
            && tail.ends_with("-->")
            && tail.matches("-->").count() == 1);
    (tail.is_empty() || notes).then(|| Fence {
        kind,
        text,
        body: start..end,
        remote: names_remote(&text[start..end]),
    })
}

/// A line that opens a code fence: its character, length and info string.
fn opener(line: &str) -> Option<(char, usize, &str)> {
    let mark = line.chars().next().filter(|c| matches!(c, '`' | '~'))?;
    let width = line.chars().take_while(|c| *c == mark).count();
    (width >= 3).then(|| (mark, width, &line[width..]))
}

/// Cheap pre-scan: whether a slide could need the browser for a fence.
pub(super) fn possible(content: &str) -> bool {
    content.lines().any(|line| {
        opener(line.trim_start()).is_some_and(|(_, _, info)| fence_renders_markup(info))
    })
}

/// Picture every text zone of `layout` whose whole segment is a graphic
/// fence. `content` is the slide body for the exported language. A fence that
/// cannot be pictured (no browser, a failed render, nothing painted) is left
/// out: the caller keeps its source text, which preflight reports.
pub(super) fn bake(
    slide: &Slide,
    layout: &LayoutDef,
    content: &str,
    raster: &RasterContext<'_>,
    index: usize,
) -> FenceExport {
    let mut out = FenceExport::default();
    if raster.browser.is_none() {
        return out; // announced once for the deck
    }
    let segments = sldr_renderer::split_segments(content);
    for zone in layout
        .zones
        .iter()
        .filter(|z| z.rep == ZoneRep::PlaceholderText)
    {
        let segment = match zone.name.as_str() {
            "heading" => segments.heading.as_deref(),
            "content" => segments.content.as_deref(),
            "left" => segments.left.as_deref(),
            "right" => segments.right.as_deref(),
            _ => None,
        };
        let Some(fence) = segment.and_then(graphic) else {
            continue;
        };
        let (name, kind) = (&zone.name, fence.kind);
        match picture(raster, slide, content, &fence, zone) {
            Ok(Some(picture)) => {
                fence.record(&mut out.report, index, name);
                out.fields.push((name.clone(), picture));
            }
            Ok(None) => println!(
                "  warning: slide '{}', {name}: the {kind} fence rendered nothing visible, left as text",
                slide.name
            ),
            Err(error) => println!(
                "  warning: slide '{}', {name}: {kind} fence picture failed ({error:#}), left as text",
                slide.name
            ),
        }
    }
    out
}

/// `content` with the fence body wrapped in the probe markers. `last` picks
/// the final occurrence: a `right` column may repeat the text of the `left`
/// one, and the right segment is always the later.
fn mark(content: &str, fence: &Fence<'_>, last: bool) -> Option<String> {
    let at = if last {
        content.rfind(fence.text)
    } else {
        content.find(fence.text)
    }?;
    let (start, end) = (at + fence.body.start, at + fence.body.end);
    Some(format!(
        "{}{OPEN}\n{}{CLOSE}\n{}",
        &content[..start],
        &content[start..end],
        &content[end..]
    ))
}

/// The fence of one zone as a picture of its real render. `None` when the
/// fence paints nothing: an invisible picture must not stand in for content.
fn picture(
    raster: &RasterContext<'_>,
    slide: &Slide,
    content: &str,
    fence: &Fence<'_>,
    zone: &Zone,
) -> Result<Option<ZoneContent>> {
    let browser = raster
        .browser
        .context("fence pictures need installed Chrome/Chromium")?;
    let mut probe = slide.clone();
    probe.content =
        mark(content, fence, zone.name == "right").context("fence not found in the slide body")?;
    let png = super::raster_slide(
        &probe,
        raster.flavor,
        raster.lang,
        raster.default_lang,
        raster.config,
        browser,
        super::RasterView::Fence,
    )?;
    crop(&png, zone)
}

/// Cut the fence out of its isolated full-slide render. The cut is the zone
/// box, grown to everything the fence painted: inside its zone the picture
/// lands pixel-exact, and a fence that overflows the zone is scaled into it
/// by the writer rather than clipped.
fn crop(png: &[u8], zone: &Zone) -> Result<Option<ZoneContent>> {
    let img = image::load_from_memory(png)
        .context("decoding fence render failed")?
        .to_rgba8();
    let (w, h) = img.dimensions();
    let mut painted: Option<(u32, u32, u32, u32)> = None;
    for (x, y, pixel) in img.enumerate_pixels() {
        if pixel[3] > 8 {
            let (x0, y0, x1, y1) = painted.unwrap_or((x, y, x, y));
            painted = Some((x0.min(x), y0.min(y), x1.max(x), y1.max(y)));
        }
    }
    let Some((px0, py0, px1, py1)) = painted else {
        return Ok(None);
    };
    let edge = |percent: f64, size: u32| {
        (percent / 100.0 * f64::from(size))
            .round()
            .clamp(0.0, f64::from(size)) as u32
    };
    let x0 = edge(zone.x, w).min(px0);
    let y0 = edge(zone.y, h).min(py0);
    let x1 = edge(zone.x + zone.w, w).max(px1 + 1);
    let y1 = edge(zone.y + zone.h, h).max(py1 + 1);
    let mut bytes = Vec::new();
    image::imageops::crop_imm(&img, x0, y0, x1 - x0, y1 - y0)
        .to_image()
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .context("encoding fence picture failed")?;
    Ok(Some(ZoneContent::Picture {
        bytes,
        ext: "png".into(),
        fit: None,
    }))
}

#[cfg(test)]
mod tests {
    use anyhow::ensure;

    use super::*;

    /// A 200x100 transparent render with opaque rectangles `(x0, y0, x1, y1)`.
    fn render(painted: &[(u32, u32, u32, u32)]) -> Result<Vec<u8>> {
        let mut img = image::RgbaImage::new(200, 100);
        for &(x0, y0, x1, y1) in painted {
            for y in y0..y1 {
                for x in x0..x1 {
                    img.put_pixel(x, y, image::Rgba([200, 30, 30, 255]));
                }
            }
        }
        let mut png = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)?;
        Ok(png)
    }

    #[test]
    fn only_a_lone_svg_or_html_fence_is_graphic() {
        let lone = "```svg\n<svg/>\n```";
        assert_eq!(
            graphic(lone),
            Some(Fence {
                kind: "svg",
                text: lone,
                body: 7..14,
                remote: false
            })
        );
        for (md, kind) in [
            ("\n~~~html {1}\n<div></div>\n~~~~\n", Some("html")),
            (
                "```svg\n<svg/>\n```\n\n<!-- notes -->\nSynthetic note",
                Some("svg"),
            ),
            (
                "```html\n<p>x</p>\n```\n<!-- notes: synthetic -->",
                Some("html"),
            ),
            ("```python\nprint(1)\n```", None),
            ("```svgz\n<svg/>\n```", None),
            ("```\n<svg/>\n```", None),
            ("Intro\n\n```svg\n<svg/>\n```", None),
            ("```svg\n<svg/>\n```\n\nCaption", None),
            ("```svg\n<svg/>\n```\n```svg\n<svg/>\n```", None),
            ("```svg\n<svg/>", None),
            ("``svg\n<svg/>\n``", None),
        ] {
            assert_eq!(graphic(md).map(|fence| fence.kind), kind, "{md}");
        }
    }

    #[test]
    fn prescan_finds_openers_of_markup_fences() {
        for (content, expected) in [
            ("x\n```svg\n<svg/>\n```", true),
            ("~~~html\n<b/>\n~~~", true),
            ("  ```svg {1}\n<svg/>\n  ```", true),
            ("```python\nprint('<svg>')\n```", false),
            ("Inline ```svg``` mention", false),
        ] {
            assert_eq!(possible(content), expected, "{content}");
        }
    }

    #[test]
    fn remote_urls_are_named_but_namespaces_are_not() -> Result<()> {
        let plain = "```svg\n<svg xmlns=\"http://www.w3.org/2000/svg\" xmlns:xlink='http://www.w3.org/1999/xlink'><rect/></svg>\n```";
        let remote = "```html\n<img src=\"HTTPS://example.invalid/a.png\">\n```";
        ensure!(
            !graphic(plain).context("svg fence")?.remote,
            "an XML namespace is not a remote reference"
        );
        let fence = graphic(remote).context("html fence")?;
        ensure!(fence.remote, "a remote image source went unnoticed");
        let mut report = Report::default();
        fence.record(&mut report, 1, "left");
        let found: Vec<_> = report
            .findings
            .iter()
            .map(|f| {
                (
                    f.part.as_str(),
                    f.element.as_str(),
                    f.feature.as_str(),
                    f.disposition,
                )
            })
            .collect();
        let expected = [
            (
                "ppt/slides/slide2.xml",
                "left",
                "region_raster",
                Disposition::Baked,
            ),
            (
                "ppt/slides/slide2.xml",
                "left",
                "remote_reference",
                Disposition::Unsupported,
            ),
        ];
        ensure!(found == expected, "{found:?}");
        Ok(())
    }

    #[test]
    fn markers_wrap_the_body_of_the_right_occurrence() -> Result<()> {
        let fence = "```svg\n<svg/>\n```";
        let content = format!("::left::\n{fence}\n::right::\n{fence}\n");
        let found = graphic(fence).context("fence not recognized")?;
        let marked = format!("```svg\n{OPEN}\n<svg/>\n{CLOSE}\n```");
        let first = mark(&content, &found, false);
        ensure!(
            first == Some(format!("::left::\n{marked}\n::right::\n{fence}\n")),
            "{first:?}"
        );
        let last = mark(&content, &found, true);
        ensure!(
            last == Some(format!("::left::\n{fence}\n::right::\n{marked}\n")),
            "{last:?}"
        );
        ensure!(
            mark("no fence here", &found, false).is_none(),
            "marked a body without the fence"
        );
        Ok(())
    }

    #[test]
    fn cut_is_the_zone_box_grown_to_what_the_fence_painted() -> Result<()> {
        // Zone box in the 200x100 render: x 50..150, y 20..60.
        let layout = LayoutDef::from_source(
            "probe",
            "<!-- sldr:zone name=content ph=body idx=1 rep=placeholder-text x=25 y=20 w=50 h=40 -->\n<div>{{content}}</div>\n",
        );
        let zone = layout.zones.first().context("probe layout has no zone")?;
        for (painted, expected) in [
            // Inside the zone: exactly the zone box, a small figure is not scaled up.
            (vec![(60, 30, 80, 40)], Some((100, 40))),
            // Overflowing left and below: grown, never clipped.
            (vec![(40, 30, 150, 75)], Some((110, 55))),
            // Nothing painted: no picture at all.
            (vec![], None),
        ] {
            let cut = match crop(&render(&painted)?, zone)? {
                Some(ZoneContent::Picture {
                    bytes, fit: None, ..
                }) => {
                    let img = image::load_from_memory(&bytes)?;
                    Some((img.width(), img.height()))
                }
                Some(_) => anyhow::bail!("not a header-sized picture"),
                None => None,
            };
            ensure!(cut == expected, "{painted:?}: {cut:?}");
        }
        Ok(())
    }
}
