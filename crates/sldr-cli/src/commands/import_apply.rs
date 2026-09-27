//! `sldr import deck.pptx --apply` — bring edits made in PowerPoint back into
//! the ORIGINAL slides (ADR-0010/0011).
//!
//! Only zones the package reports as edited since export are written, and
//! each goes where the zone document says it belongs: headline/subtitle/
//! source into frontmatter (the `translations.<lang>` block when the slide
//! has one for the exported language), body segments into the matching
//! `::lang::` block and `::left::`/`::content::`/… segment, a replaced
//! picture into the slide's `media/` with its alt text kept. Flavor-owned
//! zones (a flavor footer) are never written into a slide — changing shared
//! style is a deliberate act elsewhere (ADR-0002). Untouched zones, other
//! languages and everything PowerPoint cannot express stay exactly as they
//! were.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use colored::Colorize;
use sldr_core::config::Config;
use sldr_core::fuzzy::SldrMatcher;
use sldr_core::slide::{Slide, SlideCollection};
use sldr_core::usage::UsageIndex;
use sldr_pptx::{ImportedSlide, ImportedZone};

#[derive(Debug, Default)]
struct Plan {
    path: String,
    raw_before: String,
    raw_after: String,
    applied: Vec<String>,
    skipped: Vec<(String, String)>,
    new_media: Vec<(String, Vec<u8>)>,
}

pub fn run(file: &str, dry_run: bool, options: &super::interchange::Options) -> Result<()> {
    let config = Config::load()?;
    let bytes = std::fs::read(file).with_context(|| format!("Failed to read {file}"))?;
    let slide_dir = config.slide_dir();
    let imported = options.resolve(sldr_pptx::import_with_report(&bytes), &slide_dir.join(".sldr-apply"))?;
    let library = SlideCollection::load_from_dir(&slide_dir)?;
    let usage = UsageIndex::build(&config.playlist_dir(), &library, &SldrMatcher::new(config.matching.clone()));

    println!("{} edits from {} into the library{}", "Applying".green().bold(), file.cyan(), if dry_run { " (dry run)".yellow().to_string() } else { String::new() });

    let mut plans: BTreeMap<String, Plan> = BTreeMap::new();
    for s in &imported {
        if s.layout == super::export::RASTER_LAYOUT {
            continue; // exported as a picture: nothing editable came back
        }
        let Some(source) = s.source_id.as_deref() else {
            println!("  {} a slide without sldr identity — use plain `sldr import -o <dir>` for it", "skip".yellow());
            continue;
        };
        let Some(original) = library.find(source) else {
            println!("  {} '{source}' is no longer in the library — not applied", "skip".yellow());
            continue;
        };
        let plan = plans.entry(original.relative_path.clone()).or_insert_with(|| {
            let raw = std::fs::read_to_string(&original.path).unwrap_or_default();
            Plan { path: original.relative_path.clone(), raw_before: raw.clone(), raw_after: raw, ..Default::default() }
        });
        plan_slide(original, s, plan);
    }

    let mut changed = 0;
    for plan in plans.values() {
        if plan.applied.is_empty() && plan.skipped.is_empty() {
            continue;
        }
        let decks = usage.of(&plan.path).len();
        let shared = if decks > 1 { format!(" · in {decks} decks").yellow().to_string() } else { String::new() };
        println!("  {}{shared}", plan.path.cyan());
        for a in &plan.applied {
            println!("    {} {a}", "✓".green());
        }
        for (what, why) in &plan.skipped {
            println!("    {} {what} — {why}", "·".dimmed());
        }
        if plan.raw_after != plan.raw_before {
            changed += 1;
            if !dry_run {
                let path = slide_dir.join(&plan.path);
                let media = path.parent().context("slide has no folder")?.join("media");
                for (name, bytes) in &plan.new_media {
                    std::fs::create_dir_all(&media)?;
                    std::fs::write(media.join(name), bytes)?;
                }
                std::fs::write(&path, &plan.raw_after).with_context(|| format!("Failed to write {}", path.display()))?;
            }
        }
    }
    if changed == 0 {
        println!("  {}", "no edits to apply — every zone matches what was exported".dimmed());
    } else if dry_run {
        println!("\n{} {changed} slide(s) would change; run without --dry-run to write", "Dry run:".yellow().bold());
    } else {
        println!("\n{} {changed} slide(s) updated · review with `git diff`", "Success!".green().bold());
    }
    Ok(())
}

