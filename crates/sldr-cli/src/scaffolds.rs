//! Embedded slide scaffolds
//!
//! These scaffolds are compiled into the binary and can be installed via `sldr init`.

/// Scaffold entry containing name and content
pub struct Scaffold {
    pub name: &'static str,
    pub content: &'static str,
}

/// All bundled scaffolds
pub const SCAFFOLDS: &[Scaffold] = &[
    // Cover/Title slides
    Scaffold {
        name: "title.md",
        content: include_str!("../../../examples/scaffolds/title.md"),
    },
    Scaffold {
        name: "cover.md",
        content: include_str!("../../../examples/scaffolds/cover.md"),
    },
    Scaffold {
        name: "intro.md",
        content: include_str!("../../../examples/scaffolds/intro.md"),
    },
    Scaffold {
        name: "research-title.md",
        content: include_str!("../../../examples/scaffolds/research-title.md"),
    },
    // Section/Structure
    Scaffold {
        name: "section.md",
        content: include_str!("../../../examples/scaffolds/section.md"),
    },
    Scaffold {
        name: "default.md",
        content: include_str!("../../../examples/scaffolds/default.md"),
    },
    Scaffold {
        name: "basic.md",
        content: include_str!("../../../examples/scaffolds/basic.md"),
    },
    Scaffold {
        name: "bullets.md",
        content: include_str!("../../../examples/scaffolds/bullets.md"),
    },
    // Layout variations
    Scaffold {
        name: "two-cols.md",
        content: include_str!("../../../examples/scaffolds/two-cols.md"),
    },
    Scaffold {
        name: "two-cols-header.md",
        content: include_str!("../../../examples/scaffolds/two-cols-header.md"),
    },
    Scaffold {
        name: "comparison.md",
        content: include_str!("../../../examples/scaffolds/comparison.md"),
    },
    // Images/Visuals
    Scaffold {
        name: "image.md",
        content: include_str!("../../../examples/scaffolds/image.md"),
    },
    Scaffold {
        name: "image-left.md",
        content: include_str!("../../../examples/scaffolds/image-left.md"),
    },
    Scaffold {
        name: "image-right.md",
        content: include_str!("../../../examples/scaffolds/image-right.md"),
    },
    Scaffold {
        name: "figure.md",
        content: include_str!("../../../examples/scaffolds/figure.md"),
    },
    Scaffold {
        name: "translated-figure.md",
        content: include_str!("../../../examples/scaffolds/translated-figure.md"),
    },
    // Code/Technical
    Scaffold {
        name: "code.md",
        content: include_str!("../../../examples/scaffolds/code.md"),
    },
    Scaffold {
        name: "code-comparison.md",
        content: include_str!("../../../examples/scaffolds/code-comparison.md"),
    },
    Scaffold {
        name: "terminal.md",
        content: include_str!("../../../examples/scaffolds/terminal.md"),
    },
    Scaffold {
        name: "architecture.md",
        content: include_str!("../../../examples/scaffolds/architecture.md"),
    },
    // Data/Charts
    Scaffold {
        name: "chart.md",
        content: include_str!("../../../examples/scaffolds/chart.md"),
    },
    Scaffold {
        name: "table.md",
        content: include_str!("../../../examples/scaffolds/table.md"),
    },
    Scaffold {
        name: "results.md",
        content: include_str!("../../../examples/scaffolds/results.md"),
    },
    // Academic/Research
    Scaffold {
        name: "methodology.md",
        content: include_str!("../../../examples/scaffolds/methodology.md"),
    },
    Scaffold {
        name: "discussion.md",
        content: include_str!("../../../examples/scaffolds/discussion.md"),
    },
    Scaffold {
        name: "references.md",
        content: include_str!("../../../examples/scaffolds/references.md"),
    },
    Scaffold {
        name: "qna.md",
        content: include_str!("../../../examples/scaffolds/qna.md"),
    },
    // Closing
    Scaffold {
        name: "quote.md",
        content: include_str!("../../../examples/scaffolds/quote.md"),
    },
    Scaffold {
        name: "conclusion.md",
        content: include_str!("../../../examples/scaffolds/conclusion.md"),
    },
    Scaffold {
        name: "thank-you.md",
        content: include_str!("../../../examples/scaffolds/thank-you.md"),
    },
    Scaffold {
        name: "end.md",
        content: include_str!("../../../examples/scaffolds/end.md"),
    },
];

/// Install all bundled scaffolds to the given directory
pub fn install_scaffolds(
    scaffold_dir: &std::path::Path,
    overwrite: bool,
) -> std::io::Result<usize> {
    std::fs::create_dir_all(scaffold_dir)?;

    let mut installed = 0;
    for scaffold in SCAFFOLDS {
        let path = scaffold_dir.join(scaffold.name);
        if !path.exists() || overwrite {
            std::fs::write(&path, scaffold.content)?;
            installed += 1;
        }
    }

    Ok(installed)
}
