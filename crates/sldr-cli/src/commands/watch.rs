//! Watch command - live-reload presentation development server
//!
//! Builds the presentation, serves it on a local port, watches for file
//! changes, and triggers browser reload via Server-Sent Events (SSE).

use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use axum::extract::{Query, State};
use axum::http::{header, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use colored::Colorize;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use serde::Deserialize;
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
      if (window.__sldrSourceDirty) {
        window.dispatchEvent(new CustomEvent('sldr:remote-change'));
      } else {
        window.location.reload();
      }
    }
  };
  es.onerror = function() {
    // Reconnect on error (server restart), but never discard an open edit.
    if (!window.__sldrSourceDirty) {
      setTimeout(function() { window.location.reload(); }, 1000);
    }
  };
})();
</script>
";

pub fn run(
    playlist_name: &str,
    flavor: Option<String>,
    port: Option<u16>,
    host: &str,
) -> Result<()> {
    let config = Config::load()?;

    println!(
        "{} presentation '{}' with live reload",
        "Watching".green().bold(),
        playlist_name.cyan()
    );

    // Load playlist
    let playlist = super::build::load_playlist(&config, playlist_name)?;

    // Flavor axis: comma list = embed set, first active (same as build).
    let flavor_arg = flavor
        .or(playlist.flavor.clone())
        .unwrap_or_else(|| config.config.default_flavor.clone());
    let flavor_names: Vec<String> = flavor_arg
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect();
    let mut flavors = Vec::new();
    for name in &flavor_names {
        flavors.push(super::build::load_flavor(&config, name)?);
    }
    println!(
        "  {} {}",
        "Flavor:".dimmed(),
        flavors
            .iter()
            .map(|f| f.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
            .yellow()
    );

    // Determine port
    let port = port.unwrap_or_else(|| config.config.dev_port.parse::<u16>().unwrap_or(3030));

    // Initial build
    let slides = SlideCollection::load_from_dir(&config.slide_dir())?;
    let matcher = sldr_core::fuzzy::SldrMatcher::new(config.matching.clone());

    let mut resolved_slides = Vec::new();
    for slide_ref in &playlist.slides {
        if let Some(slide) = super::build::resolve_with_interactive(&matcher, slide_ref, &slides)? {
            resolved_slides.push(slide);
        }
    }

    if resolved_slides.is_empty() {
        anyhow::bail!("No slides resolved. Add slides to your playlist first.");
    }

    let title = playlist
        .title
        .clone()
        .unwrap_or_else(|| playlist.name.clone());

    let transition = playlist
        .render
        .transition
        .clone()
        .unwrap_or_else(|| "fade".to_string());

    let aspect_ratio = playlist
        .render
        .aspect_ratio
        .clone()
        .unwrap_or_else(|| "16/9".to_string());

    let render_config = RenderConfig {
        title: title.clone(),
        transition: transition.clone(),
        aspect_ratio: aspect_ratio.clone(),
        speaker_notes: true,
        aspect_lock: playlist.render.aspect_lock.unwrap_or(false),
        ..Default::default()
    };

    let html = build_html(&render_config, &flavors, &resolved_slides)?;
    let html = inject_live_reload(&html);

    // Shared state for the server
    let html_state = Arc::new(RwLock::new(html));
    let (reload_tx, _) = broadcast::channel::<()>(16);
    let reload_tx = Arc::new(reload_tx);

    // Start the tokio runtime for the HTTP server and file watcher
    let rt = tokio::runtime::Runtime::new()?;

    rt.block_on(async {
        // Check if port is available
        let addr = format!("{host}:{port}");
        let listener = tokio::net::TcpListener::bind(&addr)
            .await
            .with_context(|| format!("Port {port} is already in use"))?;

        // Build routes. Source editing is deliberately available only in
        // watch mode; normal built decks remain static and self-contained.
        let html_for_route = Arc::clone(&html_state);
        let reload_tx_for_sse = Arc::clone(&reload_tx);
        let source_state = SourceEditState {
            root: Arc::new(
                std::fs::canonicalize(config.slide_dir())
                    .context("Could not resolve slide directory for source editing")?,
            ),
        };

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
            )
            .route(
                "/__sldr_source",
                get(get_slide_source).put(put_slide_source),
            )
            .with_state(source_state);

        println!(
            "\n  {} http://{}:{}",
            "Serving at".green().bold(),
            host.cyan(),
            port.to_string().cyan()
        );
        if host == "0.0.0.0" {
            // Bound to all interfaces — print the LAN-reachable URLs so a
            // phone or second machine has something to actually type.
            for ip in lan_ips() {
                println!("  {} http://{}:{}", "LAN:".dimmed(), ip.cyan(), port);
            }
        }
        println!("  {} Watching for changes... (Ctrl+C to stop)", "i".blue());

        // Set up file watcher. Watch every dir that feeds a rebuild:
        // slides, playlists, and *all* flavor + layout search dirs (library
        // and configured-extra) — not just the one configured flavor dir, so
        // edits to library flavors and to layouts live-reload too (#1).
        let mut watch_dirs = vec![config.slide_dir(), config.playlist_dir()];
        watch_dirs.extend(config.flavor_dirs());
        watch_dirs.extend(config.layout_dirs());

        let html_for_watcher = Arc::clone(&html_state);
        let reload_tx_for_watcher = Arc::clone(&reload_tx);

        // Clone what the watcher callback needs. Carry flavor *names*, not the
        // resolved flavors, so each rebuild re-reads the flavor from disk and
        // flavor-file edits actually take effect.
        let watch_config = config.clone();
        let watch_playlist_name = playlist_name.to_string();
        let watch_flavor_names = flavor_names.clone();
        let watch_render_config = render_config.clone();

        let (watch_tx, mut watch_rx) = tokio::sync::mpsc::channel::<()>(1);

        let _watcher = spawn_file_watcher(&watch_dirs, watch_tx)?;

        // Spawn the rebuild task
        tokio::spawn(async move {
            while watch_rx.recv().await.is_some() {
                // Debounce: drain any queued events
                tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
                while watch_rx.try_recv().is_ok() {}

                // Rebuild
                match rebuild_presentation(
                    &watch_config,
                    &watch_playlist_name,
                    &watch_render_config,
                    &watch_flavor_names,
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
    flavors: &[Flavor],
    slides: &[sldr_core::slide::Slide],
) -> Result<String> {
    let mut renderer = HtmlRenderer::new(config.clone()).add_flavors(flavors.to_vec());
    // Reload user layouts on every rebuild so layout edits live-reload too.
    for dir in sldr_core::config::Config::load()?.layout_dirs() {
        renderer.load_layouts(&dir)?;
    }
    renderer.add_slides(slides)?;
    let html = renderer.render()?;
    Ok(inject_source_paths(&html, slides))
}

/// Tag watch-mode slide sections with their source markdown path. Keeping this
/// post-processing here avoids leaking local source information into normal
/// builds or adding a development concern to the renderer API.
fn inject_source_paths(html: &str, slides: &[sldr_core::slide::Slide]) -> String {
    const NEEDLE: &str = "<section class=\"sldr-slide\"";
    let mut output = String::with_capacity(html.len() + slides.len() * 48);
    let mut rest = html;

    for slide in slides {
        let Some(pos) = rest.find(NEEDLE) else {
            break;
        };
        let end = pos + NEEDLE.len();
        output.push_str(&rest[..end]);
        output.push_str(" data-sldr-src=\"");
        output.push_str(&html_escape_attr(&slide.relative_path));
        output.push('"');
        rest = &rest[end..];
    }
    output.push_str(rest);
    output
}

fn html_escape_attr(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
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
    playlist_name: &str,
    render_config: &RenderConfig,
    flavor_names: &[String],
) -> Result<String> {
    // Re-resolve flavors from disk on every rebuild so flavor-file edits
    // (colors, background, logos, fonts) take effect, not just slide edits.
    let mut flavors = Vec::new();
    for name in flavor_names {
        flavors.push(super::build::load_flavor(config, name)?);
    }

    let playlist = sldr_core::presentation::Playlist::load(
        &config.playlist_dir().join(format!("{playlist_name}.toml")),
    )?;

    let slides = SlideCollection::load_from_dir(&config.slide_dir())?;
    let matcher = sldr_core::fuzzy::SldrMatcher::new(config.matching.clone());

    let mut resolved = Vec::new();
    for slide_ref in &playlist.slides {
        if let sldr_core::fuzzy::ResolveResult::Found(result) =
            matcher.resolve(slide_ref, &slides.names())
        {
            if let Some(slide) = slides.find(&result.value) {
                resolved.push(slide.clone());
            }
        }
    }

    build_html(render_config, &flavors, &resolved)
}

#[derive(Clone)]
struct SourceEditState {
    root: Arc<PathBuf>,
}

#[derive(Deserialize)]
struct SourceQuery {
    path: String,
}

async fn get_slide_source(
    State(state): State<SourceEditState>,
    Query(query): Query<SourceQuery>,
) -> Response {
    let path = match resolve_slide_source(&state.root, &query.path) {
        Ok(path) => path,
        Err(response) => return response,
    };

    match tokio::fs::read_to_string(path).await {
        Ok(content) => (
            [(header::CONTENT_TYPE, "text/markdown; charset=utf-8")],
            content,
        )
            .into_response(),
        Err(_) => source_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Could not read slide source",
        ),
    }
}

async fn put_slide_source(
    State(state): State<SourceEditState>,
    Query(query): Query<SourceQuery>,
    body: String,
) -> Response {
    let path = match resolve_slide_source(&state.root, &query.path) {
        Ok(path) => path,
        Err(response) => return response,
    };

    match tokio::fs::write(path, body).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(_) => source_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Could not save slide source",
        ),
    }
}

/// Resolve an existing markdown file beneath the configured slide root.
/// Component validation blocks traversal before canonicalization; the prefix
/// check also blocks symlinks that escape the root.
fn resolve_slide_source(root: &Path, requested: &str) -> std::result::Result<PathBuf, Response> {
    let relative = Path::new(requested);
    if requested.is_empty()
        || relative.extension().and_then(|ext| ext.to_str()) != Some("md")
        || relative
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(source_error(
            StatusCode::BAD_REQUEST,
            "Invalid slide source path",
        ));
    }

    let candidate = match std::fs::canonicalize(root.join(relative)) {
        Ok(path) => path,
        Err(_) => {
            return Err(source_error(
                StatusCode::NOT_FOUND,
                "Slide source not found",
            ))
        }
    };
    if !candidate.starts_with(root) || !candidate.is_file() {
        return Err(source_error(
            StatusCode::FORBIDDEN,
            "Slide source is outside slide directory",
        ));
    }
    Ok(candidate)
}

fn source_error(status: StatusCode, message: &'static str) -> Response {
    (status, message).into_response()
}

fn spawn_file_watcher(
    dirs: &[std::path::PathBuf],
    tx: tokio::sync::mpsc::Sender<()>,
) -> Result<RecommendedWatcher> {
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(event) = res {
            if event.kind.is_modify() || event.kind.is_create() || event.kind.is_remove() {
                let _ = tx.blocking_send(());
            }
        }
    })?;

    // De-dup: library and configured-extra dirs can coincide; watching the
    // same path twice errors on some platforms.
    let mut seen = std::collections::HashSet::new();
    for dir in dirs {
        if dir.exists() && seen.insert(dir.clone()) {
            watcher.watch(dir, RecursiveMode::Recursive)?;
        }
    }

    Ok(watcher)
}

