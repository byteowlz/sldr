//! Layout candidates (ADR-0011): rank every layout by how well it fits a
//! slide's content, deterministically and without judgment.
//!
//! The rule is arithmetic over two sets — the slots a layout places and the
//! inputs a slide carries — so a human, an agent and the studio's visual
//! layout picker see the same order and the same reasons: what a layout
//! would *hide* (slide input with no slot), what it would *collapse* (a
//! column or image folded into the plain content slot), and what it would
//! leave *empty*. Nothing here picks a layout; it lists the trade-offs.

use schemars::JsonSchema;
use serde::Serialize;
use sldr_core::slide::Slide;

use crate::layout::{LayoutDef, LayoutRegistry};
use crate::markdown::split_segments;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct Candidate {
    pub layout: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Higher fits better; 100 means every input has a home and nothing is empty.
    pub score: i32,
    /// The slide's current layout.
    pub current: bool,
    /// Slide inputs this layout shows nowhere.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub hides: Vec<String>,
    /// Inputs folded into the plain content slot (columns or image without
    /// their own slot) — shown, but not as authored.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub collapses: Vec<String>,
    /// Body slots this layout places that the slide has nothing for.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub empty: Vec<String>,
    /// Whether the layout declares PPTX zones (native editable export).
    pub pptx_zones: bool,
}

const BODY_SLOTS: [&str; 5] = ["heading", "content", "left", "right", "image"];

fn hide_weight(input: &str) -> i32 {
    match input {
        "headline" | "content" | "left" | "right" | "image" => 40,
        _ => 20,
    }
}

/// Rank every layout in `registry` for `slide`. `lang`/`default_lang` pick
/// the body variant the way a build would.
pub fn layout_candidates(
    slide: &Slide,
    registry: &LayoutRegistry,
    lang: Option<&str>,
    default_lang: &str,
) -> Vec<Candidate> {
    let default_lang = if default_lang.is_empty() { "en" } else { default_lang };
    let chrome = slide.metadata.chrome_for(lang, default_lang);
    let body = sldr_core::lang::select_language(&slide.content, lang, default_lang).content;
    let seg = split_segments(&body);
    let present = |s: Option<&String>| s.is_some_and(|t| !t.trim().is_empty());

    let mut inputs: Vec<&str> = Vec::new();
    if chrome.title.is_some() {
        inputs.push("headline");
    }
    if chrome.subtitle.is_some() {
        inputs.push("subheadline");
    }
    // `source` and `footer` are not layout inputs: the flavor's chrome
    // overlay renders them on any layout it lists (ADR-0008), so no layout
    // hides them and none gets credit for placing them.
    if present(seg.heading.as_ref()) {
        inputs.push("heading");
    }
    if present(seg.left.as_ref()) {
        inputs.push("left");
    }
    if present(seg.right.as_ref()) {
        inputs.push("right");
    }
    if present(seg.content.as_ref()) {
        inputs.push("content");
    }
    if present(seg.image.as_ref()) {
        inputs.push("image");
    }
    let current = slide.metadata.layout.clone().unwrap_or_else(|| "default".to_string());

    let mut out: Vec<Candidate> = registry
        .names()
        .iter()
        .filter_map(|n| registry.get(n))
        .map(|def| score_layout(def, &inputs, &current))
        .collect();
    // Ties: the slide's current layout first, then by name.
    out.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| b.current.cmp(&a.current))
            .then_with(|| a.layout.cmp(&b.layout))
    });
    out
}

fn score_layout(def: &LayoutDef, inputs: &[&str], current: &str) -> Candidate {
    let slots = def.slots();
    let has = |s: &str| slots.contains(&s);
    let mut hides = Vec::new();
    let mut collapses = Vec::new();
    let mut score = 100;

    for input in inputs {
        if has(input) {
            continue;
        }
        // The renderer folds columns/heading/image into `{{content}}` when
        // the layout lacks the specific slot: shown, but not as authored.
        let folds = has("content") && matches!(*input, "heading" | "left" | "right" | "image");
        if folds {
            collapses.push((*input).to_string());
            score -= 5;
        } else {
            hides.push((*input).to_string());
            score -= hide_weight(input);
        }
    }

    let mut empty = Vec::new();
    for slot in BODY_SLOTS {
        if !has(slot) || inputs.contains(&slot) {
            continue;
        }
        // A content slot is satisfied by anything the renderer folds into it.
        if slot == "content" && inputs.iter().any(|i| matches!(*i, "heading" | "left" | "right" | "image")) {
            continue;
        }
        // Column layouts: a single-block body fills one column unhappily but
        // is not "empty"; still, the missing column is.
        empty.push(slot.to_string());
        score -= if slot == "image" { 25 } else { 10 };
    }

    // Affinity: authored structure matched by a dedicated slot.
    if inputs.contains(&"left") && has("left") {
        score += 10;
    }
    if inputs.contains(&"image") && has("image") {
        score += 10;
    }

    Candidate {
        layout: def.name.clone(),
        category: def.category.clone(),
        tags: def.tags.clone(),
        score,
        current: def.name == current,
        hides,
        collapses,
        empty,
        pptx_zones: !def.zones.is_empty(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slide(body: &str) -> Slide {
        Slide::from_str("s", "genai/s.md", body)
    }

    fn rank(body: &str) -> Vec<Candidate> {
        layout_candidates(&slide(body), &LayoutRegistry::builtin(), None, "en")
    }

    fn get<'a>(v: &'a [Candidate], name: &str) -> &'a Candidate {
        v.iter().find(|c| c.layout == name).unwrap_or_else(|| panic!("{name}"))
    }

    #[test]
    fn columns_prefer_column_layouts_and_collapse_elsewhere() {
        let v = rank("---\ntitle: T\n---\n::left::\n- a\n::right::\n- b\n");
        let two = get(&v, "two-cols");
        let def = get(&v, "default");
        assert!(two.score > def.score, "{two:?} vs {def:?}");
        assert!(two.collapses.is_empty());
        assert_eq!(def.collapses, vec!["left", "right"]);
        assert!(def.hides.contains(&"headline".to_string()) || def.collapses.contains(&"headline".to_string()) || !def.hides.is_empty());
    }

    #[test]
    fn image_body_prefers_image_layouts() {
        let v = rank("::content::\ntext\n::image::\n![x](media/x.png)\n");
        let il = get(&v, "image-left");
        let def = get(&v, "default");
        assert!(il.score > def.score);
        assert!(il.empty.is_empty());
        assert_eq!(def.collapses, vec!["image"]);
    }

    #[test]
    fn framed_shows_chrome_that_default_hides() {
        let v = rank("---\ntitle: Head\nsubtitle: Sub\n---\nbody\n");
        let framed = get(&v, "framed");
        let def = get(&v, "default");
        assert!(framed.hides.is_empty(), "{framed:?}");
        assert!(def.hides.contains(&"headline".to_string()));
        assert!(framed.score > def.score);
        assert!(framed.pptx_zones);
    }

    #[test]
    fn current_layout_is_marked_and_order_is_stable() {
        let s = Slide::from_str("s", "s.md", "---\nlayout: cover\n---\n# Hi\n");
        let reg = LayoutRegistry::builtin();
        let a = layout_candidates(&s, &reg, None, "en");
        let b = layout_candidates(&s, &reg, None, "en");
        assert_eq!(a, b);
        assert!(a.iter().any(|c| c.layout == "cover" && c.current));
        assert_eq!(a.iter().filter(|c| c.current).count(), 1);
    }
}
