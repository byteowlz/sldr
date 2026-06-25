//! Export command - export a presentation to PDF via headless browser
//!
//! Uses Chromium/Chrome in headless mode with --print-to-pdf.
//! The presentation's built-in @media print CSS handles the layout.

use std::net::TcpListener;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use colored::Colorize;
use sldr_core::config::Config;
use sldr_core::slide::Slide;
use sldr_pptx::ZoneContent;

/// A tiny JS snippet injected into the page that expands all slides
/// for printing (shows all slides, no transitions, no toolbar).
const PRINT_PREP_SCRIPT: &str = r"
<script>
// When loaded with ?print query param, prepare for PDF export
if (window.location.search.includes('print')) {
  document.addEventListener('DOMContentLoaded', function() {
    // Show all slides simultaneously for print
    document.querySelectorAll('.sldr-slide').forEach(function(s) {
      s.classList.add('active');
      s.style.display = 'flex';
      s.style.position = 'relative';
      s.style.pageBreakAfter = 'always';
    });
    // Per-page logos: the deck-level .sldr-logos overlay is a single
    // absolutely-positioned element, so in paged media it only lands on the
    // first page. Clone each slide's matching logos (by data-logo-layouts)
    // into the slide itself so every printed page carries its own logos.
    var overlay = document.querySelector('.sldr-logos');
    if (overlay) {
      var logos = Array.prototype.slice.call(overlay.querySelectorAll('.sldr-logo'));
      document.querySelectorAll('.sldr-slide').forEach(function(slide) {
        var layout = slide.getAttribute('data-layout') || '';
        var holder = null;
        logos.forEach(function(logo) {
          var list = (logo.getAttribute('data-logo-layouts') || '').split(/\s+/);
          if (list.indexOf('all') !== -1 || list.indexOf(layout) !== -1) {
            if (!holder) { holder = document.createElement('div'); holder.className = 'sldr-logos'; }
            var clone = logo.cloneNode(true);
            clone.classList.add('sldr-logo-on');
            holder.appendChild(clone);
          }
        });
        if (holder) slide.appendChild(holder);
      });
      overlay.style.display = 'none';
    }
    // Hide toolbar and nav
    var toolbar = document.querySelector('.sldr-toolbar');
    if (toolbar) toolbar.style.display = 'none';
    var nav = document.querySelector('.sldr-nav');
    if (nav) nav.style.display = 'none';
    var progress = document.querySelector('.sldr-progress');
    if (progress) progress.style.display = 'none';
    // Shrink-to-fit every slide once they're all laid out for print (the
    // presenter's per-slide fit hook only runs for the active slide).
    requestAnimationFrame(function() {
      if (window.__sldrFitAll) window.__sldrFitAll();
    });
  });
}
</script>
";

