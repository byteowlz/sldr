//! sldr CLI - Markdown presentation manager
//!
//! This is a CLI application, so stdout/stderr output is expected and legitimate.
#![expect(
    clippy::print_stdout,
    reason = "CLI application uses stdout for user output"
)]

mod commands;
mod templates;

use clap::{Parser, Subcommand};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Parser)]
#[command(name = "sldr")]
#[command(author, version, about = "Markdown presentations - self-contained HTML output")]
#[command(propagate_version = true)]
struct Cli {
    /// Enable debug logging
    #[arg(long, global = true)]
    debug: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Build a presentation from a skeleton
    Build {
        /// Name of the skeleton to build
        skeleton: String,

        /// Flavor to apply (overrides skeleton default)
        #[arg(short, long)]
        flavor: Option<String>,

        /// Export to PDF after building
        #[arg(long)]
        pdf: bool,

        /// Export to PPTX after building
        #[arg(long)]
        pptx: bool,

        /// Output directory (overrides config)
        #[arg(short, long)]
        output: Option<String>,
    },

    /// Add slides to a presentation skeleton
    Add {
        /// Name of the presentation/skeleton to modify
        presentation: String,

        /// Slides to add (comma-separated)
        slides: String,

        /// Insert at specific position (default: append)
        #[arg(short, long)]
        position: Option<usize>,
    },

    /// Remove slides from a presentation skeleton
    #[command(name = "rm")]
    Remove {
        /// Name of the presentation/skeleton to modify
        presentation: String,

        /// Slides to remove (comma-separated, or use --interactive)
        slides: Option<String>,

        /// Interactively select slides to remove
        #[arg(short, long)]
        interactive: bool,
    },

    /// Open a built presentation in the browser
    Open {
        /// Name of the presentation to open
        presentation: String,

        /// Port (unused, kept for backwards compat)
        #[arg(short, long, hide = true)]
        port: Option<String>,

        /// Rebuild presentation before opening
        #[arg(long)]
        rebuild: bool,
    },

    /// Watch a presentation for changes and live-reload in browser
    Watch {
        /// Name of the skeleton to watch
        skeleton: String,

        /// Flavor to apply
        #[arg(short, long)]
        flavor: Option<String>,

        /// Port for the dev server (default: from config or 3030)
        #[arg(short, long)]
        port: Option<u16>,
    },

    /// Preview a single slide quickly in the browser
    Preview {
        /// Slide to preview (name or path)
        slide: String,

        /// Port (unused, kept for backwards compat)
        #[arg(short, long, hide = true)]
        port: Option<String>,
    },

    /// List available slides, presentations, or flavors
    #[command(name = "ls")]
    List {
        /// What to list: slides, presentations, skeletons, flavors
        #[arg(default_value = "slides")]
        what: String,

        /// Show detailed information
        #[arg(short, long)]
        long: bool,

        /// Output as JSON (for machine parsing)
        #[arg(long)]
        json: bool,
    },

    /// Search slides by content, tags, or metadata
    Search {
        /// Search query
        query: String,

        /// Filter by tags (comma-separated)
        #[arg(short, long)]
        tags: Option<String>,

        /// Filter by topic
        #[arg(long)]
        topic: Option<String>,

        /// Show detailed information
        #[arg(short, long)]
        long: bool,

        /// Output as JSON (for machine parsing)
        #[arg(long)]
        json: bool,
    },

    /// Create a new slide
    New {
        /// Name for the new slide
        name: String,

        /// Template to use
        #[arg(short, long)]
        template: Option<String>,

        /// Subdirectory within slides folder
        #[arg(short, long)]
        dir: Option<String>,
    },

    /// Show or edit configuration
    Config {
        /// Configuration key to show/set
        key: Option<String>,

        /// Value to set
        value: Option<String>,

        /// Open config file in editor
        #[arg(short, long)]
        edit: bool,
    },

    /// Initialize sldr directories and configuration
    Init {
        /// Initialize globally (~/.config/sldr)
        #[arg(long)]
        global: bool,
    },

    /// Slide management commands
    Slides {
        #[command(subcommand)]
        command: SlidesCommands,
    },

    /// Skeleton management commands
    Skeleton {
        #[command(subcommand)]
        command: SkeletonCommands,
    },
}

