//! Find (ADR-0011): one ranking over the slide library shared by the CLI,
//! the studio finder, the deck board's search columns, and the new-slide
//! similarity warning. Fuzzy on names plus full text over title, tags,
//! topic, description and body — the body search is the part PowerPoint
//! cannot do at all.
//!
//! Deterministic: same library, same query, same order. No stemming, no
//! synonyms, no judgment — an agent enriching tags and topics improves the
//! ranking, the tool itself never guesses.

use schemars::JsonSchema;
use serde::Serialize;

use crate::config::MatchingConfig;
use crate::fuzzy::{MatchType, SldrMatcher};
use crate::slide::{Slide, SlideCollection};

/// Where a query matched inside one slide, with a short excerpt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct FieldHit {
    /// `name`, `title`, `tags`, `topic`, `description`, or `body`.
    pub field: &'static str,
    /// The matching text — the tag, the title, or a window of body text
    /// around the first hit.
    pub snippet: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct Hit {
    pub relative_path: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Effective layout (`layout:` field, else `default`).
    pub layout: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topic: Option<String>,
    /// Higher is better. Only meaningful for ordering within one query.
    pub score: i64,
    pub matched: Vec<FieldHit>,
}

#[derive(Debug, Clone, Default)]
pub struct FindOpts {
    /// Keep only slides carrying at least one of these tags (case-insensitive).
    pub tags: Vec<String>,
    /// Keep only slides whose topic contains this (case-insensitive).
    pub topic: Option<String>,
    /// Cap the result list; `None` returns everything that matched.
    pub limit: Option<usize>,
}

const W_NAME_EXACT: i64 = 1_000;
const W_TITLE: i64 = 400;
const W_TAG: i64 = 300;
const W_TOPIC: i64 = 200;
const W_DESCRIPTION: i64 = 120;
const W_BODY: i64 = 80;
/// Per additional body occurrence, capped.
const W_BODY_MORE: i64 = 10;
const BODY_MORE_CAP: i64 = 50;
const SNIPPET_RADIUS: usize = 60;

/// Rank `slides` against `query`. Every whitespace-separated token must
/// occur somewhere in a slide (name, title, tags, topic, description or
/// body) for it to be a text hit; a fuzzy name match counts on its own so
/// `sldr find intr` still finds `intro`.
pub fn find(query: &str, slides: &SlideCollection, matching: &MatchingConfig, opts: &FindOpts) -> Vec<Hit> {
    let query = query.trim();
    if query.is_empty() {
        return Vec::new();
    }
    let tokens: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();

    // Fuzzy arm over names, uncapped (the matcher's own cap is for
    // disambiguation prompts, not search results).
    let matcher = SldrMatcher::new(MatchingConfig {
        max_suggestions: usize::MAX,
        ..matching.clone()
    });
    let names = slides.names();
    let fuzzy: Vec<(String, i64, MatchType)> = matcher
        .find_all(query, &names)
        .into_iter()
        .map(|m| (m.value, m.score, m.match_type))
        .collect();

    let tag_filter: Vec<String> = opts.tags.iter().map(|t| t.to_lowercase()).collect();
    let topic_filter = opts.topic.as_deref().map(str::to_lowercase);

    let mut hits: Vec<Hit> = slides
        .slides
        .iter()
        .filter(|s| passes_filters(s, &tag_filter, topic_filter.as_deref()))
        .filter_map(|s| {
            let fuzzy_hit = fuzzy.iter().find(|(v, _, _)| v == &s.relative_path);
            score_slide(s, &tokens, fuzzy_hit.map(|(_, sc, mt)| (*sc, *mt)))
        })
        .collect();

    hits.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.relative_path.cmp(&b.relative_path)));
    if let Some(n) = opts.limit {
        hits.truncate(n);
    }
    hits
}

fn passes_filters(s: &Slide, tags: &[String], topic: Option<&str>) -> bool {
    if !tags.is_empty() {
        let mine: Vec<String> = s.metadata.tags.iter().map(|t| t.to_lowercase()).collect();
        if !tags.iter().any(|t| mine.contains(t)) {
            return false;
        }
    }
    if let Some(t) = topic {
        match &s.metadata.topic {
            Some(mine) if mine.to_lowercase().contains(t) => {}
            _ => return false,
        }
    }
    true
}

