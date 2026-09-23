//! Where-used (ADR-0011): which playlists reference a slide, which slides use
//! a layout, and when a file was last touched in git.
//!
//! Everything here is *derived* — from playlists on disk and git history —
//! and never stored. It backs blast-radius warnings before a shared-source
//! edit (ADR-0002), the "in N decks" badges on the deck board, and reuse
//! sorting, without adding a field to any format (ADR-0001).
//!
//! Playlist entries resolve to slides with the same rule a build uses
//! (exact path or name first, then the configured fuzzy matcher), minus the
//! interactive fallback: an entry that stays ambiguous counts for nobody and
//! is reported in [`UsageIndex::unresolved`] instead of guessed.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

use schemars::JsonSchema;
use serde::Serialize;

use crate::fuzzy::{ResolveResult, SldrMatcher};
use crate::presentation::Playlist;
use crate::slide::SlideCollection;

/// One playlist that references a slide.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct PlaylistRef {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flavor: Option<String>,
    /// 1-based position of the slide in the playlist.
    pub position: usize,
    /// Total slides in that playlist.
    pub of: usize,
}

/// A playlist entry that did not resolve to exactly one slide.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct Unresolved {
    pub playlist: String,
    pub entry: String,
    pub reason: String,
}

/// The last git commit that touched a path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct GitTouch {
    /// ISO-8601 committer date.
    pub date: String,
    pub commit: String,
}

/// Slide → playlists, for a whole library. Build once, look up many.
#[derive(Debug, Default, Clone, Serialize, JsonSchema)]
pub struct UsageIndex {
    /// Keyed by the slide's library-relative path (`genai/intro.md`).
    pub slides: BTreeMap<String, Vec<PlaylistRef>>,
    pub playlists: usize,
    pub unresolved: Vec<Unresolved>,
}

impl UsageIndex {
    /// Index every playlist in `playlist_dir` against `slides`.
    /// Unreadable playlists are skipped and named in `unresolved`.
    pub fn build(playlist_dir: &Path, slides: &SlideCollection, matcher: &SldrMatcher) -> Self {
        let mut playlists: Vec<(String, Playlist)> = Vec::new();
        let mut unresolved = Vec::new();
        if let Ok(entries) = std::fs::read_dir(playlist_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_none_or(|e| e != "toml") {
                    continue;
                }
                let stem = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or_default()
                    .to_string();
                match Playlist::load(&path) {
                    Ok(p) => playlists.push((stem, p)),
                    Err(e) => unresolved.push(Unresolved {
                        playlist: stem,
                        entry: String::new(),
                        reason: format!("playlist did not load: {e}"),
                    }),
                }
            }
        }
        playlists.sort_by(|a, b| a.0.cmp(&b.0));
        Self::from_playlists(&playlists, slides, matcher, unresolved)
    }

    /// Index from already-loaded playlists; `(file_stem, playlist)` pairs.
    pub fn from_playlists(
        playlists: &[(String, Playlist)],
        slides: &SlideCollection,
        matcher: &SldrMatcher,
        mut unresolved: Vec<Unresolved>,
    ) -> Self {
        let names = slides.names();
        let mut index: BTreeMap<String, Vec<PlaylistRef>> = BTreeMap::new();
        for (stem, playlist) in playlists {
            let of = playlist.slides.len();
            for (i, entry) in playlist.slides.iter().enumerate() {
                match resolve_entry(entry, slides, &names, matcher) {
                    Ok(rel) => index.entry(rel).or_default().push(PlaylistRef {
                        name: stem.clone(),
                        title: playlist.title.clone(),
                        flavor: playlist.flavor.clone(),
                        position: i + 1,
                        of,
                    }),
                    Err(reason) => unresolved.push(Unresolved {
                        playlist: stem.clone(),
                        entry: entry.clone(),
                        reason,
                    }),
                }
            }
        }
        Self {
            slides: index,
            playlists: playlists.len(),
            unresolved,
        }
    }

    /// Playlists referencing the slide at `relative_path`.
    pub fn of(&self, relative_path: &str) -> &[PlaylistRef] {
        self.slides
            .get(relative_path)
            .map_or(&[], Vec::as_slice)
    }

    /// Slides no playlist references (candidates for "unused" badges).
    pub fn unused<'a>(&self, slides: &'a SlideCollection) -> Vec<&'a str> {
        slides
            .slides
            .iter()
            .filter(|s| !self.slides.contains_key(&s.relative_path))
            .map(|s| s.relative_path.as_str())
            .collect()
    }
}

/// Resolve one playlist entry the way a non-interactive build does.
fn resolve_entry(
    entry: &str,
    slides: &SlideCollection,
    names: &[String],
    matcher: &SldrMatcher,
) -> std::result::Result<String, String> {
    if let Some(s) = slides.find(entry) {
        return Ok(s.relative_path.clone());
    }
    match matcher.resolve(entry, names) {
        ResolveResult::Found(m) => slides
            .find(&m.value)
            .map(|s| s.relative_path.clone())
            .ok_or_else(|| "matched a name that is not in the library".to_string()),
        ResolveResult::NotFound => Err("no slide matches".to_string()),
        ResolveResult::Multiple(m) => Err(format!(
            "ambiguous: {}",
            m.iter().map(|x| x.value.as_str()).collect::<Vec<_>>().join(", ")
        )),
    }
}

