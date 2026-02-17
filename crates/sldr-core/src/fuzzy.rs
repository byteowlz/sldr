//! Fuzzy matching utilities for slide and presentation resolution
//!
//! Implements multi-tier matching: anchor > exact > fuzzy > index > interactive

use crate::config::MatchingConfig;
use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
use tracing::debug;

/// Error when fuzzy matching fails unexpectedly
#[derive(Debug, Clone)]
pub struct MatchError {
    pub message: String,
}

/// A match result with score and metadata
#[derive(Debug, Clone)]
pub struct MatchResult {
    /// The matched item's identifier/path
    pub value: String,
    /// Match score (higher is better)
    pub score: i64,
    /// The type of match that succeeded
    pub match_type: MatchType,
}

/// Types of matches in order of priority
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchType {
    /// Exact anchor match (e.g., ^ABC123)
    Anchor,
    /// Exact string match
    Exact,
    /// Fuzzy string match
    Fuzzy,
    /// Numeric index match
    Index,
}

/// Matcher for resolving slide and presentation names
pub struct SldrMatcher {
    config: MatchingConfig,
    matcher: SkimMatcherV2,
}

impl SldrMatcher {
    /// Create a new matcher with the given configuration
    pub fn new(config: MatchingConfig) -> Self {
        Self {
            config,
            matcher: SkimMatcherV2::default(),
        }
    }

    /// Find the best match for a query among candidates
    /// Returns None if no match meets the threshold
    pub fn find_best(&self, query: &str, candidates: &[String]) -> Option<MatchResult> {
        let matches = self.find_all(query, candidates);
        matches.into_iter().next()
    }

    /// Find all matches for a query, sorted by score (highest first)
    pub fn find_all(&self, query: &str, candidates: &[String]) -> Vec<MatchResult> {
        if query.is_empty() || candidates.is_empty() {
            return Vec::new();
        }

        // Normalize query: lowercase and remove .md extension
        let query_normalized = normalize_path(query);
        let query_lower = query_normalized.to_lowercase();
        let query_name = extract_name(&query_normalized).to_lowercase();
        let mut results: Vec<MatchResult> = Vec::new();

        for candidate in candidates {
            // Normalize candidate: lowercase and remove .md extension
            let candidate_normalized = normalize_path(candidate);
            let candidate_lower = candidate_normalized.to_lowercase();
            let candidate_name = extract_name(&candidate_normalized).to_lowercase();

            // Try exact match first (highest priority)
            // Match if:
            // 1. Full paths match (e.g., "subdir/slide" == "subdir/slide")
            // 2. Query matches candidate filename (e.g., "slide" matches "subdir/slide")
            // 3. Query with path matches candidate (e.g., "subdir/slide" matches "subdir/slide.md")
            if candidate_lower == query_lower
                || candidate_name == query_lower
                || candidate_lower == query_name
            {
                results.push(MatchResult {
                    value: candidate.clone(),
                    score: i64::MAX,
                    match_type: MatchType::Exact,
                });
                continue;
            }

            // Try fuzzy match on full path and name separately
            let path_score = self.matcher.fuzzy_match(&candidate_lower, &query_lower);
            let name_score = self.matcher.fuzzy_match(&candidate_name, &query_name);

            // Use the better of path or name match
            let best_score = match (path_score, name_score) {
                (Some(p), Some(n)) => Some(std::cmp::max(p, n)),
                (Some(p), None) => Some(p),
                (None, Some(n)) => Some(n),
                (None, None) => None,
            };

            if let Some(score) = best_score {
                // Threshold is 0-100, safe to convert to i64
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "threshold is 0-100, fits in i64"
                )]
                let threshold = self.config.threshold as i64;
                if score >= threshold {
                    results.push(MatchResult {
                        value: candidate.clone(),
                        score,
                        match_type: MatchType::Fuzzy,
                    });
                }
            }

