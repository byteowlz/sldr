//! Embedded reference deck — the sldr user guide.
//!
//! Twenty-six slides, one per built-in layout, plus the playlist that
//! wires them together. Compiled into the binary and installed into the
//! library on `sldr init` so every installation ships its own manual:
//! `sldr build reference` works out of the box, and rebuilding it with
//! `--flavor` is the canonical way to audition a flavor.

/// One file of the reference deck (slide markdown or SVG asset).
pub struct ReferenceFile {
    pub name: &'static str,
    pub content: &'static str,
}

/// Slides and assets, installed to `<library>/slides/reference/`.
pub const REFERENCE_SLIDES: &[ReferenceFile] = &[
    ReferenceFile {
        name: "01-cover.md",
        content: include_str!("../../../examples/reference/slides/01-cover.md"),
    },
    ReferenceFile {
        name: "02-intro.md",
        content: include_str!("../../../examples/reference/slides/02-intro.md"),
    },
    ReferenceFile {
        name: "03-agenda.md",
        content: include_str!("../../../examples/reference/slides/03-agenda.md"),
    },
    ReferenceFile {
        name: "04-section-basics.md",
        content: include_str!("../../../examples/reference/slides/04-section-basics.md"),
    },
    ReferenceFile {
        name: "05-slides-are-files.md",
        content: include_str!("../../../examples/reference/slides/05-slides-are-files.md"),
    },
    ReferenceFile {
        name: "06-statement.md",
        content: include_str!("../../../examples/reference/slides/06-statement.md"),
    },
    ReferenceFile {
        name: "07-pillars.md",
        content: include_str!("../../../examples/reference/slides/07-pillars.md"),
    },
    ReferenceFile {
        name: "08-cli.md",
        content: include_str!("../../../examples/reference/slides/08-cli.md"),
    },
    ReferenceFile {
        name: "09-workflow.md",
        content: include_str!("../../../examples/reference/slides/09-workflow.md"),
    },
    ReferenceFile {
        name: "10-two-cols.md",
        content: include_str!("../../../examples/reference/slides/10-two-cols.md"),
    },
    ReferenceFile {
        name: "11-two-cols-header.md",
        content: include_str!("../../../examples/reference/slides/11-two-cols-header.md"),
    },
    ReferenceFile {
        name: "12-hero-stat.md",
        content: include_str!("../../../examples/reference/slides/12-hero-stat.md"),
    },
    ReferenceFile {
        name: "13-versus.md",
        content: include_str!("../../../examples/reference/slides/13-versus.md"),
    },
    ReferenceFile {
        name: "14-flavors.md",
        content: include_str!("../../../examples/reference/slides/14-flavors.md"),
    },
    ReferenceFile {
        name: "15-quote.md",
        content: include_str!("../../../examples/reference/slides/15-quote.md"),
    },
    ReferenceFile {
        name: "16-shortcuts.md",
        content: include_str!("../../../examples/reference/slides/16-shortcuts.md"),
    },
    ReferenceFile {
        name: "17-image-right.md",
        content: include_str!("../../../examples/reference/slides/17-image-right.md"),
    },
    ReferenceFile {
        name: "18-image-left.md",
        content: include_str!("../../../examples/reference/slides/18-image-left.md"),
    },
    ReferenceFile {
        name: "19-feature-image.md",
        content: include_str!("../../../examples/reference/slides/19-feature-image.md"),
    },
    ReferenceFile {
        name: "20-full-bleed.md",
        content: include_str!("../../../examples/reference/slides/20-full-bleed.md"),
    },
    ReferenceFile {
        name: "21-image-grid.md",
        content: include_str!("../../../examples/reference/slides/21-image-grid.md"),
    },
    ReferenceFile {
        name: "22-image-row.md",
        content: include_str!("../../../examples/reference/slides/22-image-row.md"),
    },
    ReferenceFile {
        name: "23-portraits.md",
        content: include_str!("../../../examples/reference/slides/23-portraits.md"),
    },
    ReferenceFile {
        name: "24-image-stack.md",
        content: include_str!("../../../examples/reference/slides/24-image-stack.md"),
    },
    ReferenceFile {
        name: "25-end.md",
        content: include_str!("../../../examples/reference/slides/25-end.md"),
    },
    ReferenceFile {
        name: "26-contact.md",
        content: include_str!("../../../examples/reference/slides/26-contact.md"),
    },
    ReferenceFile {
        name: "ref-landscape.svg",
        content: include_str!("../../../examples/reference/slides/ref-landscape.svg"),
    },
    ReferenceFile {
        name: "ref-portrait.svg",
        content: include_str!("../../../examples/reference/slides/ref-portrait.svg"),
    },
    ReferenceFile {
        name: "ref-square.svg",
        content: include_str!("../../../examples/reference/slides/ref-square.svg"),
    },
];

/// The playlist, installed to `<playlist_dir>/reference.toml`.
pub const REFERENCE_PLAYLIST: &str = include_str!("../../../examples/reference/reference.toml");
