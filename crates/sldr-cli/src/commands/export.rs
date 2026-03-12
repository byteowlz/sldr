//! Export command - export a presentation to PDF via headless browser
//!
//! Uses Chromium/Chrome in headless mode with --print-to-pdf.
//! The presentation's built-in @media print CSS handles the layout.

use std::net::TcpListener;
use std::path::PathBuf;

use anyhow::{Context, Result};
use colored::Colorize;
use sldr_core::config::Config;

/// A tiny JS snippet injected into the page that expands all slides
/// for printing (shows all slides, no transitions, no toolbar).
const PRINT_PREP_SCRIPT: &str = r#"
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
    // Hide toolbar and nav
    var toolbar = document.querySelector('.sldr-toolbar');
    if (toolbar) toolbar.style.display = 'none';
    var nav = document.querySelector('.sldr-nav');
    if (nav) nav.style.display = 'none';
    var progress = document.querySelector('.sldr-progress');
    if (progress) progress.style.display = 'none';
  });
}
</script>
"#;

pub fn run(
    skeleton_name: &str,
    flavor: Option<String>,
    output: Option<String>,
    format: &str,
) -> Result<()> {
    let config = Config::load()?;

    println!(
        "{} presentation '{}' to {}",
        "Exporting".green().bold(),
        skeleton_name.cyan(),
        format.to_uppercase().yellow()
    );

    // Build the presentation first
    let skeleton = super::build::load_skeleton(&config, skeleton_name)?;
    let flavor_name = flavor
        .or(skeleton.flavor.clone())
        .unwrap_or_else(|| config.config.default_flavor.clone());
    let flavor = super::build::load_flavor(&config, &flavor_name)?;

    let slides = sldr_core::slide::SlideCollection::load_from_dir(&config.slide_dir())?;
    let matcher = sldr_core::fuzzy::SldrMatcher::new(config.matching.clone());

    let mut resolved_slides = Vec::new();
    for slide_ref in &skeleton.slides {
        if let Some(slide) =
            super::build::resolve_with_interactive(&matcher, slide_ref, &slides)?
        {
            resolved_slides.push(slide);
        }
    }

    if resolved_slides.is_empty() {
        anyhow::bail!("No slides resolved.");
    }

    let title = skeleton
        .title
        .clone()
        .unwrap_or_else(|| skeleton.name.clone());

    let transition = skeleton
        .slidev_config
        .transition
        .clone()
        .unwrap_or_else(|| "none".to_string()); // No transitions for export

    let render_config = sldr_renderer::RenderConfig {
        title,
        transition,
        aspect_ratio: skeleton
            .slidev_config
            .aspect_ratio
            .clone()
            .unwrap_or_else(|| "16/9".to_string()),
        speaker_notes: false, // No notes in PDF
    };

    let mut renderer =
        sldr_renderer::HtmlRenderer::new(render_config).add_flavor(flavor);
    renderer.add_slides(&resolved_slides);
    let html = renderer.render()?;

    // Inject print preparation script
    let html = inject_print_prep(&html);

    // Determine output path
    let output_path = if let Some(out) = output {
        PathBuf::from(out)
    } else {
        let output_dir = config.output_dir().join(&skeleton.name);
        std::fs::create_dir_all(&output_dir)?;
        output_dir.join(format!("{}.pdf", skeleton.name))
    };

    match format {
        "pdf" => export_pdf(&html, &output_path)?,
        "pptx" => {
            let pptx_path = if output_path.extension().is_some_and(|e| e == "pdf") {
                output_path.with_extension("pptx")
            } else {
                output_path.clone()
            };
            export_pptx(&html, resolved_slides.len(), &pptx_path)?;
            println!(
                "\n{} Exported to {}",
                "Success!".green().bold(),
                pptx_path.display().to_string().cyan()
            );
            return Ok(());
        }
        other => anyhow::bail!("Unsupported export format: {other}. Supported: pdf, pptx"),
    }

    println!(
        "\n{} Exported to {}",
        "Success!".green().bold(),
        output_path.display().to_string().cyan()
    );

    Ok(())
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

        let status = tokio::process::Command::new(&browser)
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
            .status()
            .await
            .context("Failed to run headless browser")?;

        // Abort the server
        server_handle.abort();

        if !status.success() {
            anyhow::bail!("Headless browser exited with error. Is Chrome/Chromium installed?");
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
