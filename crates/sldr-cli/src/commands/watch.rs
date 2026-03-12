//! Watch command - live-reload presentation development server
//!
//! Builds the presentation, serves it on a local port, watches for file
//! changes, and triggers browser reload via Server-Sent Events (SSE).

use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::Html;
use axum::routing::get;
use axum::Router;
use colored::Colorize;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use sldr_core::config::Config;
use sldr_core::flavor::Flavor;
use sldr_core::slide::SlideCollection;
use sldr_renderer::{HtmlRenderer, RenderConfig};
use tokio::sync::broadcast;
use tokio::sync::RwLock;

/// JavaScript snippet injected into the HTML for live reload via SSE
const LIVE_RELOAD_SCRIPT: &str = r"
<script>
(function() {
  var es = new EventSource('/__sldr_reload');
  es.onmessage = function(e) {
    if (e.data === 'reload') {
      window.location.reload();
    }
  };
  es.onerror = function() {
    // Reconnect on error (server restart)
    setTimeout(function() { window.location.reload(); }, 1000);
  };
})();
</script>
";

pub fn run(
    skeleton_name: &str,
    flavor: Option<String>,
    port: Option<u16>,
) -> Result<()> {
    let config = Config::load()?;

    println!(
        "{} presentation '{}' with live reload",
        "Watching".green().bold(),
        skeleton_name.cyan()
    );

    // Load skeleton
    let skeleton = super::build::load_skeleton(&config, skeleton_name)?;

    // Determine flavor
    let flavor_name = flavor
        .or(skeleton.flavor.clone())
        .unwrap_or_else(|| config.config.default_flavor.clone());
    let flavor = super::build::load_flavor(&config, &flavor_name)?;
    println!("  {} {}", "Flavor:".dimmed(), flavor.name.yellow());

    // Determine port
    let port = port.unwrap_or_else(|| {
        config
            .config
            .dev_port
            .parse::<u16>()
            .unwrap_or(3030)
    });

    // Initial build
    let slides = SlideCollection::load_from_dir(&config.slide_dir())?;
    let matcher = sldr_core::fuzzy::SldrMatcher::new(config.matching.clone());

    let mut resolved_slides = Vec::new();
    for slide_ref in &skeleton.slides {
        if let Some(slide) = super::build::resolve_with_interactive(&matcher, slide_ref, &slides)? {
            resolved_slides.push(slide);
        }
    }

    if resolved_slides.is_empty() {
        anyhow::bail!("No slides resolved. Add slides to your skeleton first.");
    }

    let title = skeleton
        .title
        .clone()
        .unwrap_or_else(|| skeleton.name.clone());

    let transition = skeleton
        .slidev_config
        .transition
        .clone()
        .unwrap_or_else(|| "fade".to_string());

    let aspect_ratio = skeleton
        .slidev_config
        .aspect_ratio
        .clone()
        .unwrap_or_else(|| "16/9".to_string());

    let render_config = RenderConfig {
        title: title.clone(),
        transition: transition.clone(),
        aspect_ratio: aspect_ratio.clone(),
        speaker_notes: true,
    };

    let html = build_html(&render_config, &flavor, &resolved_slides)?;
    let html = inject_live_reload(&html);

    // Shared state for the server
    let html_state = Arc::new(RwLock::new(html));
    let (reload_tx, _) = broadcast::channel::<()>(16);
    let reload_tx = Arc::new(reload_tx);

    // Start the tokio runtime for the HTTP server and file watcher
    let rt = tokio::runtime::Runtime::new()?;

    rt.block_on(async {
        // Check if port is available
        let addr = format!("127.0.0.1:{port}");
        let listener = tokio::net::TcpListener::bind(&addr)
            .await
            .with_context(|| format!("Port {port} is already in use"))?;

        // Build routes
        let html_for_route = Arc::clone(&html_state);
        let reload_tx_for_sse = Arc::clone(&reload_tx);

        let app = Router::new()
            .route(
                "/",
                get(move || {
                    let html = Arc::clone(&html_for_route);
                    async move {
                        let content = html.read().await;
                        Html(content.clone())
                    }
                }),
            )
            .route(
                "/__sldr_reload",
                get(move || {
                    let tx = Arc::clone(&reload_tx_for_sse);
                    async move {
                        let mut rx = tx.subscribe();
                        Sse::new(async_stream::stream! {
                            loop {
                                match rx.recv().await {
                                    Ok(()) => {
                                        yield Ok::<_, std::convert::Infallible>(Event::default().data("reload"));
                                    }
                                    Err(broadcast::error::RecvError::Lagged(_)) => {},
                                    Err(broadcast::error::RecvError::Closed) => break,
                                }
                            }
                        })
                        .keep_alive(KeepAlive::default())
                    }
                }),
            );

        println!(
            "\n  {} http://127.0.0.1:{}",
            "Serving at".green().bold(),
            port.to_string().cyan()
        );
        println!("  {} Watching for changes... (Ctrl+C to stop)", "i".blue());

        // Set up file watcher
        let slide_dir = config.slide_dir();
        let skeleton_dir = config.skeleton_dir();
        let flavor_dir = config.flavor_dir();

        let html_for_watcher = Arc::clone(&html_state);
        let reload_tx_for_watcher = Arc::clone(&reload_tx);

        // Clone what the watcher callback needs
        let watch_config = config.clone();
        let watch_skeleton_name = skeleton_name.to_string();
        let watch_flavor = flavor.clone();
        let watch_render_config = render_config.clone();

        let (watch_tx, mut watch_rx) = tokio::sync::mpsc::channel::<()>(1);

        let _watcher = spawn_file_watcher(
            &slide_dir,
            &skeleton_dir,
            &flavor_dir,
            watch_tx,
        )?;

        // Spawn the rebuild task
        tokio::spawn(async move {
            while watch_rx.recv().await.is_some() {
                // Debounce: drain any queued events
                tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
                while watch_rx.try_recv().is_ok() {}

                // Rebuild
                match rebuild_presentation(
                    &watch_config,
                    &watch_skeleton_name,
                    &watch_render_config,
                    &watch_flavor,
                ) {
                    Ok(new_html) => {
                        let new_html = inject_live_reload(&new_html);
                        *html_for_watcher.write().await = new_html;
                        let _ = reload_tx_for_watcher.send(());
                        println!(
                            "  {} Rebuilt and reloaded",
                            "~".green()
                        );
                    }
                    Err(err) => {
                        println!(
                            "  {} Rebuild failed: {}",
                            "!".red(),
                            err
                        );
                    }
                }
            }
        });

        // Open browser
        #[cfg(target_os = "linux")]
        {
            let _ = std::process::Command::new("xdg-open")
                .arg(format!("http://127.0.0.1:{port}"))
                .spawn();
        }
        #[cfg(target_os = "macos")]
        {
            let _ = std::process::Command::new("open")
                .arg(format!("http://127.0.0.1:{port}"))
                .spawn();
        }

        // Serve
        axum::serve(listener, app)
            .await
            .context("Server error")?;

        Ok::<(), anyhow::Error>(())
    })?;

    Ok(())
}