            // Substring fallback for short queries
            if query_name.len() <= 3 && candidate_name.contains(&query_name) {
                // query.len() <= 3, so query.len() * 50 <= 150, fits in i64
                #[expect(clippy::cast_possible_wrap, reason = "small value <= 150, safe")]
                let substring_score = (query_name.len() * 50) as i64;
                if !results.iter().any(|r| r.value == *candidate) {
                    results.push(MatchResult {
                        value: candidate.clone(),
                        score: substring_score,
                        match_type: MatchType::Fuzzy,
                    });
                }
            }
        }

        // Sort by match type priority, then by score
        results.sort_by(|a, b| {
            let type_order = |t: MatchType| match t {
                MatchType::Anchor => 0,
                MatchType::Exact => 1,
                MatchType::Fuzzy => 2,
                MatchType::Index => 3,
            };
            type_order(a.match_type)
                .cmp(&type_order(b.match_type))
                .then_with(|| b.score.cmp(&a.score))
        });

        // Limit to max suggestions
        results.truncate(self.config.max_suggestions);

        debug!(
            "Found {} matches for query '{}': {:?}",
            results.len(),
            query,
            results.iter().map(|r| &r.value).collect::<Vec<_>>()
        );

        results
    }

    /// Try to resolve a query to exactly one match
    /// Returns Err with suggestions if multiple matches found
    pub fn resolve(&self, query: &str, candidates: &[String]) -> ResolveResult {
        let mut matches = self.find_all(query, candidates);

        match matches.len() {
            0 => ResolveResult::NotFound,
            1 => {
                // Safe: we just checked len() == 1
                ResolveResult::Found(matches.swap_remove(0))
            }
            _ => {
                // Check if first match is significantly better
                if let (Some(first), Some(second)) = (matches.first(), matches.get(1)) {
                    if first.match_type == MatchType::Exact && second.match_type != MatchType::Exact
                    {
                        return ResolveResult::Found(first.clone());
                    }
                    // If first score is much higher, prefer it
                    if first.score > second.score.saturating_mul(2) {
                        return ResolveResult::Found(first.clone());
                    }
                }
                ResolveResult::Multiple(matches)
            }
        }
    }
}

/// Result of attempting to resolve a query
#[derive(Debug)]
pub enum ResolveResult {
    /// Exactly one match found
    Found(MatchResult),
    /// No matches found
    NotFound,
    /// Multiple matches found - user must disambiguate
    Multiple(Vec<MatchResult>),
}

/// Normalize a path by removing the .md extension
fn normalize_path(path: &str) -> String {
    path.trim_end_matches(".md").to_string()
}

/// Extract the base name from a path (without extension or directories)
fn extract_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

// Note: expect unwrap in tests - they should panic on failure
#[expect(
    clippy::unwrap_used,
    reason = "test code - panic on failure is acceptable"
)]
#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> MatchingConfig {
        MatchingConfig {
            resolution_order: vec!["exact".to_string(), "fuzzy".to_string()],
            threshold: 50.0,
            max_suggestions: 6,
        }
    }

    #[test]
    fn test_exact_match() {
        let matcher = SldrMatcher::new(test_config());
        let candidates = vec![
            "intro.md".to_string(),
            "conclusion.md".to_string(),
            "overview.md".to_string(),
        ];

        let result = matcher.find_best("intro", &candidates);
        assert!(result.is_some());
        assert_eq!(result.unwrap().value, "intro.md");
    }

    #[test]
    fn test_fuzzy_match() {
        let matcher = SldrMatcher::new(test_config());
        let candidates = vec![
            "introduction.md".to_string(),
            "conclusion.md".to_string(),
            "overview.md".to_string(),
        ];

        let result = matcher.find_best("intro", &candidates);
        assert!(result.is_some());
        assert_eq!(result.unwrap().value, "introduction.md");
    }

    #[test]
    fn test_no_match() {
        let matcher = SldrMatcher::new(test_config());
        let candidates = vec!["apple.md".to_string(), "banana.md".to_string()];

        let result = matcher.find_best("xyz123", &candidates);
        assert!(result.is_none());
    }

    #[test]
    fn test_subdir_exact_match() {
        let matcher = SldrMatcher::new(test_config());
        let candidates = vec![
            "lora/intro.md".to_string(),
            "lora/concepts.md".to_string(),
            "ai/intro.md".to_string(),
        ];

        // Should match with full path (without .md)
        let result = matcher.find_best("lora/intro", &candidates);
        assert!(result.is_some());
        assert_eq!(result.unwrap().value, "lora/intro.md");

        // Should also match with full path (with .md)
        let result = matcher.find_best("lora/intro.md", &candidates);
        assert!(result.is_some());
        assert_eq!(result.unwrap().value, "lora/intro.md");
    }

    #[test]
    fn test_subdir_name_only_match() {
        let matcher = SldrMatcher::new(test_config());
        let candidates = vec![
            "lora/concepts.md".to_string(),
            "lora/adapters.md".to_string(),
        ];

        // Should match name without directory
        let result = matcher.find_best("concepts", &candidates);
        assert!(result.is_some());
        assert_eq!(result.unwrap().value, "lora/concepts.md");
    }

    #[test]
    fn test_md_extension_normalization() {
        let matcher = SldrMatcher::new(test_config());
        let candidates = vec!["slide-one.md".to_string(), "slide-two.md".to_string()];

        // With .md extension
        let result = matcher.find_best("slide-one.md", &candidates);
        assert!(result.is_some());
        assert_eq!(result.unwrap().value, "slide-one.md");

        // Without .md extension
        let result = matcher.find_best("slide-one", &candidates);
        assert!(result.is_some());
        assert_eq!(result.unwrap().value, "slide-one.md");
    }
}
