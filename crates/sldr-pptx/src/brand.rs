//! Flavor branding in native PPTX: background artwork on the slide master and
//! logos on the slide layouts they are declared for — so every slide inherits
//! them from where PowerPoint users expect ("View → Slide Master"), and they
//! cannot be nudged on a single slide by accident.
//!
//! Pure: the caller resolves the flavor's asset files and hands over ready
//! PNG/JPEG bytes (low-level writers never discover local files).

/// A raster image ready to embed.
#[derive(Debug, Clone)]
pub struct BrandImage {
    pub bytes: Vec<u8>,
    /// `png` or `jpeg`.
    pub ext: String,
    pub width_px: u32,
    pub height_px: u32,
}

/// One flavor logo placed on the layouts it applies to.
#[derive(Debug, Clone)]
pub struct BrandLogo {
    pub image: BrandImage,
    /// Left / top edge and width as percent of the slide (0–100).
    pub x: f64,
    pub y: f64,
    pub w: f64,
    /// 0.0–1.0.
    pub opacity: f32,
    /// sldr layout names, or `all`.
    pub layouts: Vec<String>,
    /// Index in the flavor's `logos` list (for reporting).
    pub index: usize,
}

impl BrandLogo {
    pub fn applies_to(&self, layout: &str) -> bool {
        // A slide exported as one picture already shows its logos.
        layout != crate::RASTER_LAYOUT_NAME && self.layouts.iter().any(|l| l == "all" || l == layout)
    }
}

/// Everything branding-related the writer places. Empty by default.
#[derive(Debug, Clone, Default)]
pub struct Brand {
    /// Full-slide background, already cropped/scaled to 16:9.
    pub background: Option<BrandImage>,
    pub logos: Vec<BrandLogo>,
}

/// Media part path for the background.
pub(crate) fn background_part(img: &BrandImage) -> String {
    format!("ppt/media/brand-background.{}", img.ext)
}

/// Media part path for a logo.
pub(crate) fn logo_part(logo: &BrandLogo) -> String {
    format!("ppt/media/brand-logo{}.{}", logo.index + 1, logo.image.ext)
}

/// `<p:bg>` for the master: the picture when present, else the dk1 color.
pub(crate) fn master_background(rel: Option<&str>) -> String {
    match rel {
        Some(rid) => format!(
            "<p:bg><p:bgPr><a:blipFill dpi=\"0\" rotWithShape=\"1\"><a:blip r:embed=\"{rid}\"/><a:srcRect/><a:stretch><a:fillRect/></a:stretch></a:blipFill><a:effectLst/></p:bgPr></p:bg>"
        ),
        None => "<p:bg><p:bgPr><a:solidFill><a:schemeClr val=\"dk1\"/></a:solidFill><a:effectLst/></p:bgPr></p:bg>".into(),
    }
}

/// A logo picture for a layout's shape tree. Height follows the image's
/// aspect ratio from the width the flavor declares.
pub(crate) fn logo_pic(logo: &BrandLogo, shape_id: usize, rid: &str) -> String {
    let cx = crate::emu_x(logo.w);
    let cy = if logo.image.width_px > 0 {
        (cx as f64 * logo.image.height_px as f64 / logo.image.width_px as f64).round() as i64
    } else {
        crate::emu_y(logo.w)
    };
    let alpha = if logo.opacity < 0.999 {
        format!("<a:alphaModFix amt=\"{}\"/>", (logo.opacity.clamp(0.0, 1.0) * 100_000.0).round() as i64)
    } else {
        String::new()
    };
    format!(
        r#"<p:pic><p:nvPicPr><p:cNvPr id="{shape_id}" name="Logo {n}" descr="Brand logo"/><p:cNvPicPr><a:picLocks noChangeAspect="1"/></p:cNvPicPr><p:nvPr userDrawn="1"/></p:nvPicPr>
<p:blipFill><a:blip r:embed="{rid}">{alpha}</a:blip><a:stretch><a:fillRect/></a:stretch></p:blipFill>
<p:spPr><a:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr></p:pic>"#,
        n = logo.index + 1,
        x = crate::emu_x(logo.x),
        y = crate::emu_y(logo.y),
    )
}