fn build_html(
    config: &RenderConfig,
    flavor: &Flavor,
    slides: &[sldr_core::slide::Slide],
) -> Result<String> {
    let mut renderer = HtmlRenderer::new(config.clone()).add_flavor(flavor.clone());
    renderer.add_slides(slides);
    renderer.render()
}

fn inject_live_reload(html: &str) -> String {
    // Inject before </body>
    if let Some(pos) = html.rfind("</body>") {
        let mut result = String::with_capacity(html.len() + LIVE_RELOAD_SCRIPT.len());
        result.push_str(&html[..pos]);
        result.push_str(LIVE_RELOAD_SCRIPT);
        result.push_str(&html[pos..]);
        result
    } else {
        // Fallback: append
        format!("{html}{LIVE_RELOAD_SCRIPT}")
    }
}

fn rebuild_presentation(
    config: &Config,
    skeleton_name: &str,
    render_config: &RenderConfig,
    flavor: &Flavor,
) -> Result<String> {
    let skeleton = sldr_core::presentation::Skeleton::load(
        &config
            .skeleton_dir()
            .join(format!("{skeleton_name}.toml")),
    )?;

    let slides = SlideCollection::load_from_dir(&config.slide_dir())?;
    let matcher = sldr_core::fuzzy::SldrMatcher::new(config.matching.clone());

    let mut resolved = Vec::new();
    for slide_ref in &skeleton.slides {
        if let sldr_core::fuzzy::ResolveResult::Found(result) =
            matcher.resolve(slide_ref, &slides.names())
        {
            if let Some(slide) = slides.find(&result.value) {
                resolved.push(slide.clone());
            }
        }
    }

    build_html(render_config, flavor, &resolved)
}

fn spawn_file_watcher(
    slide_dir: &Path,
    skeleton_dir: &Path,
    flavor_dir: &Path,
    tx: tokio::sync::mpsc::Sender<()>,
) -> Result<RecommendedWatcher> {
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(event) = res {
            if event.kind.is_modify() || event.kind.is_create() || event.kind.is_remove() {
                let _ = tx.blocking_send(());
            }
        }
    })?;

    if slide_dir.exists() {
        watcher.watch(slide_dir, RecursiveMode::Recursive)?;
    }
    if skeleton_dir.exists() {
        watcher.watch(skeleton_dir, RecursiveMode::Recursive)?;
    }
    if flavor_dir.exists() {
        watcher.watch(flavor_dir, RecursiveMode::Recursive)?;
    }

    Ok(watcher)
}
