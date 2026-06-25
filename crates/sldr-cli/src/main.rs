//! sldr CLI - Markdown presentation manager
//!
//! This is a CLI application, so stdout/stderr output is expected and legitimate.
#![expect(
    clippy::print_stdout,
    reason = "CLI application uses stdout for user output"
)]

mod commands;
mod flavors;
mod reference;
mod scaffolds;

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
    /// Build a presentation from a playlist
    Build {
        /// Name of the playlist to build
        playlist: String,

        /// Flavor to apply (overrides playlist default)
        #[arg(short, long)]
        flavor: Option<String>,

        /// Language to build (slides with ::lang:xx:: blocks; overrides
        /// the playlist's default_lang)
        #[arg(short, long)]
        lang: Option<String>,

        /// Export to PDF after building
        #[arg(long)]
        pdf: bool,

        /// Export to PPTX after building
        #[arg(long)]
        pptx: bool,

        /// Output directory (overrides config)
        #[arg(short, long)]
        output: Option<String>,

        /// Inline all media as data URIs into one universal HTML file
        /// (default output is a presentation directory with media siblings
        /// in assets/ — the browser-native form)
        #[arg(long)]
        single_file: bool,
    },

    /// Add slides to a presentation playlist
    Add {
        /// Name of the presentation/playlist to modify
        presentation: String,

        /// Slides to add (comma-separated)
        slides: String,

        /// Insert at specific position (default: append)
        #[arg(short, long)]
        position: Option<usize>,
    },

    /// Remove slides from a presentation playlist
    #[command(name = "rm")]
    Remove {
        /// Name of the presentation/playlist to modify
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

    /// Export a presentation to PDF or PowerPoint
    Export {
        /// Playlist to export (omit only with --template, which is
        /// flavor-scoped)
        playlist: Option<String>,

        /// Flavor to apply
        #[arg(short, long)]
        flavor: Option<String>,

        /// Output file path (default: <output_dir>/<playlist>.<ext>)
        #[arg(short, long)]
        output: Option<String>,

        /// Language(s) to render (comma list). PDF/PPTX can't toggle language,
        /// so multiple languages export one file per language
        /// (deck.de.pdf, deck.en.pdf).
        #[arg(short, long)]
        lang: Option<String>,

        /// Export format: pdf or pptx
        #[arg(long, default_value = "pdf")]
        format: String,

        /// PPTX only: emit an editable *template* (theme + masters + layouts,
        /// no slides) for the flavor, instead of the deck
        #[arg(long)]
        template: bool,

        /// PPTX only: use the lossy screenshot-per-slide path instead of the
        /// native editable export (fallback for un-annotated layouts)
        #[arg(long)]
        flatten: bool,
    },

    /// Watch a presentation for changes and live-reload in browser
    Watch {
        /// Name of the playlist to watch
        playlist: String,

        /// Flavor to apply
        #[arg(short, long)]
        flavor: Option<String>,

        /// Port for the dev server (default: from config or 3030)
        #[arg(short, long)]
        port: Option<u16>,

        /// Address to bind (use 0.0.0.0 to expose on the network)
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
    },

    /// Preview a single slide quickly in the browser
    Preview {
        /// Slide to preview (name or path)
        slide: String,

        /// Port (unused, kept for backwards compat)
        #[arg(short, long, hide = true)]
        port: Option<String>,
    },

    /// Interactive visual flavor builder (opens in browser)
    #[command(name = "flavor", visible_aliases = ["flavor-build", "flavour"])]
    FlavorBuilder {
        /// Existing flavor to load as starting point
        #[arg(short, long)]
        name: Option<String>,

        /// Port for the builder server
        #[arg(short, long, default_value = "3031")]
        port: u16,

        /// Address to bind (use 0.0.0.0 to expose on the network)
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
    },

    /// Run a long-lived HTTP daemon that exposes sldr to external agents.
    ///
    /// External tools (web-to-slide pipelines, MCP servers, custom scripts)
    /// drive sldr over HTTP instead of forking the CLI per call. Boundary:
    /// sldr handles slide/playlist/asset CRUD + rendering. It does NOT fetch
    /// URLs, OCR, or summarize content — those are agent jobs.
    ///
    /// Endpoints are listed at GET / (the root URL).
    Serve {
        /// Port to listen on
        #[arg(short, long, default_value = "3032")]
        port: u16,

        /// Open the API landing page in the browser on start
        #[arg(long)]
        open: bool,

        /// Address to bind (use 0.0.0.0 to expose on the network)
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
    },

    /// Render the bundled sample deck against a flavor and open it.
    ///
    /// The sample deck is a canonical set of placeholder slides exercising
    /// every major layout — useful for evaluating a flavor visually without
    /// authoring real content. Same artifact also powers the flavor builder
    /// gallery and the agent slide catalog (GET /api/sample on `sldr serve`).
    Sample {
        /// Flavor to render the sample with
        #[arg(short, long, default_value = "default")]
        flavor: String,

        /// Write to a specific path instead of a temp file
        #[arg(short, long)]
        output: Option<std::path::PathBuf>,

        /// Don't open the browser, just print the output path
        #[arg(long)]
        no_open: bool,
    },

    /// List available slides, presentations, or flavors
    #[command(name = "ls")]
    List {
        /// What to list: slides, presentations, playlists, flavors, scaffolds, layouts
        #[arg(default_value = "slides")]
        what: String,

        /// Show detailed information
        #[arg(short, long)]
        long: bool,

        /// Output as JSON (for machine parsing)
        #[arg(long)]
        json: bool,
    },

    /// Print the raw source of a layout or flavor (what the name resolves to)
    ///
    /// `ls` lists names; `show` prints the actual source — the authored
    /// layout `.html` or flavor `.toml` — honoring the same resolution order
    /// as a build (user library/config dirs override built-ins). Source to
    /// stdout (pipeable), origin to stderr. For learning the format, copying
    /// a starting point, or seeing what a name really resolves to.
    Show {
        /// What to show: layout or flavor
        what: String,

        /// Name of the layout or flavor (fuzzy-matched)
        name: String,

        /// Output as JSON ({kind, name, origin, source})
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

    /// Pack a playlist with its slides, flavors, layouts, and media into
    /// a portable .sldr bundle (a plain zip)
    Bundle {
        /// Name of the playlist to bundle
        playlist: String,

        /// Flavor embed set (comma list; overrides playlist default)
        #[arg(short, long)]
        flavor: Option<String>,

        /// Language embed set (comma list)
        #[arg(short, long)]
        lang: Option<String>,

        /// Output file (defaults to <playlist>.sldr)
        #[arg(short, long)]
        output: Option<String>,
    },

    /// Create a new slide
    New {
        /// Name for the new slide
        name: String,

        /// Scaffold to use
        #[arg(short, long)]
        scaffold: Option<String>,

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

        /// Overwrite existing scaffolds and config with bundled versions
        #[arg(long)]
        force: bool,
    },

    /// Slide management commands
    Slides {
        #[command(subcommand)]
        command: SlidesCommands,
    },

    /// Playlist management commands
    Playlist {
        #[command(subcommand)]
        command: PlaylistCommands,
    },
}

#[derive(Subcommand)]
enum SlidesCommands {
    /// Create empty slides for all missing slides referenced in a playlist
    Derive {
        /// Name of the playlist to derive slides from
        playlist: String,

        /// Scaffold to use for new slides
        #[arg(short, long)]
        scaffold: Option<String>,

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
enum PlaylistCommands {
    /// Create a playlist from JSON input or from a slide directory
    Create {
        /// Read JSON from file instead of stdin
        #[arg(short, long, conflicts_with = "from_dir")]
        file: Option<String>,

        /// Auto-generate playlist from all slides in a directory
        #[arg(long)]
        from_dir: Option<String>,

        /// Name for the playlist (required with --from-dir)
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

        /// Overwrite existing playlist
        #[arg(long)]
        force: bool,
    },

    /// Validate a playlist - check all referenced slides exist
    Validate {
        /// Name of the playlist to validate
        playlist: String,

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
            playlist,
            flavor,
            lang,
            pdf,
            pptx,
            output,
            single_file,
        } => commands::build::run(&playlist, flavor, lang, pdf, pptx, output, single_file),

        Commands::Bundle {
            playlist,
            flavor,
            lang,
            output,
        } => commands::bundle::create(&playlist, flavor, lang, output),

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

        Commands::Export {
            playlist,
            flavor,
            output,
            lang,
            format,
            template,
            flatten,
        } => commands::export::run(
            playlist.as_deref(),
            flavor,
            output,
            lang,
            &format,
            template,
            flatten,
        ),

        Commands::Watch {
            playlist,
            flavor,
            port,
            host,
        } => commands::watch::run(&playlist, flavor, port, &host),

        Commands::Preview { slide, port } => commands::preview::run(&slide, port),

        Commands::FlavorBuilder { name, port, host } => commands::flavor_builder::run(name, port, &host),
        Commands::Serve { port, open, host } => commands::serve::run(port, open, &host),
        Commands::Sample {
            flavor,
            output,
            no_open,
        } => commands::sample::run(&flavor, output, no_open),

        Commands::List { what, long, json } => commands::list::run(&what, long, json),

        Commands::Show { what, name, json } => commands::show::run(&what, &name, json),

        Commands::Search {
            query,
            tags,
            topic,
            long,
            json,
        } => commands::search::run(&query, tags, topic, long, json),

        Commands::New {
            name,
            scaffold,
            dir,
        } => commands::new::run(&name, scaffold, dir.as_ref()),

        Commands::Config { key, value, edit } => commands::config::run(key, value, edit),

        Commands::Init { global, force } => commands::init::run(global, force),

        Commands::Slides { command } => match command {
            SlidesCommands::Derive {
                playlist,
                scaffold,
                dry_run,
            } => commands::slides::derive(&playlist, scaffold.as_deref(), dry_run),

            SlidesCommands::Create {
                file,
                dry_run,
                json,
                force,
            } => commands::slides::create(file.as_deref(), dry_run, json, force),
        },

        Commands::Playlist { command } => match command {
            PlaylistCommands::Create {
                file,
                from_dir,
                name,
                save_slides,
                dry_run,
                json,
                force,
            } => {
                if let Some(dir) = from_dir {
                    commands::playlist::create_from_dir(&dir, name.as_deref(), dry_run, json, force)
                } else {
                    commands::playlist::create(file.as_deref(), dry_run, json, force, save_slides)
                }
            }

            PlaylistCommands::Validate { playlist, json } => {
                commands::playlist::validate(&playlist, json)
            }
        },
    }
}
