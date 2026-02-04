//! Embedded slide templates
//!
//! These templates are compiled into the binary and can be installed via `sldr init`.

/// Template entry containing name and content
pub struct Template {
    pub name: &'static str,
    pub content: &'static str,
}

/// All bundled templates
pub const TEMPLATES: &[Template] = &[
    // Cover/Title slides
    Template {
        name: "title.md",
        content: include_str!("../../../examples/templates/title.md"),
    },
    Template {
        name: "cover.md",
        content: include_str!("../../../examples/templates/cover.md"),
    },
    Template {
        name: "intro.md",
        content: include_str!("../../../examples/templates/intro.md"),
    },
    Template {
        name: "research-title.md",
        content: include_str!("../../../examples/templates/research-title.md"),
    },
    // Section/Structure
    Template {
        name: "section.md",
        content: include_str!("../../../examples/templates/section.md"),
    },
    Template {
        name: "default.md",
        content: include_str!("../../../examples/templates/default.md"),
    },
    Template {
        name: "basic.md",
        content: include_str!("../../../examples/templates/basic.md"),
    },
    Template {
        name: "bullets.md",
        content: include_str!("../../../examples/templates/bullets.md"),
    },
    // Layout variations
    Template {
        name: "two-cols.md",
        content: include_str!("../../../examples/templates/two-cols.md"),
    },
    Template {
        name: "two-cols-header.md",
        content: include_str!("../../../examples/templates/two-cols-header.md"),
    },
    Template {
        name: "comparison.md",
        content: include_str!("../../../examples/templates/comparison.md"),
    },
    // Images/Visuals
    Template {
        name: "image.md",
        content: include_str!("../../../examples/templates/image.md"),
    },
    Template {
        name: "image-left.md",
        content: include_str!("../../../examples/templates/image-left.md"),
    },
    Template {
        name: "image-right.md",
        content: include_str!("../../../examples/templates/image-right.md"),
    },
    Template {
        name: "figure.md",
        content: include_str!("../../../examples/templates/figure.md"),
    },
    // Code/Technical
    Template {
        name: "code.md",
        content: include_str!("../../../examples/templates/code.md"),
    },
    Template {
        name: "code-comparison.md",
        content: include_str!("../../../examples/templates/code-comparison.md"),
    },
    Template {
        name: "terminal.md",
        content: include_str!("../../../examples/templates/terminal.md"),
    },
    Template {
        name: "architecture.md",
        content: include_str!("../../../examples/templates/architecture.md"),
    },
    // Data/Charts
    Template {
        name: "chart.md",
        content: include_str!("../../../examples/templates/chart.md"),
    },
    Template {
        name: "table.md",
        content: include_str!("../../../examples/templates/table.md"),
    },
    Template {
        name: "results.md",
        content: include_str!("../../../examples/templates/results.md"),
    },
    // Academic/Research
    Template {
        name: "methodology.md",
        content: include_str!("../../../examples/templates/methodology.md"),
    },
    Template {
        name: "discussion.md",
        content: include_str!("../../../examples/templates/discussion.md"),
    },
    Template {
        name: "references.md",
        content: include_str!("../../../examples/templates/references.md"),
    },
    Template {
        name: "qna.md",
        content: include_str!("../../../examples/templates/qna.md"),
    },
    // Closing
    Template {
        name: "quote.md",
        content: include_str!("../../../examples/templates/quote.md"),
    },
    Template {
        name: "conclusion.md",
        content: include_str!("../../../examples/templates/conclusion.md"),
    },
    Template {
        name: "thank-you.md",
        content: include_str!("../../../examples/templates/thank-you.md"),
    },
    Template {
        name: "end.md",
        content: include_str!("../../../examples/templates/end.md"),
    },
];

/// Install all bundled templates to the given directory
pub fn install_templates(
    template_dir: &std::path::Path,
    overwrite: bool,
) -> std::io::Result<usize> {
    std::fs::create_dir_all(template_dir)?;

    let mut installed = 0;
    for template in TEMPLATES {
        let path = template_dir.join(template.name);
        if !path.exists() || overwrite {
            std::fs::write(&path, template.content)?;
            installed += 1;
        }
    }

    Ok(installed)
}
