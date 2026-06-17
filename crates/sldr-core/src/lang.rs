//! In-file language blocks (ADR-0007, trx-h4b6).
//!
//! All languages of a slide live in one file — shared frontmatter, layout,
//! media references, and step structure; only prose varies. A `talk_de.md`
//! fork is the translation equivalent of a `-v2` copy: structure drifts
//! immediately. In-file blocks keep staleness visible in the same diff.
//!
//! Syntax follows the existing `::marker::` idiom:
//!
//! ```markdown
//! ![shared-diagram](arch.png)
//!
//! ::lang:en::
//! # Title
//! Body in English.
//!
//! ::lang:de::
//! # Titel
//! Inhalt auf Deutsch.
//! ```
//!
//! Content before the first marker is shared by every language. A slide
//! with no markers is language-neutral and renders unchanged.

/// How a slide's content was selected for a requested language.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LanguageOutcome {
    /// No language markers — the slide is language-neutral.
    Neutral,
    /// The requested (or deck-default) language was present.
    Found(String),
    /// The slide lacks the requested language; another was substituted.
    /// This must surface as a loud build warning — never silently.
    Fallback {
        requested: String,
        used: String,
        available: Vec<String>,
    },
}

/// Result of selecting one language from a slide's content.
#[derive(Debug)]
pub struct LanguageSelection {
    pub content: String,
    pub outcome: LanguageOutcome,
}

/// A language marker line: `::lang:de::` (whitespace-tolerant).
fn parse_marker(line: &str) -> Option<&str> {
    let t = line.trim();
    let inner = t.strip_prefix("::lang:")?.strip_suffix("::")?;
    let code = inner.trim();
    if code.is_empty()
        || !code
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return None;
    }
    Some(code)
}

/// Split content into the shared prefix and ordered per-language sections.
/// A language appearing more than once has its sections concatenated in
/// source order.
fn split_languages(content: &str) -> (String, Vec<(String, String)>) {
    let mut shared = String::new();
    let mut sections: Vec<(String, String)> = Vec::new();
    let mut current: Option<usize> = None;
    let mut in_fence = false;

    for line in content.lines() {
        let t = line.trim();
        if t.starts_with("```") || t.starts_with("~~~") {
            in_fence = !in_fence;
        }
        if let Some(code) = parse_marker(line).filter(|_| !in_fence) {
            let code = code.to_lowercase();
            current = Some(
                sections
                    .iter()
                    .position(|(c, _)| *c == code)
                    .unwrap_or_else(|| {
                        sections.push((code, String::new()));
                        sections.len() - 1
                    }),
            );
            continue;
        }
        match current {
            Some(i) => {
                sections[i].1.push_str(line);
                sections[i].1.push('\n');
            }
            None => {
                shared.push_str(line);
                shared.push('\n');
            }
        }
    }
    (shared, sections)
}

/// Languages present in a slide's content, in source order.
pub fn available_languages(content: &str) -> Vec<String> {
    split_languages(content).1.into_iter().map(|(c, _)| c).collect()
}

