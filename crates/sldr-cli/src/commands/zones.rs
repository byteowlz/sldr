//! `sldr zones <slide>` — print the zone document (ADR-0011): every region the
//! slide's layout declares, what fills it, and which file an edit writes to.
//!
//! The one-door view of what a visual editor would show. Agents ask this
//! before editing a slide; the studio and the PPTX exporter read the same
//! structure. Nothing is written.

use anyhow::{bail, Context, Result};
use colored::Colorize;
use sldr_core::config::Config;
use sldr_core::flavor::Flavor;
use sldr_core::fuzzy::{ResolveResult, SldrMatcher};
use sldr_core::slide::{Slide, SlideCollection};
use sldr_renderer::{Binding, LayoutRegistry, Writes, ZoneDocument, ZoneOpts};

pub struct ZonesArgs<'a> {
    pub slide: &'a str,
    /// Override the slide's `layout` field.
    pub layout: Option<&'a str>,
    /// Flavor to resolve style chrome (footer) against. `None` = config default.
    pub flavor: Option<&'a str>,
    pub lang: Option<&'a str>,
    pub json: bool,
}

pub fn run(args: &ZonesArgs) -> Result<()> {
    let config = Config::load()?;
    let slide = resolve_slide(&config, args.slide)?;

    let mut registry = LayoutRegistry::builtin();
    let mut user_layouts: Vec<String> = Vec::new();
    for dir in config.layout_dirs() {
        if let Ok(entries) = std::fs::read_dir(&dir) {
            user_layouts.extend(entries.flatten().filter_map(|e| {
                let p = e.path();
                (p.extension()? == "html").then(|| p.file_stem()?.to_str().map(String::from))?
            }));
        }
        registry.load_dir(&dir)?;
    }
    let layout_name = args
        .layout
        .map(String::from)
        .or_else(|| slide.metadata.layout.clone())
        .unwrap_or_else(|| "default".to_string());
    let layout = registry.resolve(&layout_name)?;
    let layout_builtin = !user_layouts.iter().any(|n| n == &layout.name);

    let flavor_name = args
        .flavor
        .map(String::from)
        .unwrap_or_else(|| config.config.default_flavor.clone());
    let flavor: Option<Flavor> = match crate::commands::build::load_flavor(&config, &flavor_name) {
        Ok(f) => Some(f),
        Err(e) if args.flavor.is_none() => {
            eprintln!("{} {e:#} — footer chrome resolved without a flavor", "note:".yellow());
            None
        }
        Err(e) => return Err(e),
    };

    let opts = ZoneOpts {
        lang: args.lang,
        default_lang: "en",
        layout_builtin,
        layout_used_by: None,
    };
    let doc = sldr_renderer::zone_document(&slide, layout, flavor.as_ref(), &opts);

    if args.json {
        println!("{}", serde_json::to_string_pretty(&doc)?);
    } else {
        print_table(&doc);
    }
    Ok(())
}

fn resolve_slide(config: &Config, name: &str) -> Result<Slide> {
    let slides = SlideCollection::load_from_dir(&config.slide_dir())?;
    if let Some(s) = slides.find(name) {
        return Ok(s.clone());
    }
    let direct = Config::expand_path(name);
    if direct.is_file() {
        return Slide::load_with_base(&direct, &config.slide_dir())
            .or_else(|_| Slide::load(&direct))
            .with_context(|| format!("Could not load slide {}", direct.display()));
    }
    let matcher = SldrMatcher::new(config.matching.clone());
    match matcher.resolve(name, &slides.names()) {
        ResolveResult::Found(r) => slides.find(&r.value).cloned().context("Slide not found"),
        ResolveResult::NotFound => bail!("Slide '{name}' not found"),
        ResolveResult::Multiple(m) => bail!(
            "Ambiguous slide '{name}'. Candidates: {}",
            m.iter().map(|x| x.value.as_str()).collect::<Vec<_>>().join(", ")
        ),
    }
}

fn print_table(doc: &ZoneDocument) {
    println!(
        "{} {}  {} {}{}  {} {}",
        "slide".dimmed(),
        doc.slide.cyan(),
        "layout".dimmed(),
        doc.layout.cyan(),
        if doc.geometry.builtin { " (built-in)".dimmed().to_string() } else { String::new() },
        "lang".dimmed(),
        doc.language
    );
    if let Some(f) = &doc.flavor {
        println!("{} {}", "flavor".dimmed(), f.cyan());
    }
    println!();
    println!(
        "{:<12} {:<16} {:<22} {:<28} {}",
        "ZONE".bold(), "REP".bold(), "BOX %".bold(), "BINDING".bold(), "WRITES".bold()
    );
    for z in &doc.zones {
        let [x, y, w, h] = z.bbox;
        let binding = match &z.binding {
            Binding::Frontmatter { field } => format!("frontmatter:{field}"),
            Binding::Markdown { slot, range } => match range {
                Some((a, b)) => format!("markdown:{slot} [{a}..{b}]"),
                None => format!("markdown:{slot}"),
            },
            Binding::Image { slot, src } => match src {
                Some(s) => format!("image:{slot} {s}"),
                None => format!("image:{slot} (empty)"),
            },
            Binding::Flavor { field } => format!("flavor:{field}"),
            Binding::None => "—".to_string(),
        };
        let writes = match &z.writes {
            Some(Writes::Slide { path }) => path.clone(),
            Some(Writes::Flavor { name }) => format!("flavor {name}").yellow().to_string(),
            None => "layout only".dimmed().to_string(),
        };
        println!(
            "{:<12} {:<16} {:<22} {:<28} {}",
            z.name,
            z.rep,
            format!("{x:.1} {y:.1} {w:.1}×{h:.1}"),
            binding,
            writes
        );
        if let Some(c) = &z.content {
            let one = c.lines().next().unwrap_or("").trim();
            let more = if c.lines().count() > 1 { " …" } else { "" };
            println!("             {}{}", one.dimmed(), more.dimmed());
        }
    }
    if !doc.unbound.is_empty() {
        println!();
        println!(
            "{} {}",
            "not shown by this layout:".yellow(),
            doc.unbound.iter().map(|u| u.name.as_str()).collect::<Vec<_>>().join(", ")
        );
    }
    println!();
    println!(
        "{}",
        format!(
            "geometry belongs to layout '{}'{} — moving a box edits every slide using it",
            doc.geometry.layout,
            doc.geometry.used_by.map(|n| format!(" (used by {n} slides)")).unwrap_or_default()
        )
        .dimmed()
    );
}