fn plan_slide(original: &Slide, s: &ImportedSlide, plan: &mut Plan) {
    let lang = s.language.as_deref().unwrap_or("en").to_lowercase();
    let translated = original.metadata.translations.contains_key(&lang);
    let (mut fm, mut body) = split_raw(&plan.raw_after);

    for z in &s.zones {
        let edited = match (&z.image, z.changed) {
            (Some(_), _) => true, // compared below against the source file
            (None, Some(c)) => c,
            (None, None) => false,
        };
        if !edited {
            continue;
        }
        if z.owner == "flavor" {
            plan.skipped.push((z.zone.clone(), "comes from the flavor (shared); edit the flavor to change it".into()));
            continue;
        }
        match z.zone.as_str() {
            "headline" | "subheadline" | "footer" | "source" => {
                let key = match z.zone.as_str() { "headline" => "title", "subheadline" => "subtitle", k => k };
                let value = if key == "source" { strip_source_label(&z.value) } else { z.value.clone() };
                fm = if translated {
                    set_nested(&fm, &["translations", &lang, key], &value)
                } else {
                    set_top(&fm, key, &value)
                };
                plan.applied.push(format!("{key}{}", if translated { format!(" ({lang})") } else { String::new() }));
            }
            slot @ ("heading" | "content" | "left" | "right") => match replace_segment(&body, &lang, slot, &z.value) {
                Ok(b) => {
                    body = b;
                    plan.applied.push(format!("{slot} text"));
                }
                Err(why) => plan.skipped.push((slot.to_string(), why)),
            },
            "image" => match replace_image(original, &body, &lang, z) {
                Ok(Some((b, name, bytes))) => {
                    body = b;
                    plan.new_media.push((name.clone(), bytes));
                    plan.applied.push(format!("image → media/{name}"));
                }
                Ok(None) => {}
                Err(why) => plan.skipped.push(("image".into(), why)),
            },
            other => plan.skipped.push((other.to_string(), "no mapping back to the slide".into())),
        }
    }
    plan.raw_after = join_raw(&fm, &body);
}

// ---------------------------------------------------------------------------
// Frontmatter: minimal, formatting-preserving line edits
// ---------------------------------------------------------------------------

fn split_raw(raw: &str) -> (String, String) {
    if let Some(rest) = raw.strip_prefix("---\n") {
        if let Some(end) = rest.find("\n---\n") {
            return (rest[..end + 1].to_string(), rest[end + 5..].to_string());
        }
        if let Some(stripped) = rest.strip_suffix("\n---") {
            return (format!("{stripped}\n"), String::new());
        }
    }
    (String::new(), raw.to_string())
}

fn join_raw(fm: &str, body: &str) -> String {
    if fm.is_empty() { body.to_string() } else { format!("---\n{fm}---\n{body}") }
}