/// Select one language from a slide's content.
///
/// - No markers → content unchanged (`Neutral`).
/// - Target is `requested` if given, else `deck_default`.
/// - Target present → shared prefix + that language's sections (`Found`).
/// - Target absent → fall back to the deck default's sections, or to the
///   first language in the file — reported as `Fallback` so the build can
///   warn loudly. A language gap must be visible, never silent.
pub fn select_language(
    content: &str,
    requested: Option<&str>,
    deck_default: &str,
) -> LanguageSelection {
    let (shared, sections) = split_languages(content);
    if sections.is_empty() {
        return LanguageSelection {
            content: content.to_string(),
            outcome: LanguageOutcome::Neutral,
        };
    }

    let target = requested.unwrap_or(deck_default).to_lowercase();
    let available: Vec<String> = sections.iter().map(|(c, _)| c.clone()).collect();

    let pick = |code: &str| -> Option<&(String, String)> {
        sections.iter().find(|(c, _)| c == code)
    };

    let (used, body, fell_back) = match pick(&target) {
        Some((code, body)) => (code.clone(), body.clone(), false),
        None => {
            let default_code = deck_default.to_lowercase();
            let (code, body) = pick(&default_code).unwrap_or(&sections[0]);
            (code.clone(), body.clone(), true)
        }
    };

    let mut content = shared;
    if !content.is_empty() && !content.ends_with("\n\n") && !body.is_empty() {
        content.push('\n');
    }
    content.push_str(&body);

    LanguageSelection {
        content: content.trim_end().to_string() + "\n",
        outcome: if fell_back {
            LanguageOutcome::Fallback {
                requested: target,
                used,
                available,
            }
        } else {
            LanguageOutcome::Found(used)
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BILINGUAL: &str = "\
![diagram](arch.png)

::lang:en::
# Title
Body in English.

::lang:de::
# Titel
Inhalt auf Deutsch.
";

    #[test]
    fn neutral_slide_passes_through_unchanged() {
        let sel = select_language("# Hello\n\nNo markers here.\n", Some("de"), "en");
        assert_eq!(sel.outcome, LanguageOutcome::Neutral);
        assert_eq!(sel.content, "# Hello\n\nNo markers here.\n");
    }

    #[test]
    fn selects_requested_language_with_shared_prefix() {
        let sel = select_language(BILINGUAL, Some("de"), "en");
        assert_eq!(sel.outcome, LanguageOutcome::Found("de".to_string()));
        assert!(sel.content.contains("![diagram](arch.png)"));
        assert!(sel.content.contains("# Titel"));
        assert!(!sel.content.contains("# Title\n"));
        assert!(!sel.content.contains("::lang:"));
    }

    #[test]
    fn no_request_uses_deck_default() {
        let sel = select_language(BILINGUAL, None, "en");
        assert_eq!(sel.outcome, LanguageOutcome::Found("en".to_string()));
        assert!(sel.content.contains("# Title"));
        assert!(!sel.content.contains("# Titel"));
    }

    #[test]
    fn missing_language_falls_back_to_deck_default_and_reports() {
        let sel = select_language(BILINGUAL, Some("fr"), "en");
        match sel.outcome {
            LanguageOutcome::Fallback {
                requested,
                used,
                available,
            } => {
                assert_eq!(requested, "fr");
                assert_eq!(used, "en");
                assert_eq!(available, vec!["en", "de"]);
            }
            other => panic!("expected fallback, got {other:?}"),
        }
        assert!(sel.content.contains("# Title"));
    }

    #[test]
    fn missing_default_falls_back_to_first_language() {
        let only_de = "::lang:de::\n# Titel\n";
        let sel = select_language(only_de, Some("fr"), "en");
        match sel.outcome {
            LanguageOutcome::Fallback { used, .. } => assert_eq!(used, "de"),
            other => panic!("expected fallback, got {other:?}"),
        }
    }

    #[test]
    fn repeated_language_sections_concatenate() {
        let interleaved = "\
::lang:en::
First.
::lang:de::
Erstens.
::lang:en::
Second.
";
        let sel = select_language(interleaved, Some("en"), "en");
        assert!(sel.content.contains("First."));
        assert!(sel.content.contains("Second."));
        assert!(!sel.content.contains("Erstens."));
    }

    #[test]
    fn marker_is_case_insensitive_and_whitespace_tolerant() {
        let content = "  ::lang:EN::  \nHello\n";
        let sel = select_language(content, Some("en"), "en");
        assert_eq!(sel.outcome, LanguageOutcome::Found("en".to_string()));
        assert!(sel.content.contains("Hello"));
    }

    #[test]
    fn lists_available_languages_in_order() {
        assert_eq!(available_languages(BILINGUAL), vec!["en", "de"]);
        assert!(available_languages("plain content").is_empty());
    }

    #[test]
    fn lang_markers_inside_code_fences_are_ignored() {
        let md = "::lang:en::\nReal English.\n```markdown\n::lang:de::\nthis is documentation\n```\nStill English.\n";
        let sel = select_language(md, Some("en"), "en");
        assert_eq!(sel.outcome, LanguageOutcome::Found("en".to_string()));
        assert!(sel.content.contains("Still English."));
        assert!(sel.content.contains("this is documentation"));
        assert_eq!(available_languages(md), vec!["en"]);
    }
}
