//! sldr CLI - Markdown presentation manager powered by slidev
//!
//! This is a CLI application, so stdout/stderr output is expected and legitimate.
#![expect(clippy::print_stdout, reason = "CLI application uses stdout for user output")]

mod commands;

use clap::{Parser, Subcommand};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Parser)]
#[command(name = "sldr")]
#[command(author, version, about = "Markdown presentations powered by slidev")]
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

    /// Open a presentation in slidev
    Open {
        /// Name of the presentation to open
        presentation: String,

        /// Port for slidev server
        #[arg(short, long)]
        port: Option<String>,

        /// Rebuild presentation before opening
        #[arg(long)]
        rebuild: bool,
    },

    /// Preview a single slide quickly
    Preview {
        /// Slide to preview (name or path)
        slide: String,

        /// Port for slidev server
        #[arg(short, long)]
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

        Commands::Preview { slide, port } => commands::preview::run(&slide, port),

        Commands::List { what, long } => commands::list::run(&what, long),

        Commands::Search {
            query,
            tags,
            topic,
            long,
        } => commands::search::run(&query, tags, topic, long),

        Commands::New {
            name,
            template,
            dir,
        } => commands::new::run(&name, template, dir.as_ref()),

        Commands::Config { key, value, edit } => commands::config::run(key, value, edit),

        Commands::Init { global } => commands::init::run(global),
    }
}