#[derive(Subcommand)]
enum SlidesCommands {
    /// Create empty slides for all missing slides referenced in a skeleton
    Derive {
        /// Name of the skeleton to derive slides from
        skeleton: String,

        /// Template to use for new slides
        #[arg(short, long)]
        template: Option<String>,

        /// Dry run - show what would be created without creating files
        #[arg(long)]
        dry_run: bool,
    },

    /// Create slides from JSON input (agent-friendly batch creation)
    Create {
        /// Read JSON from file instead of stdin
        #[arg(short, long)]
        file: Option<String>,

        /// Dry run - show what would be created without creating files
        #[arg(long)]
        dry_run: bool,

        /// Output as JSON (for machine parsing)
        #[arg(long)]
        json: bool,

        /// Overwrite existing slides
        #[arg(long)]
        force: bool,
    },
}

#[derive(Subcommand)]
enum SkeletonCommands {
    /// Create a skeleton from JSON input or from a slide directory
    Create {
        /// Read JSON from file instead of stdin
        #[arg(short, long, conflicts_with = "from_dir")]
        file: Option<String>,

        /// Auto-generate skeleton from all slides in a directory
        #[arg(long)]
        from_dir: Option<String>,

        /// Name for the skeleton (required with --from-dir)
        #[arg(short, long)]
        name: Option<String>,

        /// Also save individual slide markdown files from JSON input
        #[arg(long)]
        save_slides: bool,

        /// Dry run - show what would be created without creating files
        #[arg(long)]
        dry_run: bool,

        /// Output as JSON (for machine parsing)
        #[arg(long)]
        json: bool,

        /// Overwrite existing skeleton
        #[arg(long)]
        force: bool,
    },

    /// Validate a skeleton - check all referenced slides exist
    Validate {
        /// Name of the skeleton to validate
        skeleton: String,

        /// Output as JSON (for machine parsing)
        #[arg(long)]
        json: bool,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    let log_level = if cli.debug { "debug" } else { "warn" };
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| format!("sldr={log_level}")),
        ))
        .with(tracing_subscriber::fmt::layer().without_time())
        .init();

    match cli.command {
        Commands::Build {
            skeleton,
            flavor,
            pdf,
            pptx,
            output,
        } => commands::build::run(&skeleton, flavor, pdf, pptx, output),

        Commands::Add {
            presentation,
            slides,
            position,
        } => commands::add::run(&presentation, &slides, position),

        Commands::Remove {
            presentation,
            slides,
            interactive,
        } => commands::rm::run(&presentation, slides.as_ref(), interactive),

        Commands::Open {
            presentation,
            port,
            rebuild,
        } => commands::open::run(&presentation, port, rebuild),

        Commands::Watch {
            skeleton,
            flavor,
            port,
        } => commands::watch::run(&skeleton, flavor, port),

        Commands::Preview { slide, port } => commands::preview::run(&slide, port),

        Commands::List { what, long, json } => commands::list::run(&what, long, json),

        Commands::Search {
            query,
            tags,
            topic,
            long,
            json,
        } => commands::search::run(&query, tags, topic, long, json),

        Commands::New {
            name,
            template,
            dir,
        } => commands::new::run(&name, template, dir.as_ref()),

        Commands::Config { key, value, edit } => commands::config::run(key, value, edit),

        Commands::Init { global } => commands::init::run(global),

        Commands::Slides { command } => match command {
            SlidesCommands::Derive {
                skeleton,
                template,
                dry_run,
            } => commands::slides::derive(&skeleton, template.as_deref(), dry_run),

            SlidesCommands::Create {
                file,
                dry_run,
                json,
                force,
            } => commands::slides::create(file.as_deref(), dry_run, json, force),
        },

        Commands::Skeleton { command } => match command {
            SkeletonCommands::Create {
                file,
                from_dir,
                name,
                save_slides,
                dry_run,
                json,
                force,
            } => {
                if let Some(dir) = from_dir {
                    commands::skeleton::create_from_dir(&dir, name.as_deref(), dry_run, json, force)
                } else {
                    commands::skeleton::create(file.as_deref(), dry_run, json, force, save_slides)
                }
            }

            SkeletonCommands::Validate { skeleton, json } => {
                commands::skeleton::validate(&skeleton, json)
            }
        },
    }
}