fn score_slide(s: &Slide, tokens: &[String], fuzzy: Option<(i64, MatchType)>) -> Option<Hit> {
    let mut score = 0i64;
    let mut matched: Vec<FieldHit> = Vec::new();

    let title = s.metadata.title.as_deref().unwrap_or("");
    let title_l = title.to_lowercase();
    let topic_l = s.metadata.topic.as_deref().unwrap_or("").to_lowercase();
    let desc = s.metadata.description.as_deref().unwrap_or("");
    let desc_l = desc.to_lowercase();
    let body_l = s.content.to_lowercase();
    let name_l = s.relative_path.to_lowercase();
    let tags_l: Vec<String> = s.metadata.tags.iter().map(|t| t.to_lowercase()).collect();

    // Text arm: every token must land somewhere.
    let mut all_tokens_hit = true;
    let mut body_first: Option<usize> = None;
    for tok in tokens {
        let mut hit = false;
        if name_l.contains(tok.as_str()) {
            hit = true;
        }
        if !title_l.is_empty() && title_l.contains(tok.as_str()) {
            score += W_TITLE;
            hit = true;
        }
        if let Some(tag) = tags_l.iter().position(|t| t.contains(tok.as_str())) {
            score += W_TAG;
            hit = true;
            push_unique(&mut matched, "tags", s.metadata.tags[tag].clone());
        }
        if !topic_l.is_empty() && topic_l.contains(tok.as_str()) {
            score += W_TOPIC;
            hit = true;
        }
        if !desc_l.is_empty() && desc_l.contains(tok.as_str()) {
            score += W_DESCRIPTION;
            hit = true;
        }
        if let Some(pos) = body_l.find(tok.as_str()) {
            let extra = body_l.matches(tok.as_str()).count().saturating_sub(1);
            let more = i64::try_from(extra).unwrap_or(i64::MAX).min(BODY_MORE_CAP / W_BODY_MORE);
            score += W_BODY + more * W_BODY_MORE;
            hit = true;
            body_first = Some(body_first.map_or(pos, |p| p.min(pos)));
        }
        if !hit {
            all_tokens_hit = false;
        }
    }
    if all_tokens_hit {
        if tokens.iter().any(|t| title_l.contains(t.as_str())) && !title.is_empty() {
            push_unique(&mut matched, "title", title.to_string());
        }
        if tokens.iter().any(|t| topic_l.contains(t.as_str())) {
            if let Some(t) = &s.metadata.topic {
                push_unique(&mut matched, "topic", t.clone());
            }
        }
        if tokens.iter().any(|t| desc_l.contains(t.as_str())) && !desc.is_empty() {
            push_unique(&mut matched, "description", desc.to_string());
        }
        if let Some(pos) = body_first {
            push_unique(&mut matched, "body", snippet(&s.content, pos));
        }
    } else {
        score = 0;
        matched.clear();
    }

    // Fuzzy arm on the name/path.
    if let Some((fscore, mt)) = fuzzy {
        let add = match mt {
            MatchType::Exact | MatchType::Anchor => W_NAME_EXACT,
            _ => fscore.min(W_TITLE - 1),
        };
        score += add;
        push_unique(&mut matched, "name", s.relative_path.clone());
    }

    if score == 0 {
        return None;
    }
    Some(Hit {
        relative_path: s.relative_path.clone(),
        name: s.name.clone(),
        title: s.metadata.title.clone(),
        layout: s.metadata.layout.clone().unwrap_or_else(|| "default".to_string()),
        tags: s.metadata.tags.clone(),
        topic: s.metadata.topic.clone(),
        score,
        matched,
    })
}

fn push_unique(v: &mut Vec<FieldHit>, field: &'static str, snippet: String) {
    if !v.iter().any(|h| h.field == field) {
        v.push(FieldHit { field, snippet });
    }
}

