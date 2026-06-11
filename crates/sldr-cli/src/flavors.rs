//! Embedded example flavors
//!
//! These flavors ship bundled in the binary and get installed into the user's
//! `flavor_dir` on `sldr init` (similar to bundled scaffolds in `scaffolds.rs`).
//! Once installed they behave like any other flavor — users are free to edit,
//! rename, or delete them.

/// One file inside a bundled flavor directory.
pub struct FlavorFile {
    pub name: &'static str,
    pub content: &'static str,
}

/// A bundled flavor: directory name + list of files to drop into it.
pub struct BundledFlavor {
    pub slug: &'static str,
    pub files: &'static [FlavorFile],
}

/// All bundled example flavors.
pub const FLAVORS: &[BundledFlavor] = &[
    // --- Shipped set v2 (trx: 10 unique flavors). --------------------------
    BundledFlavor {
        slug: "aurora",
        files: &[FlavorFile {
            name: "flavor.toml",
            content: include_str!("../../../examples/flavors/aurora/flavor.toml"),
        }],
    },
    BundledFlavor {
        slug: "letterpress",
        files: &[FlavorFile {
            name: "flavor.toml",
            content: include_str!("../../../examples/flavors/letterpress/flavor.toml"),
        }],
    },
    BundledFlavor {
        slug: "neon-noir",
        files: &[FlavorFile {
            name: "flavor.toml",
            content: include_str!("../../../examples/flavors/neon-noir/flavor.toml"),
        }],
    },
    BundledFlavor {
        slug: "terracotta",
        files: &[FlavorFile {
            name: "flavor.toml",
            content: include_str!("../../../examples/flavors/terracotta/flavor.toml"),
        }],
    },
    BundledFlavor {
        slug: "blueprint",
        files: &[FlavorFile {
            name: "flavor.toml",
            content: include_str!("../../../examples/flavors/blueprint/flavor.toml"),
        }],
    },
    BundledFlavor {
        slug: "sakura",
        files: &[FlavorFile {
            name: "flavor.toml",
            content: include_str!("../../../examples/flavors/sakura/flavor.toml"),
        }],
    },
    BundledFlavor {
        slug: "midnight-gold",
        files: &[FlavorFile {
            name: "flavor.toml",
            content: include_str!("../../../examples/flavors/midnight-gold/flavor.toml"),
        }],
    },
    BundledFlavor {
        slug: "acid-lab",
        files: &[FlavorFile {
            name: "flavor.toml",
            content: include_str!("../../../examples/flavors/acid-lab/flavor.toml"),
        }],
    },
    BundledFlavor {
        slug: "fjord",
        files: &[FlavorFile {
            name: "flavor.toml",
            content: include_str!("../../../examples/flavors/fjord/flavor.toml"),
        }],
    },
    BundledFlavor {
        slug: "kraft",
        files: &[FlavorFile {
            name: "flavor.toml",
            content: include_str!("../../../examples/flavors/kraft/flavor.toml"),
        }],
    },
    // --- Seed flavors (token-only, exercise the schema). -------------------
    BundledFlavor {
        slug: "editorial-serif",
        files: &[
            FlavorFile {
                name: "flavor.toml",
                content: include_str!("../../../examples/flavors/editorial-serif/flavor.toml"),
            },
            FlavorFile {
                name: "flavor.css",
                content: include_str!("../../../examples/flavors/editorial-serif/flavor.css"),
            },
        ],
    },
    BundledFlavor {
        slug: "minimal-light",
        files: &[FlavorFile {
            name: "flavor.toml",
            content: include_str!("../../../examples/flavors/minimal-light/flavor.toml"),
        }],
    },
    BundledFlavor {
        slug: "technical-dark",
        files: &[FlavorFile {
            name: "flavor.toml",
            content: include_str!("../../../examples/flavors/technical-dark/flavor.toml"),
        }],
    },
    BundledFlavor {
        slug: "brutalist-mono",
        files: &[FlavorFile {
            name: "flavor.toml",
            content: include_str!("../../../examples/flavors/brutalist-mono/flavor.toml"),
        }],
    },
    BundledFlavor {
        slug: "swiss-grid",
        files: &[FlavorFile {
            name: "flavor.toml",
            content: include_str!("../../../examples/flavors/swiss-grid/flavor.toml"),
        }],
    },
    // --- BHT-derived flavors (trx-s27h). -----------------------------------
    BundledFlavor {
        slug: "neo-grid-bold",
        files: &[FlavorFile {
            name: "flavor.toml",
            content: include_str!("../../../examples/flavors/neo-grid-bold/flavor.toml"),
        }],
    },
    BundledFlavor {
        slug: "monochrome",
        files: &[FlavorFile {
            name: "flavor.toml",
            content: include_str!("../../../examples/flavors/monochrome/flavor.toml"),
        }],
    },
    BundledFlavor {
        slug: "vellum",
        files: &[FlavorFile {
            name: "flavor.toml",
            content: include_str!("../../../examples/flavors/vellum/flavor.toml"),
        }],
    },
    BundledFlavor {
        slug: "signal",
        files: &[FlavorFile {
            name: "flavor.toml",
            content: include_str!("../../../examples/flavors/signal/flavor.toml"),
        }],
    },
    BundledFlavor {
        slug: "coral",
        files: &[FlavorFile {
            name: "flavor.toml",
            content: include_str!("../../../examples/flavors/coral/flavor.toml"),
        }],
    },
    BundledFlavor {
        slug: "raw-grid",
        files: &[FlavorFile {
            name: "flavor.toml",
            content: include_str!("../../../examples/flavors/raw-grid/flavor.toml"),
        }],
    },
];

/// Install all bundled flavors into `flavor_dir`. Each flavor lives in its
/// own subdirectory. Existing files are kept unless `overwrite` is true.
///
/// Returns the number of flavor *directories* that received at least one
/// fresh write.
pub fn install_flavors(flavor_dir: &std::path::Path, overwrite: bool) -> std::io::Result<usize> {
    std::fs::create_dir_all(flavor_dir)?;

    let mut installed = 0;
    for flavor in FLAVORS {
        let dir = flavor_dir.join(flavor.slug);
        std::fs::create_dir_all(&dir)?;

        let mut wrote_any = false;
        for file in flavor.files {
            let path = dir.join(file.name);
            if !path.exists() || overwrite {
                std::fs::write(&path, file.content)?;
                wrote_any = true;
            }
        }
        if wrote_any {
            installed += 1;
        }
    }

    Ok(installed)
}
