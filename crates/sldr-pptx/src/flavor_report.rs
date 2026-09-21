use crate::{Disposition as D, Report};

/// Report flavor features not projected by the native adapter. Asset resolution
/// belongs to the caller; low-level writers never discover local files.
pub fn flavor_report(flavor: &sldr_core::flavor::Flavor) -> Report {
    let mut report = Report::default();
    for (i, _) in flavor.logos.iter().enumerate() {
        report.record(None, "flavor", &format!("logos/{i}"), "logo_asset", D::Unsupported,
            "Logo is not yet supplied to native PPTX; retain HTML or choose explicit lossy export");
    }
    if flavor.custom_css.as_ref().is_some_and(|s| !s.trim().is_empty()) {
        report.record(None, "flavor", "custom_css", "custom_css", D::Unsupported,
            "CSS is not interpreted by the native PPTX writer");
    }
    if flavor.background.background_type.is_some() || flavor.background.value.is_some() || flavor.background.opacity.is_some() {
        report.record(None, "flavor", "background", "background_override", D::Unsupported,
            "Only colors.background currently maps to the master; CSS backgrounds/effects do not");
    }
    // Invalid CSS colors must not silently become the Theme fallback palette.
    for (name, color) in [("background", &flavor.colors.background), ("text", &flavor.colors.text),
        ("accent", &flavor.colors.accent), ("primary", &flavor.colors.primary),
        ("secondary", &flavor.colors.secondary), ("surface", &flavor.colors.surface),
        ("text_dim", &flavor.colors.text_dim), ("muted", &flavor.colors.muted)] {
        if color.as_ref().is_some_and(|c| crate::norm_hex(Some(c)).is_none()) {
            report.record(None, "flavor", name, "non_hex_color", D::Unsupported,
                "Native palette requires #RGB or #RRGGBB; fallback color would be used");
        }
    }
    report
}