/// Non-loopback IPv4 addresses of this machine, for printing reachable
/// URLs when bound to 0.0.0.0. Best-effort: parses `ip -4 addr` /
/// `ifconfig` output; returns empty when neither tool exists.
pub(crate) fn lan_ips() -> Vec<String> {
    let output = std::process::Command::new("ip")
        .args(["-4", "addr"])
        .output()
        .or_else(|_| std::process::Command::new("ifconfig").output());
    let Ok(output) = output else {
        return Vec::new();
    };
    let text = String::from_utf8_lossy(&output.stdout);
    let mut ips = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("inet ") {
            let ip = rest.split(['/', ' ']).next().unwrap_or("");
            if !ip.is_empty() && !ip.starts_with("127.") && !ips.contains(&ip.to_string()) {
                ips.push(ip.to_string());
            }
        }
    }
    ips
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_paths_are_injected_and_escaped() {
        let slides = vec![
            sldr_core::slide::Slide::from_str("one", "nested/one.md", "# One"),
            sldr_core::slide::Slide::from_str("two", "two&more.md", "# Two"),
        ];
        let html = concat!(
            "<section class=\"sldr-slide\" data-index=\"0\"></section>",
            "<section class=\"sldr-slide\" data-index=\"1\"></section>"
        );

        let tagged = inject_source_paths(html, &slides);

        assert!(tagged.contains("data-sldr-src=\"nested/one.md\""));
        assert!(tagged.contains("data-sldr-src=\"two&amp;more.md\""));
    }

    #[test]
    fn source_resolution_stays_inside_slide_root() {
        let dir = tempfile::tempdir().expect("temp directory");
        let root = dir.path().join("slides");
        std::fs::create_dir_all(root.join("nested")).expect("slide directories");
        std::fs::write(root.join("nested/slide.md"), "# Slide").expect("slide source");
        std::fs::write(root.join("not-markdown.txt"), "nope").expect("other source");
        let root = std::fs::canonicalize(root).expect("canonical root");

        let resolved = resolve_slide_source(&root, "nested/slide.md").expect("valid source");
        assert_eq!(resolved, root.join("nested/slide.md"));
        assert!(resolve_slide_source(&root, "../outside.md").is_err());
        assert!(resolve_slide_source(&root, "/etc/passwd").is_err());
        assert!(resolve_slide_source(&root, "not-markdown.txt").is_err());
        assert!(resolve_slide_source(&root, "missing.md").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn source_resolution_rejects_symlinks_outside_slide_root() {
        use std::os::unix::fs::symlink;

        let dir = tempfile::tempdir().expect("temp directory");
        let root = dir.path().join("slides");
        std::fs::create_dir_all(&root).expect("slide directory");
        let outside = dir.path().join("outside.md");
        std::fs::write(&outside, "# Outside").expect("outside source");
        symlink(&outside, root.join("escape.md")).expect("source symlink");
        let root = std::fs::canonicalize(root).expect("canonical root");

        assert!(resolve_slide_source(&root, "escape.md").is_err());
    }
}