#[allow(clippy::too_many_arguments)]
pub fn run(
    playlist_name: Option<&str>,
    flavor: Option<String>,
    output: Option<String>,
    lang: Option<String>,
    format: &str,
    template: bool,
    flatten: bool,
) -> Result<()> {
    let config = Config::load()?;

    // Template mode is flavor-scoped, not playlist-scoped: it emits masters +
    // theme + layouts and no slides, so it short-circuits before any slide
    // resolution. (trx-4s9s.3, now folded into one export channel.)
    if template {
        if format != "pptx" {
            anyhow::bail!("--template is only valid with --format pptx");
        }
        return export_template(&config, playlist_name, flavor, output);
    }

    let playlist_name = playlist_name
        .context("a playlist name is required (or pass --template for a flavor's masters)")?;

    println!(
        "{} presentation '{}' to {}",
        "Exporting".green().bold(),
        playlist_name.cyan(),
        format.to_uppercase().yellow()
    );

    // Build the presentation first
    let playlist = super::build::load_playlist(&config, playlist_name)?;
    let flavor_name = flavor
        .or(playlist.flavor.clone())
        .unwrap_or_else(|| config.config.default_flavor.clone());
    let flavor = super::build::load_flavor(&config, &flavor_name)?;

    let slides = sldr_core::slide::SlideCollection::load_from_dir(&config.slide_dir())?;
    let matcher = sldr_core::fuzzy::SldrMatcher::new(config.matching.clone());

    let mut resolved_slides = Vec::new();
    for slide_ref in &playlist.slides {
        if let Some(slide) =
            super::build::resolve_with_interactive(&matcher, slide_ref, &slides)?
        {
            resolved_slides.push(slide);
        }
    }

    if resolved_slides.is_empty() {
        anyhow::bail!("No slides resolved.");
    }

    let title = playlist
        .title
        .clone()
        .unwrap_or_else(|| playlist.name.clone());

    let transition = playlist
        .render
        .transition
        .clone()
        .unwrap_or_else(|| "none".to_string()); // No transitions for export

    // Language axis: CLI --lang (comma list) > playlist default_lang > "en".
    // A PDF can't toggle language, so multiple languages export one file each
    // (deck.de.pdf, deck.en.pdf) — see the per-language loop below.
    let default_language = playlist
        .default_lang
        .clone()
        .unwrap_or_else(|| "en".to_string());
    let languages: Vec<String> = lang
        .as_deref()
        .map(|l| {
            l.split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();

    let aspect_ratio = playlist
        .render
        .aspect_ratio
        .clone()
        .unwrap_or_else(|| "16/9".to_string());

    // A PDF/PPTX can't toggle language at view time, so each requested
    // language is its own file (deck.de.pdf, deck.en.pdf) rather than one
    // combined document. No --lang → a single file in the deck default.
    let export_langs: Vec<Option<String>> = if languages.len() > 1 {
        languages.iter().cloned().map(Some).collect()
    } else {
        vec![languages.first().cloned()]
    };
    let multi = export_langs.len() > 1;

    // Base output path: custom --output, or <output_dir>/<playlist>.<ext>.
    let base_path = if let Some(out) = output {
        PathBuf::from(out)
    } else {
        let output_dir = config.output_dir().join(&playlist.name);
        std::fs::create_dir_all(&output_dir)?;
        let ext = if format == "pptx" { "pptx" } else { "pdf" };
        output_dir.join(format!("{}.{ext}", playlist.name))
    };

    // The native PPTX path (default for --format pptx) maps slides to editable
    // OOXML and never renders HTML/screenshots. `--flatten` opts back into the
    // lossy screenshot writer; PDF and screenshot both still need the HTML.
    let native_pptx = format == "pptx" && !flatten;

    for lang_opt in export_langs {
        // Suffix the filename with the language when emitting one per lang.
        let out_path = match (&lang_opt, multi) {
            (Some(l), true) => insert_lang_suffix(&base_path, l),
            _ => base_path.clone(),
        };

        let final_path = if native_pptx {
            let pptx_path = ensure_ext(&out_path, "pptx");
            let bytes = build_native_deck(
                &config,
                &resolved_slides,
                &flavor,
                &title,
                lang_opt.as_deref(),
                &default_language,
            )?;
            std::fs::write(&pptx_path, &bytes)
                .with_context(|| format!("Failed to write {}", pptx_path.display()))?;
            if !pptx_path.exists() {
                anyhow::bail!("PPTX write reported success but no file at {}", pptx_path.display());
            }
            pptx_path
        } else {
            // PDF or screenshot-PPTX: render the HTML deck first.
            let render_config = sldr_renderer::RenderConfig {
                title: title.clone(),
                transition: transition.clone(),
                aspect_ratio: aspect_ratio.clone(),
                speaker_notes: false,
                languages: lang_opt.clone().map(|l| vec![l]).unwrap_or_default(),
                default_language: default_language.clone(),
                aspect_lock: playlist.render.aspect_lock.unwrap_or(false),
                ..Default::default()
            };
            let mut renderer =
                sldr_renderer::HtmlRenderer::new(render_config).add_flavor(flavor.clone());
            for dir in config.layout_dirs() {
                renderer.load_layouts(&dir)?;
            }
            renderer.add_slides(&resolved_slides)?;
            let html = inject_print_prep(&renderer.render()?);

            match format {
                "pdf" => {
                    export_pdf(&html, &out_path)?;
                    out_path
                }
                "pptx" => {
                    let pptx_path = ensure_ext(&out_path, "pptx");
                    export_pptx(&html, resolved_slides.len(), &pptx_path)?;
                    pptx_path
                }
                other => {
                    anyhow::bail!("Unsupported export format: {other}. Supported: pdf, pptx")
                }
            }
        };

        println!(
            "\n{} Exported to {}",
            "Success!".green().bold(),
            final_path.display().to_string().cyan()
        );
    }

    Ok(())
}

/// Force a path's extension (`foo.pdf` → `foo.pptx`).
fn ensure_ext(path: &Path, ext: &str) -> PathBuf {
    if path.extension().and_then(|e| e.to_str()) == Some(ext) {
        path.to_path_buf()
    } else {
        path.with_extension(ext)
    }
}

/// `--template`: emit a flavor's masters + theme + layouts (no slides) so a
/// user can author branded slides directly in PowerPoint (trx-4s9s.3).
fn export_template(
    config: &Config,
    playlist_name: Option<&str>,
    flavor: Option<String>,
    output: Option<String>,
) -> Result<()> {
    // Flavor: --flavor, else the named playlist's flavor, else config default.
    let flavor_name = match flavor {
        Some(f) => f,
        None => match playlist_name {
            Some(p) => super::build::load_playlist(config, p)?
                .flavor
                .unwrap_or_else(|| config.config.default_flavor.clone()),
            None => config.config.default_flavor.clone(),
        },
    };
    let flavor = super::build::load_flavor(config, &flavor_name)?;

    println!(
        "{} PowerPoint template for flavor '{}'",
        "Generating".green().bold(),
        flavor.name.cyan()
    );

    let mut registry = sldr_renderer::LayoutRegistry::builtin();
    for dir in config.layout_dirs() {
        registry.load_dir(&dir)?;
    }
    let names = registry.names();
    let defs: Vec<&sldr_renderer::LayoutDef> =
        names.iter().filter_map(|n| registry.get(n)).collect();
    let (eligible, skipped) = sldr_pptx::select_layouts(defs);

    if eligible.is_empty() {
        anyhow::bail!(
            "No layouts with PPTX placeholder zones. Annotate a layout with \
             `<!-- sldr:zone … rep=placeholder-text … -->` directives."
        );
    }
    if !skipped.is_empty() {
        println!(
            "  {} {} (no placeholder-text zones — filled-deck only)",
            "skipped:".yellow(),
            skipped.join(", ")
        );
    }

    let theme = sldr_pptx::Theme::from_flavor(&flavor);
    let bytes = sldr_pptx::build_template(&theme, &eligible)
        .context("Failed to generate PPTX template OOXML")?;

    let out_path = match output {
        Some(o) => PathBuf::from(o),
        None => {
            let dir = config.output_dir();
            std::fs::create_dir_all(&dir)?;
            dir.join(format!("{}-template.pptx", flavor.name))
        }
    };
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(&out_path, &bytes)
        .with_context(|| format!("Failed to write {}", out_path.display()))?;

    let included: Vec<&str> = eligible.iter().map(|d| d.name.as_str()).collect();
    println!(
        "\n{} {} layout(s): {}\n{} Wrote {}",
        "Included".green(),
        eligible.len(),
        included.join(", ").cyan(),
        "Success!".green().bold(),
        out_path.display().to_string().cyan()
    );
    Ok(())
}

/// Build a native, editable deck `.pptx` for one language: resolve each
/// slide's layout and chrome, map them onto the layout's placeholder zones,
/// and hand off to the OOXML generator (trx-4s9s.4).
fn build_native_deck(
    config: &Config,
    slides: &[Slide],
    flavor: &sldr_core::flavor::Flavor,
    title: &str,
    lang: Option<&str>,
    default_lang: &str,
) -> Result<Vec<u8>> {
    let mut registry = sldr_renderer::LayoutRegistry::builtin();
    for dir in config.layout_dirs() {
        registry.load_dir(&dir)?;
    }

    let mut inputs: Vec<sldr_pptx::SlideInput> = Vec::new();
    for slide in slides {
        let layout_name = slide
            .metadata
            .layout
            .clone()
            .unwrap_or_else(|| "default".to_string());
        let layout = registry.resolve(&layout_name)?;

        let chrome = slide.metadata.chrome_for(lang, default_lang);
        let footer = chrome.footer.clone().or_else(|| flavor.footer.clone());

        // Resolve in-file `::lang:xx::` blocks for this language before
        // splitting columns — otherwise the markers leak into the body. A
        // language gap surfaces loudly, never silently (mirrors the HTML path).
        let lang_sel = sldr_core::lang::select_language(&slide.content, lang, default_lang);
        if let sldr_core::lang::LanguageOutcome::Fallback {
            requested, used, ..
        } = &lang_sel.outcome
        {
            println!(
                "  {} slide '{}' has no '{}' body — used '{}'",
                "warning:".yellow(),
                slide.name,
                requested,
                used
            );
        }
        let segments = sldr_renderer::split_segments(&lang_sel.content);

        let mut fields: Vec<(String, ZoneContent)> = Vec::new();
        if let Some(t) = chrome.title {
            fields.push(("headline".into(), ZoneContent::Text(t)));
        }
        if let Some(s) = chrome.subtitle {
            fields.push(("subheadline".into(), ZoneContent::Text(s)));
        }
        if let Some(f) = footer {
            fields.push(("footer".into(), ZoneContent::Text(f)));
        }
        if let Some(src) = chrome.source {
            let label = source_label(lang, default_lang);
            let text = match chrome.source_url {
                Some(u) => format!("{label} {src} ({u})"),
                None => format!("{label} {src}"),
            };
            fields.push(("source".into(), ZoneContent::Text(text)));
        }
        if let Some(h) = segments.heading {
            fields.push(("heading".into(), ZoneContent::Markdown(h)));
        }
        if let Some(c) = segments.content {
            fields.push(("content".into(), ZoneContent::Markdown(c)));
        }
        if let Some(l) = segments.left {
            fields.push(("left".into(), ZoneContent::Markdown(l)));
        }
        if let Some(r) = segments.right {
            fields.push(("right".into(), ZoneContent::Markdown(r)));
        }
        // Image segment → an embedded picture for a `picture` zone. Only
        // raster formats PowerPoint reads natively; anything else (svg, remote)
        // is left out and reported, never silently mangled.
        if let Some(img_md) = segments.image {
            match resolve_picture(&img_md, &slide.path) {
                Some((bytes, ext)) => {
                    fields.push(("image".into(), ZoneContent::Picture { bytes, ext }));
                }
                None => println!(
                    "  {} slide '{}': image not embeddable as native PPTX (left empty)",
                    "note:".yellow(),
                    slide.name
                ),
            }
        }

        inputs.push(sldr_pptx::SlideInput { layout, fields });
    }

    let theme = sldr_pptx::Theme::from_flavor(flavor);
    sldr_pptx::build_deck(&theme, title, &inputs)
}

/// Resolve the first image in an `::image::` segment to raw bytes + a PPTX
/// media extension, or `None` if it isn't a local raster PowerPoint reads
/// natively (remote URLs, data URIs, svg → caller reports and skips).
fn resolve_picture(image_md: &str, slide_path: &Path) -> Option<(Vec<u8>, String)> {
    let src = extract_img_src(image_md)?;
    if src.starts_with("http://") || src.starts_with("https://") || src.starts_with("data:") {
        return None;
    }
    let ext = Path::new(&src)
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)?;
    // Normalize jpg→jpeg; only formats declared in [Content_Types].
    let ext = match ext.as_str() {
        "png" | "gif" | "jpeg" => ext,
        "jpg" => "jpeg".to_string(),
        _ => return None,
    };
    let dir = slide_path.parent()?;
    let bytes = std::fs::read(dir.join(&src)).ok()?;
    Some((bytes, ext))
}

/// Extract the `src` of the first `![alt](src)` image in a markdown fragment.
fn extract_img_src(md: &str) -> Option<String> {
    let start = md.find("![")?;
    let open = md[start..].find("](")? + start + 2;
    let close = md[open..].find(')')? + open;
    let src = md[open..close].trim();
    // Strip an optional "title" after the URL: ](src "title").
    let src = src.split_whitespace().next().unwrap_or(src);
    (!src.is_empty()).then(|| src.to_string())
}

/// Localized "Source:" label, mirroring the HTML renderer's set.
fn source_label(requested: Option<&str>, default_lang: &str) -> &'static str {
    const LABELS: &[(&str, &str)] = &[
        ("en", "Source:"),
        ("de", "Quelle:"),
        ("fr", "Source :"),
        ("es", "Fuente:"),
        ("it", "Fonte:"),
        ("pt", "Fonte:"),
        ("nl", "Bron:"),
    ];
    let lookup = |code: &str| {
        LABELS
            .iter()
            .find(|(c, _)| *c == code)
            .map(|(_, l)| *l)
    };
    let target = requested.unwrap_or(default_lang).to_lowercase();
    lookup(&target)
        .or_else(|| lookup(&default_lang.to_lowercase()))
        .unwrap_or("Source:")
}