fn indent_of(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

/// Replace `key: …` at the given indent within `lines[start..end]`, or insert
/// it at `end`. A block scalar's continuation lines are replaced too.
fn set_in(lines: &mut Vec<String>, start: usize, end: usize, indent: usize, key: &str, value: &str) {
    let prefix = format!("{}{key}:", " ".repeat(indent));
    let existing = (start..end).find(|&i| lines[i].starts_with(&prefix) && indent_of(&lines[i]) == indent);
    // Keep the author's quoting: a value that was double-quoted stays so.
    let was_quoted = existing.is_some_and(|i| lines[i][prefix.len()..].trim_start().starts_with('"'));
    let rendered = super::import::yaml_value(value);
    let rendered = if was_quoted && !rendered.starts_with('"') {
        format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        rendered
    };
    let line = format!("{}{key}: {rendered}", " ".repeat(indent));
    if let Some(i) = existing {
        let mut j = i + 1;
        while j < end && (lines[j].trim().is_empty() || indent_of(&lines[j]) > indent) {
            j += 1;
        }
        lines.splice(i..j, [line]);
    } else {
        lines.insert(end, line);
    }
}

fn set_top(fm: &str, key: &str, value: &str) -> String {
    let mut lines: Vec<String> = fm.lines().map(String::from).collect();
    let n = lines.len();
    set_in(&mut lines, 0, n, 0, key, value);
    lines.join("\n") + "\n"
}

/// `translations: / <lang>: / <key>: value`, creating missing levels.
fn set_nested(fm: &str, path: &[&str], value: &str) -> String {
    let mut lines: Vec<String> = fm.lines().map(String::from).collect();
    let (mut start, mut end, mut indent) = (0usize, lines.len(), 0usize);
    for (depth, seg) in path.iter().enumerate() {
        if depth == path.len() - 1 {
            set_in(&mut lines, start, end, indent, seg, value);
            break;
        }
        let head = format!("{}{seg}:", " ".repeat(indent));
        let found = (start..end).find(|&i| lines[i].starts_with(&head) && indent_of(&lines[i]) == indent);
        let at = match found {
            Some(i) => i,
            None => {
                lines.insert(end, head.clone());
                end
            }
        };
        let child = indent + 2;
        let mut stop = at + 1;
        while stop < lines.len() && (lines[stop].trim().is_empty() || indent_of(&lines[stop]) >= child) {
            stop += 1;
        }
        start = at + 1;
        end = stop;
        indent = child;
    }
    lines.join("\n") + "\n"
}

fn strip_source_label(v: &str) -> String {
    // `[Source: label](url)` — the current export form.
    let v = match v.strip_prefix('[').and_then(|i| i.strip_suffix(')')).and_then(|b| b.rsplit_once("](")) {
        Some((text, _url)) => text,
        None => v,
    };
    let body = v.strip_prefix("Source: ").or_else(|| v.strip_prefix("Quelle: ")).unwrap_or(v);
    match body.strip_suffix(')').and_then(|b| b.rsplit_once(" (")) {
        Some((text, url)) if url.starts_with("http") => text.to_string(),
        _ => body.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Body: language block + segment
// ---------------------------------------------------------------------------

/// Byte range of the `::lang:<lang>::` block in `body`, or the whole body
/// when the slide has no language blocks.
fn lang_block(body: &str, lang: &str) -> std::ops::Range<usize> {
    let marker = format!("::lang:{lang}::");
    match body.find(&marker) {
        Some(at) => {
            let start = at + marker.len();
            let end = body[start..].find("::lang:").map_or(body.len(), |e| start + e);
            start..end
        }
        None => 0..body.len(),
    }
}

fn replace_segment(body: &str, lang: &str, slot: &str, value: &str) -> std::result::Result<String, String> {
    let range = lang_block(body, lang);
    let block = &body[range.clone()];
    let seg = sldr_renderer::split_segments(block);
    let old = match slot {
        "heading" => seg.heading,
        "content" => seg.content,
        "left" => seg.left,
        "right" => seg.right,
        _ => None,
    }
    .filter(|s| !s.is_empty())
    .ok_or_else(|| format!("the slide has no {slot} segment to replace"))?;
    let hits = block.matches(old.as_str()).count();
    if hits != 1 {
        return Err(format!("could not locate the {slot} segment unambiguously"));
    }
    let at = range.start + block.find(old.as_str()).unwrap_or(0);
    Ok(format!("{}{}{}", &body[..at], value.trim(), &body[at + old.len()..]))
}

/// Replace the image reference in the image segment when PowerPoint carries
/// different bytes than the file it points at. Alt text is kept.
fn replace_image(
    original: &Slide,
    body: &str,
    lang: &str,
    z: &ImportedZone,
) -> std::result::Result<Option<(String, String, Vec<u8>)>, String> {
    let img = z.image.as_ref().ok_or("no image bytes")?;
    let range = lang_block(body, lang);
    let block = &body[range.clone()];
    let seg = sldr_renderer::split_segments(block).image.or_else(|| sldr_renderer::split_segments(block).content).unwrap_or_default();
    let src = sldr_core::media::references(&seg).into_iter().next().ok_or("the slide has no image reference to replace")?;
    let dir = original.path.parent().ok_or("slide has no folder")?;
    if std::fs::read(dir.join(&src)).is_ok_and(|old| old == img.bytes) {
        return Ok(None); // same picture
    }
    let stem = original.name.as_str();
    let ext = Path::new(&img.file_name).extension().and_then(|e| e.to_str()).unwrap_or("png");
    let name = (1..)
        .map(|n| format!("{stem}-pptx-{n}.{ext}"))
        .find(|n| !dir.join("media").join(n).exists())
        .unwrap_or_else(|| format!("{stem}-pptx.{ext}"));
    let at = range.start + block.find(src.as_str()).ok_or("image reference not found")?;
    let new_body = format!("{}media/{name}{}", &body[..at], &body[at + src.len()..]);
    Ok(Some((new_body, name, img.bytes.clone())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn top_level_key_is_replaced_in_place() {
        let fm = "title: Old\nlayout: framed\n";
        assert_eq!(set_top(fm, "title", "New: yes"), "title: \"New: yes\"\nlayout: framed\n");
        assert_eq!(set_top(fm, "subtitle", "S"), "title: Old\nlayout: framed\nsubtitle: S\n");
    }

    #[test]
    fn nested_translation_is_replaced_or_created() {
        let fm = "title: Hello\ntranslations:\n  de:\n    title: Hallo\n    source: Q\nlayout: framed\n";
        let out = set_nested(fm, &["translations", "de", "title"], "Servus");
        assert_eq!(out, "title: Hello\ntranslations:\n  de:\n    title: Servus\n    source: Q\nlayout: framed\n");
        let quoted = set_nested("translations:\n  de:\n    title: \"Alt\"\n", &["translations", "de", "title"], "Neu");
        assert_eq!(quoted, "translations:\n  de:\n    title: \"Neu\"\n");
        let out = set_nested("title: Hello\n", &["translations", "fr", "title"], "Bonjour");
        assert_eq!(out, "title: Hello\ntranslations:\n  fr:\n    title: Bonjour\n");
    }

    #[test]
    fn segment_replaced_only_in_its_language_block() {
        let body = "::lang:en::\n::content::\nold text\n::image::\n![a](media/x.png)\n\n::lang:de::\n::content::\nalter Text\n::image::\n![b](media/x.png)\n";
        let out = replace_segment(body, "en", "content", "new text").unwrap();
        assert!(out.contains("::content::\nnew text\n::image::"));
        assert!(out.contains("alter Text"), "German block untouched");
        let out = replace_segment(body, "de", "content", "neuer Text").unwrap();
        assert!(out.contains("neuer Text") && out.contains("old text"));
    }

    #[test]
    fn plain_body_segment_replacement() {
        let body = "\n- a\n- b\n";
        assert_eq!(replace_segment(body, "en", "content", "- c").unwrap(), "\n- c\n");
        assert!(replace_segment(body, "en", "left", "x").is_err());
    }

    #[test]
    fn raw_split_round_trips() {
        let raw = "---\ntitle: T\n---\nbody\n";
        let (fm, body) = split_raw(raw);
        assert_eq!((fm.as_str(), body.as_str()), ("title: T\n", "body\n"));
        assert_eq!(join_raw(&fm, &body), raw);
    }
}
