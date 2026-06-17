//! Bundled sample deck — a canonical set of placeholder slides used to
//! evaluate any flavor against the same content.
//!
//! Compiled into the binary via `include_str!` so `sldr flavor sample` and
//! the flavor-builder UI work offline with zero filesystem state.
//!
//! Producers:
//! - `sldr flavor sample [--flavor X]` — render to a temp file and open
//! - `sldr serve` exposes `GET /api/sample` (markdown sources) and
//!   `GET /sample.html` (rendered output) for agents and the builder UI.

use std::path::PathBuf;

use anyhow::Result;
use base64::Engine;
use sldr_core::flavor::Flavor;
use sldr_core::slide::Slide;

use crate::render::{HtmlRenderer, RenderConfig};

/// One sample slide: logical name + markdown source.
pub struct SampleSlide {
    pub name: &'static str,
    pub source: &'static str,
}

/// The bundled sample deck, in render order.
pub const SAMPLE_SLIDES: &[SampleSlide] = &[
    SampleSlide {
        name: "01-cover",
        source: include_str!("../samples/sample/slides/01-cover.md"),
    },
    SampleSlide {
        name: "02-section",
        source: include_str!("../samples/sample/slides/02-section.md"),
    },
    SampleSlide {
        name: "03-default",
        source: include_str!("../samples/sample/slides/03-default.md"),
    },
    SampleSlide {
        name: "04-two-cols",
        source: include_str!("../samples/sample/slides/04-two-cols.md"),
    },
    SampleSlide {
        name: "05-quote",
        source: include_str!("../samples/sample/slides/05-quote.md"),
    },
    SampleSlide {
        name: "06-code",
        source: include_str!("../samples/sample/slides/06-code.md"),
    },
    SampleSlide {
        name: "07-image-right",
        source: include_str!("../samples/sample/slides/07-image-right.md"),
    },
    SampleSlide {
        name: "08-table",
        source: include_str!("../samples/sample/slides/08-table.md"),
    },
    SampleSlide {
        name: "11-image-grid",
        source: include_str!("../samples/sample/slides/11-image-grid.md"),
    },
    SampleSlide {
        name: "12-image-row",
        source: include_str!("../samples/sample/slides/12-image-row.md"),
    },
    SampleSlide {
        name: "13-image-portraits",
        source: include_str!("../samples/sample/slides/13-image-portraits.md"),
    },
    SampleSlide {
        name: "14-image-stack",
        source: include_str!("../samples/sample/slides/14-image-stack.md"),
    },
    SampleSlide {
        name: "09-conclusion",
        source: include_str!("../samples/sample/slides/09-conclusion.md"),
    },
    SampleSlide {
        name: "10-thank-you",
        source: include_str!("../samples/sample/slides/10-thank-you.md"),
    },
];

/// SVGs bundled alongside the sample slides. The first is the legacy
/// 4:5 portrait used by 07-image-right and several collage slides; the
/// landscape and square variants exist so collage layouts can be visually
/// validated against mixed aspect ratios without needing real photos.
pub const SAMPLE_IMAGE_SVG: &str = include_str!("../samples/sample/slides/sample-image.svg");
pub const SAMPLE_LANDSCAPE_SVG: &str =
    include_str!("../samples/sample/slides/sample-landscape.svg");
pub const SAMPLE_SQUARE_SVG: &str = include_str!("../samples/sample/slides/sample-square.svg");

/// Playlist TOML for the sample deck.
pub const SAMPLE_PLAYLIST_TOML: &str =
    include_str!("../samples/sample/playlists/sample.toml");

/// Render the bundled sample deck against the given flavor.
///
/// Returns a single self-contained HTML string with all CSS/JS inlined,
/// suitable for writing to a temp file and opening in a browser, or
/// serving from `sldr serve`.
///
/// `extra_flavors`: additional flavors to embed for the multi-flavor
/// runtime toggle (T key in the presenter). The first flavor in the
/// returned deck is the active one. Pass `&[]` for single-flavor mode.
pub fn render_sample(flavor: Flavor, extra_flavors: &[Flavor]) -> Result<String> {
    let config = RenderConfig {
        title: "sldr Sample Deck".to_string(),
        ..RenderConfig::default()
    };

    let mut renderer = HtmlRenderer::new(config).add_flavor(flavor);
    for extra in extra_flavors {
        renderer = renderer.add_flavor(extra.clone());
    }

    // Use a virtual base path so any relative image references in slides
    // resolve via the sample-asset shim below rather than the real FS.
    let base = PathBuf::from("sldr://sample/slides");

    let portrait_uri = data_uri(SAMPLE_IMAGE_SVG);
    let landscape_uri = data_uri(SAMPLE_LANDSCAPE_SVG);
    let square_uri = data_uri(SAMPLE_SQUARE_SVG);

    for sample in SAMPLE_SLIDES {
        let virtual_path = base.join(format!("{}.md", sample.name));
        // Replace the bundled image references with data URIs so the sample
        // deck renders without filesystem state. Order matters — replace
        // the longer, more-specific filenames first so they don't get
        // partially matched by the shorter `sample-image.svg`.
        let source = sample
            .source
            .replace("sample-landscape.svg", &landscape_uri)
            .replace("sample-square.svg", &square_uri)
            .replace("sample-image.svg", &portrait_uri);
        let slide = Slide::from_str(sample.name, virtual_path, &source);
        renderer.add_slide(&slide)?;
    }

    renderer.render()
}

/// Encode an SVG string as a `data:` URI for inline use.
fn data_uri(svg: &str) -> String {
    let encoded = base64::engine::general_purpose::STANDARD.encode(svg.as_bytes());
    format!("data:image/svg+xml;base64,{encoded}")
}

/// Look up a bundled sample slide by name (without extension).
#[must_use]
pub fn find_sample_slide(name: &str) -> Option<&'static SampleSlide> {
    SAMPLE_SLIDES.iter().find(|s| s.name == name)
}