/// Slides whose effective layout (`layout:` field, else `default`) is `layout`.
pub fn slides_using_layout<'a>(layout: &str, slides: &'a SlideCollection) -> Vec<&'a str> {
    slides
        .slides
        .iter()
        .filter(|s| s.metadata.layout.as_deref().unwrap_or("default") == layout)
        .map(|s| s.relative_path.as_str())
        .collect()
}

/// Last commit touching `path`, or `None` when git is absent, the path is not
/// in a repository, or the file is untracked. Never fails: history is a
/// convenience, and a library outside git is still a valid library.
pub fn git_last_touched(path: &Path) -> Option<GitTouch> {
    let dir = if path.is_dir() { path } else { path.parent()? };
    let file = path.file_name()?;
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["log", "-1", "--format=%cI%x00%h", "--"])
        .arg(file)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let line = text.lines().next()?.trim();
    let (date, commit) = line.split_once('\0')?;
    if date.is_empty() || commit.is_empty() {
        return None;
    }
    Some(GitTouch {
        date: date.to_string(),
        commit: commit.to_string(),
    })
}

/// Everything known about one slide's use.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct SlideUsage {
    pub slide: String,
    pub playlists: Vec<PlaylistRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_touched: Option<GitTouch>,
}

/// Everything known about one layout's use.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct LayoutUsage {
    pub layout: String,
    pub slides: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::MatchingConfig;
    use crate::slide::Slide;

    fn library() -> SlideCollection {
        SlideCollection {
            slides: vec![
                Slide::from_str("intro", "genai/intro.md", "---\ntitle: A\nlayout: framed\n---\nx"),
                Slide::from_str("loop", "genai/loop.md", "---\ntitle: B\n---\nx"),
                Slide::from_str("questions", "shared/questions.md", "---\nlayout: cover\n---\nx"),
            ],
            base_dir: std::path::PathBuf::from("/lib"),
        }
    }

    fn playlist(name: &str, slides: &[&str]) -> (String, Playlist) {
        (
            name.to_string(),
            Playlist {
                name: name.to_string(),
                title: None,
                description: None,
                slides: slides.iter().map(|s| s.to_string()).collect(),
                flavor: Some("byteowlz".into()),
                default_lang: None,
                render: Default::default(),
            },
        )
    }

    #[test]
    fn index_counts_references_by_relative_path() {
        let lib = library();
        let matcher = SldrMatcher::new(MatchingConfig::default());
        let pls = vec![
            playlist("talk-a", &["genai/intro", "shared/questions.md"]),
            playlist("talk-b", &["genai/intro.md", "genai/loop", "questions"]),
        ];
        let idx = UsageIndex::from_playlists(&pls, &lib, &matcher, Vec::new());
        assert_eq!(idx.playlists, 2);
        let intro = idx.of("genai/intro.md");
        assert_eq!(intro.len(), 2);
        assert_eq!(intro[0].name, "talk-a");
        assert_eq!(intro[0].position, 1);
        assert_eq!(intro[0].of, 2);
        assert_eq!(intro[1].flavor.as_deref(), Some("byteowlz"));
        assert_eq!(idx.of("shared/questions.md").len(), 2);
        assert_eq!(idx.of("genai/loop.md").len(), 1);
        assert!(idx.unresolved.is_empty(), "{:?}", idx.unresolved);
        assert!(idx.unused(&lib).is_empty());
    }

    #[test]
    fn unresolvable_entries_are_reported_not_guessed() {
        let lib = library();
        let matcher = SldrMatcher::new(MatchingConfig::default());
        let pls = vec![playlist("talk", &["genai/intro", "does-not-exist-anywhere"])];
        let idx = UsageIndex::from_playlists(&pls, &lib, &matcher, Vec::new());
        assert_eq!(idx.of("genai/intro.md").len(), 1);
        assert_eq!(idx.unresolved.len(), 1);
        assert_eq!(idx.unresolved[0].entry, "does-not-exist-anywhere");
        let unused = idx.unused(&lib);
        assert_eq!(unused, vec!["genai/loop.md", "shared/questions.md"]);
    }

    #[test]
    fn layout_usage_defaults_missing_layout_to_default() {
        let lib = library();
        assert_eq!(slides_using_layout("framed", &lib), vec!["genai/intro.md"]);
        assert_eq!(slides_using_layout("default", &lib), vec!["genai/loop.md"]);
        assert!(slides_using_layout("two-cols", &lib).is_empty());
    }

    #[test]
    fn git_touch_is_none_outside_a_repo() {
        let dir = std::env::temp_dir().join(format!("sldr-usage-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("x.md");
        std::fs::write(&f, "x").unwrap();
        // temp_dir may itself live inside a repo on a dev box; an untracked
        // file yields no commit either way.
        assert_eq!(git_last_touched(&f), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
