use crate::{Disposition as D, Report};

/// Report flavor features not projected by the native adapter. Asset resolution
/// belongs to the caller; low-level writers never discover local files.
pub fn flavor_report(flavor: &sldr_core::flavor::Flavor, brand: &crate::Brand) -> Report {
    let mut report = Report::default();
    for (i, _) in flavor.logos.iter().enumerate() {
        if brand.logos.iter().any(|l| l.index == i) {
            report.record(None, "flavor", &format!("logos/{i}"), "logo_asset", D::Converted,
                "Logo placed on the matching slide layouts");
        } else {
            report.record(None, "flavor", &format!("logos/{i}"), "logo_asset", D::Unsupported,
                "Logo file could not be resolved or rasterized; it is missing from the PowerPoint");
        }
    }
    if flavor.custom_css.as_ref().is_some_and(|s| !s.trim().is_empty()) {
        report.record(None, "flavor", "custom_css", "custom_css", D::Unsupported,
            "CSS is not interpreted by the native PPTX writer");
    }
    let has_bg = flavor.background.background_type.as_deref().is_some_and(|t| t != "color") && flavor.background.value.is_some();
    if has_bg && brand.background.is_some() {
        report.record(None, "flavor", "background", "background_override", D::Baked,
            "Background rendered to a picture on the slide master (not editable geometry)");
    } else if has_bg {
        report.record(None, "flavor", "background", "background_override", D::Unsupported,
            "Background could not be rendered; only colors.background reaches the master");
    }
    // Invalid CSS colors must not silently become the Theme fallback palette.
    for (name, color) in [("background", &flavor.colors.background), ("text", &flavor.colors.text),
        ("accent", &flavor.colors.accent), ("primary", &flavor.colors.primary),
        ("secondary", &flavor.colors.secondary), ("surface", &flavor.colors.surface),
        ("text_dim", &flavor.colors.text_dim), ("muted", &flavor.colors.muted)] {
        if color.as_ref().is_some_and(|c| crate::css_hex(c, None).is_none()) {
            report.record(None, "flavor", name, "non_hex_color", D::Unsupported,
                "Native palette requires #RGB or #RRGGBB; fallback color would be used");
        }
    }
    report
}