/// Parse a CSS color into bare uppercase hex, blending any alpha over
/// `over` (itself hex). Handles `#rgb`, `#rrggbb`, `#rrggbbaa`, `rgb()`,
/// `rgba()`. `None` for anything else (gradients, `var()`, names).
pub fn css_hex(color: &str, over: Option<&str>) -> Option<String> {
    let c = color.trim();
    let (r, g, b, a) = if let Some(hex) = c.strip_prefix('#') {
        let full: String = match hex.len() {
            3 | 4 => hex.chars().flat_map(|ch| [ch, ch]).collect(),
            6 | 8 => hex.to_string(),
            _ => return None,
        };
        let p = |i: usize| u8::from_str_radix(full.get(i..i + 2)?, 16).ok();
        (p(0)?, p(2)?, p(4)?, if full.len() == 8 { f64::from(p(6)?) / 255.0 } else { 1.0 })
    } else {
        let inner = c.strip_prefix("rgba(").or_else(|| c.strip_prefix("rgb("))?.strip_suffix(')')?;
        let parts: Vec<&str> = inner.split([',', '/', ' ']).map(str::trim).filter(|s| !s.is_empty()).collect();
        if parts.len() < 3 {
            return None;
        }
        let ch = |s: &str| -> Option<u8> {
            if let Some(p) = s.strip_suffix('%') {
                Some((p.parse::<f64>().ok()? * 2.55).round().clamp(0.0, 255.0) as u8)
            } else {
                Some(s.parse::<f64>().ok()?.round().clamp(0.0, 255.0) as u8)
            }
        };
        let alpha = match parts.get(3) {
            Some(s) if s.ends_with('%') => s.trim_end_matches('%').parse::<f64>().ok()? / 100.0,
            Some(s) => s.parse::<f64>().ok()?,
            None => 1.0,
        };
        (ch(parts[0])?, ch(parts[1])?, ch(parts[2])?, alpha.clamp(0.0, 1.0))
    };
    let (br, bg, bb) = over
        .and_then(|o| css_hex(o, None))
        .and_then(|h| Some((u8::from_str_radix(&h[0..2], 16).ok()?, u8::from_str_radix(&h[2..4], 16).ok()?, u8::from_str_radix(&h[4..6], 16).ok()?)))
        .unwrap_or((255, 255, 255));
    let mix = |fg: u8, bgc: u8| (f64::from(fg) * a + f64::from(bgc) * (1.0 - a)).round() as u8;
    Some(format!("{:02X}{:02X}{:02X}", mix(r, br), mix(g, bg), mix(b, bb)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn css_colors_to_hex() {
        assert_eq!(css_hex("#0a1018", None).as_deref(), Some("0A1018"));
        assert_eq!(css_hex("#fff", None).as_deref(), Some("FFFFFF"));
        assert_eq!(css_hex("rgb(255, 0, 0)", None).as_deref(), Some("FF0000"));
        // 5 % white over a near-black background → barely lighter than it.
        assert_eq!(css_hex("rgba(238, 243, 248, 0.05)", Some("#0a1018")).as_deref(), Some("151B23"));
        assert_eq!(css_hex("#00000080", Some("#ffffff")).as_deref(), Some("7F7F7F"));
        assert_eq!(css_hex("linear-gradient(red, blue)", None), None);
        assert_eq!(css_hex("var(--x)", None), None);
    }

    #[test]
    fn logo_height_follows_aspect_and_opacity_is_written() {
        let logo = BrandLogo {
            image: BrandImage { bytes: vec![], ext: "png".into(), width_px: 400, height_px: 100 },
            x: 80.0, y: 90.0, w: 10.0, opacity: 0.8, layouts: vec!["all".into()], index: 0,
        };
        let xml = logo_pic(&logo, 7, "rId9");
        let cx = crate::emu_x(10.0);
        assert!(xml.contains(&format!("cx=\"{cx}\" cy=\"{}\"", cx / 4)));
        assert!(xml.contains("alphaModFix amt=\"80000\""));
        assert!(logo.applies_to("framed") && !logo.applies_to(crate::RASTER_LAYOUT_NAME));
    }
}