/// Insert `.<lang>` before a path's extension: `foo.pdf` + `de` → `foo.de.pdf`.
fn insert_lang_suffix(path: &Path, lang: &str) -> PathBuf {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("pdf");
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("export");
    let mut out = path.to_path_buf();
    out.set_file_name(format!("{stem}.{lang}.{ext}"));
    out
}

fn inject_print_prep(html: &str) -> String {
    if let Some(pos) = html.rfind("</body>") {
        let mut result = String::with_capacity(html.len() + PRINT_PREP_SCRIPT.len());
        result.push_str(&html[..pos]);
        result.push_str(PRINT_PREP_SCRIPT);
        result.push_str(&html[pos..]);
        result
    } else {
        format!("{html}{PRINT_PREP_SCRIPT}")
    }
}

fn export_pdf(html: &str, output_path: &std::path::Path) -> Result<()> {
    // Find a browser binary
    let browser = find_browser()?;
    println!("  {} Using {}", "i".blue(), browser.display());

    // Serve the HTML on a temp port
    let port = allocate_port()?;
    let html_owned = html.to_string();

    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let addr = format!("127.0.0.1:{port}");
        let listener = tokio::net::TcpListener::bind(&addr).await?;

        let app = axum::Router::new().route(
            "/",
            axum::routing::get(move || {
                let content = html_owned.clone();
                async move { axum::response::Html(content) }
            }),
        );

        // Spawn server
        let server_handle = tokio::spawn(async move {
            axum::serve(listener, app).await.ok();
        });

        // Give the server a moment to start
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Run headless browser
        let url = format!("http://127.0.0.1:{port}/?print");
        println!("  {} Rendering PDF...", ">".cyan());

        let out = tokio::process::Command::new(&browser)
            .args([
                "--headless",
                "--disable-gpu",
                "--no-sandbox",
                "--run-all-compositor-stages-before-draw",
                "--virtual-time-budget=5000",
                &format!("--print-to-pdf={}", output_path.display()),
                "--print-to-pdf-no-header",
                &url,
            ])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .output()
            .await
            .context("Failed to run headless browser")?;

        // Abort the server
        server_handle.abort();

        // Verify the PDF was actually written — Chrome can exit 0 yet write
        // nothing (e.g. a sandbox/permission denial on the output path), so
        // the exit code alone is not proof. Fail loud with Chrome's own error.
        let wrote = std::fs::metadata(output_path)
            .map(|m| m.len() > 0)
            .unwrap_or(false);
        if !out.status.success() || !wrote {
            let stderr = String::from_utf8_lossy(&out.stderr);
            let mut tail: Vec<&str> = stderr.lines().rev().filter(|l| !l.trim().is_empty()).take(3).collect();
            tail.reverse();
            let detail = if tail.is_empty() {
                "Is Chrome/Chromium installed and able to write the output path?".to_string()
            } else {
                tail.join("\n")
            };
            anyhow::bail!(
                "Headless browser did not produce a PDF at {}.\n{detail}",
                output_path.display()
            );
        }

        Ok::<(), anyhow::Error>(())
    })?;

    Ok(())
}