/// A one-line window of `text` around byte offset `pos` (from the lowercased
/// copy; offsets coincide for ASCII and are clamped to char boundaries
/// otherwise), whitespace collapsed.
fn snippet(text: &str, pos: usize) -> String {
    let pos = pos.min(text.len());
    let mut start = pos.saturating_sub(SNIPPET_RADIUS);
    let mut end = (pos + SNIPPET_RADIUS).min(text.len());
    while start > 0 && !text.is_char_boundary(start) {
        start -= 1;
    }
    while end < text.len() && !text.is_char_boundary(end) {
        end += 1;
    }
    let window = text[start..end].split_whitespace().collect::<Vec<_>>().join(" ");
    let lead = if start > 0 { "…" } else { "" };
    let tail = if end < text.len() { "…" } else { "" };
    format!("{lead}{window}{tail}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lib() -> SlideCollection {
        SlideCollection {
            slides: vec![
                Slide::from_str("intro", "genai/intro.md", "---\ntitle: Why agents need evals\ntags: [agents, evals]\ntopic: agents\n---\nAn agent without an eval harness is a demo.\n"),
                Slide::from_str("loop", "genai/loop.md", "---\ntitle: The tool-use loop\ntags: [agents, tool-use]\n---\nModel, tool, observation. Repeat until the model answers.\n"),
                Slide::from_str("crdt", "coedit/crdt.md", "---\ntitle: CRDT vs OT\n---\nTwo cursors, one file. Evals are irrelevant here.\n"),
                Slide::from_str("cover", "shared/cover.md", "---\nlayout: cover\n---\n# Welcome\n"),
            ],
            base_dir: std::path::PathBuf::from("/lib"),
        }
    }

    fn run(q: &str) -> Vec<Hit> {
        find(q, &lib(), &MatchingConfig::default(), &FindOpts::default())
    }

    #[test]
    fn title_and_tag_hits_outrank_body_only() {
        let hits = run("evals");
        let paths: Vec<&str> = hits.iter().map(|h| h.relative_path.as_str()).collect();
        assert_eq!(paths, vec!["genai/intro.md", "coedit/crdt.md"]);
        assert!(hits[0].matched.iter().any(|m| m.field == "title"));
        assert!(hits[0].matched.iter().any(|m| m.field == "tags" && m.snippet == "evals"));
        let crdt = &hits[1];
        assert_eq!(crdt.matched.len(), 1);
        assert_eq!(crdt.matched[0].field, "body");
        assert!(crdt.matched[0].snippet.contains("Evals are irrelevant"));
    }

    #[test]
    fn every_token_must_land_somewhere() {
        let bad = run("harness cursors");
        assert!(bad.is_empty(), "no slide has both, got {bad:?}");
        let hits = run("tool observation");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].relative_path, "genai/loop.md");
    }

    #[test]
    fn fuzzy_name_finds_without_text_hit() {
        let hits = run("intr");
        assert_eq!(hits[0].relative_path, "genai/intro.md");
        assert!(hits[0].matched.iter().any(|m| m.field == "name"));
    }

    #[test]
    fn exact_name_wins_over_everything() {
        let hits = run("crdt");
        assert_eq!(hits[0].relative_path, "coedit/crdt.md");
        assert!(hits[0].score >= W_NAME_EXACT);
    }

    #[test]
    fn filters_and_limit_apply() {
        let o = FindOpts { tags: vec!["Tool-Use".into()], ..Default::default() };
        let hits = find("agents", &lib(), &MatchingConfig::default(), &o);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].relative_path, "genai/loop.md");
        let o = FindOpts { limit: Some(1), ..Default::default() };
        assert_eq!(find("agents", &lib(), &MatchingConfig::default(), &o).len(), 1);
        assert_eq!(hits[0].layout, "default");
    }

    #[test]
    fn deterministic_order_and_empty_query() {
        assert!(run("").is_empty());
        let a = run("the");
        let b = run("the");
        assert_eq!(a, b);
    }

    #[test]
    fn snippet_respects_char_boundaries() {
        let s = "ä".repeat(100) + "needle" + &"ö".repeat(100);
        let out = snippet(&s, s.find("needle").unwrap());
        assert!(out.contains("needle"));
        assert!(out.starts_with('…') && out.ends_with('…'));
    }
}
