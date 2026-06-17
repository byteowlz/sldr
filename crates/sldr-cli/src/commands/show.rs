//! Show command — print the raw source of a layout or flavor.
//!
//! Legibility primitive for agents and humans: `ls` lists *names*, `show`
//! prints the actual *source* that a given name resolves to. It honors the
//! same resolution order as a build (user library/config dirs override the
//! embedded built-ins) and prints what would really be used — so it works
//! uniformly whether a layout/flavor is a built-in or a user override, with
//! no need to know where it lives on disk. Source goes to stdout (pipeable);
//! a one-line origin note goes to stderr.

use anyhow::{bail, Result};
use colored::Colorize;
use sldr_core::config::Config;
use sldr_core::fuzzy::{ResolveResult, SldrMatcher};

use crate::flavors;

/// `sldr show <what> <name>` — `what` is "layout" or "flavor".
pub fn run(what: &str, name: &str, json: bool) -> Result<()> {
    let config = Config::load()?;
    let matcher = SldrMatcher::new(config.matching.clone());

    match what.to_lowercase().as_str() {
        "layout" | "layouts" => show_layout(&config, &matcher, name, json),
        "flavor" | "flavors" | "flavour" | "flavours" => {
            show_flavor(&config, &matcher, name, json)
        }
        other => bail!(
            "Don't know how to show '{other}'. Try: sldr show layout <name> | sldr show flavor <name>"
        ),
    }
}

/// Resolve `name` against `candidates` (exact first, then fuzzy), failing
/// loud with the available set — the message is the fix.
fn resolve_name(matcher: &SldrMatcher, kind: &str, name: &str, mut candidates: Vec<String>) -> Result<String> {
    candidates.sort();
    candidates.dedup();
    match matcher.resolve(name, &candidates) {
        ResolveResult::Found(m) => Ok(m.value),
        ResolveResult::NotFound => bail!(
            "{kind} '{name}' not found. Available: {}",
            candidates.join(", ")
        ),
        ResolveResult::Multiple(matches) => {
            let opts: Vec<&str> = matches.iter().map(|m| m.value.as_str()).collect();
            bail!("Ambiguous {kind} '{name}'. Candidates: {}", opts.join(", "))
        }
    }
}

fn show_layout(config: &Config, matcher: &SldrMatcher, name: &str, json: bool) -> Result<()> {
    // Candidate names: built-ins + every `*.html` stem in the user layout dirs.
    let mut names: Vec<String> = sldr_renderer::builtin_layout_names()
        .into_iter()
        .map(String::from)
        .collect();
    for dir in config.layout_dirs() {
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "html") {
                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                        names.push(stem.to_string());
                    }
                }
            }
        }
    }
    let resolved = resolve_name(matcher, "Layout", name, names)?;

    // Resolution order matches a build: user dirs (library first, then the
    // configured extra) win over the built-in of the same name.
    for dir in config.layout_dirs() {
        let path = dir.join(format!("{resolved}.html"));
        if path.is_file() {
            let source = std::fs::read_to_string(&path)?;
            return emit(json, "layout", &resolved, &path.display().to_string(), &source);
        }
    }
    if let Some(source) = sldr_renderer::builtin_layout_source(&resolved) {
        return emit(json, "layout", &resolved, "built-in", source);
    }
    bail!("Layout '{resolved}' resolved but no source found (internal error)")
}

fn show_flavor(config: &Config, matcher: &SldrMatcher, name: &str, json: bool) -> Result<()> {
    // Candidate names: built-in slugs + every subdir of the flavor dirs.
    let mut names: Vec<String> = flavors::builtin_flavor_slugs()
        .into_iter()
        .map(String::from)
        .collect();
    for dir in config.flavor_dirs() {
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    if let Some(n) = entry.file_name().to_str() {
                        names.push(n.to_string());
                    }
                }
            }
        }
    }
    let resolved = resolve_name(matcher, "Flavor", name, names)?;

    // User dirs (library, then configured extra) override the built-in slug.
    for dir in config.flavor_dirs() {
        let fdir = dir.join(&resolved);
        let toml = fdir.join("flavor.toml");
        if toml.is_file() {
            let mut source = std::fs::read_to_string(&toml)?;
            // A flavor may ship a free-form `flavor.css` escape hatch; show it
            // too so the picture is complete, clearly delimited.
            let css = fdir.join("flavor.css");
            if css.is_file() {
                source.push_str("\n\n/* ---- flavor.css (escape hatch) ---- */\n");
                source.push_str(&std::fs::read_to_string(&css)?);
            }
            return emit(json, "flavor", &resolved, &toml.display().to_string(), &source);
        }
    }
    if let Some(files) = flavors::builtin_flavor_files(&resolved) {
        // Built-ins ship a single flavor.toml today; concatenate any others.
        let source = files
            .iter()
            .map(|f| f.content)
            .collect::<Vec<_>>()
            .join("\n");
        return emit(json, "flavor", &resolved, "built-in", &source);
    }
    bail!("Flavor '{resolved}' resolved but no source found (internal error)")
}

/// Print the source to stdout and the origin to stderr (or one JSON object
/// to stdout with `--json`).
fn emit(json: bool, kind: &str, name: &str, origin: &str, source: &str) -> Result<()> {
    if json {
        let obj = serde_json::json!({
            "kind": kind,
            "name": name,
            "origin": origin,
            "source": source,
        });
        println!("{}", serde_json::to_string_pretty(&obj)?);
    } else {
        eprintln!(
            "{} {} {} {}",
            "#".dimmed(),
            kind.dimmed(),
            name.cyan(),
            format!("({origin})").dimmed()
        );
        print!("{source}");
        if !source.ends_with('\n') {
            println!();
        }
    }
    Ok(())
}