fn export_pptx(html: &str, slide_count: usize, output_path: &std::path::Path) -> Result<()> {
    let browser = find_browser()?;
    println!("  {} Using {}", "i".blue(), browser.display());
    println!(
        "  {} Capturing {} slide screenshots...",
        ">".cyan(),
        slide_count
    );

    let temp_dir = tempfile::tempdir()?;
    let html_owned = html.to_string();

    let rt = tokio::runtime::Runtime::new()?;
    let images = rt.block_on(async {
        // Allocate port and start server
        let port = allocate_port()?;
        let addr = format!("127.0.0.1:{port}");
        let listener = tokio::net::TcpListener::bind(&addr).await?;

        let app = axum::Router::new().route(
            "/",
            axum::routing::get(move || {
                let content = html_owned.clone();
                async move { axum::response::Html(content) }
            }),
        );

        let server_handle = tokio::spawn(async move {
            axum::serve(listener, app).await.ok();
        });

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Screenshot each slide by navigating to #/N
        let mut image_paths = Vec::new();
        for i in 1..=slide_count {
            let url = format!("http://127.0.0.1:{port}/#{i}");
            let img_path = temp_dir.path().join(format!("slide_{i}.png"));

            let status = tokio::process::Command::new(&browser)
                .args([
                    "--headless",
                    "--disable-gpu",
                    "--no-sandbox",
                    "--window-size=1920,1080",
                    "--hide-scrollbars",
                    "--virtual-time-budget=3000",
                    &format!("--screenshot={}", img_path.display()),
                    &url,
                ])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .await
                .with_context(|| format!("Failed to screenshot slide {i}"))?;

            if !status.success() {
                anyhow::bail!("Chrome screenshot failed for slide {i}");
            }

            image_paths.push(img_path);
        }

        server_handle.abort();
        Ok::<Vec<PathBuf>, anyhow::Error>(image_paths)
    })?;

    println!("  {} Creating PPTX...", ">".cyan());
    sldr_renderer::pptx::create_pptx(&images, output_path)?;

    Ok(())
}

