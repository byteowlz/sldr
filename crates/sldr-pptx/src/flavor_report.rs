use crate::{Disposition as D, Report};

/// Report flavor features not projected by the native adapter. Asset resolution
/// belongs to the caller; low-level writers never discover local files.
/// Fonts shipped with Windows, macOS and Office, or generic families.
fn is_common_font(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    matches!(n.as_str(),
        "arial" | "helvetica" | "helvetica neue" | "calibri" | "cambria" | "times new roman" | "times" | "georgia" |
        "verdana" | "tahoma" | "trebuchet ms" | "segoe ui" | "courier new" | "consolas" | "menlo" | "monaco" |
        "garamond" | "palatino" | "book antiqua" | "century gothic" | "franklin gothic" | "gill sans" | "optima" |
        "sans-serif" | "serif" | "monospace" | "system-ui" | "-apple-system" | "ui-sans-serif" | "ui-serif" | "ui-monospace")
}

pub fn flavor_report(flavor: &sldr_core::flavor::Flavor, brand: &crate::Brand) -> Report {
    let mut report = Report::default();
    for (i, _) in flavor.logos.iter().enumerate() {
        if brand.logos.iter().any(|l| l.index == i) {
            report.record(None, "flavor", &format!("logos/{i}"), "logo_asset", D::Converted,
                "Logo placed on the matching slide layouts");
        } else {
            report.record(None, "flavor", &format!("logos/{i}"), "logo_asset", D::Unsupported,
                "Logo missing from the PowerPoint: the file was not found or, for an SVG, no Chrome/Chromium was available to rasterize it (see the `flavor` warning above; CHROME_BIN points at a browser)");
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
    // A theme font that is not on most machines falls back to a serif in
    // PowerPoint. Informational: the fallback is named, strict still passes.
    for (role, stack) in [("heading_font", &flavor.typography.heading_font), ("body_font", &flavor.typography.body_font)] {
        let Some(stack) = stack else { continue };
        let mut families = stack.split(',').map(|f| f.trim().trim_matches(|c| c == '"' || c == '\'')).filter(|f| !f.is_empty());
        let Some(first) = families.next() else { continue };
        if !is_common_font(first) {
            let fallback = families.find(|f| is_common_font(f)).unwrap_or("the viewer's default");
            report.record(None, "flavor", role, "font_availability", D::Converted,
                &format!("'{first}' must be installed on the viewing machine; otherwise PowerPoint shows {fallback}. The flavor's font files are not embedded"));
        }
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