/// Find Chrome/Chromium binary (respects CHROME_BIN env var)
fn find_browser() -> Result<PathBuf> {
    // Check CHROME_BIN environment variable first
    if let Ok(chrome_bin) = std::env::var("CHROME_BIN") {
        let path = PathBuf::from(&chrome_bin);
        if path.exists() {
            return Ok(path);
        }
    }

    let candidates = [
        "chromium",
        "chromium-browser",
        "google-chrome",
        "google-chrome-stable",
        "/usr/bin/chromium",
        "/usr/bin/google-chrome",
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        "/Applications/Chromium.app/Contents/MacOS/Chromium",
    ];

    for candidate in &candidates {
        let path = PathBuf::from(candidate);
        // Check if it's an absolute path that exists
        if path.is_absolute() && path.exists() {
            return Ok(path);
        }
        // Check if it's in PATH
        if let Ok(output) = std::process::Command::new("which")
            .arg(candidate)
            .output()
        {
            if output.status.success() {
                let found = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !found.is_empty() {
                    return Ok(PathBuf::from(found));
                }
            }
        }
    }

    anyhow::bail!(
        "No Chrome/Chromium browser found. Install one of:\n\
         - chromium\n\
         - google-chrome\n\
         Or set CHROME_BIN environment variable."
    );
}

fn allocate_port() -> Result<u16> {
    let listener = TcpListener::bind("127.0.0.1:0").context("Failed to bind to ephemeral port")?;
    let port = listener
        .local_addr()
        .context("Failed to read assigned port")?
        .port();
    Ok(port)
}
