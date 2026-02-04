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

        let query_lower = query.to_lowercase();
        let query_trimmed = query.trim_end_matches(".md");
        let mut results: Vec<MatchResult> = Vec::new();

        for candidate in candidates {
            let candidate_name = extract_name(candidate);
            let candidate_lower = candidate_name.to_lowercase();

            // Try exact match first (highest priority)
            if candidate_lower == query_lower
                || candidate_name == query_trimmed
                || candidate == query
            {
                results.push(MatchResult {
                    value: candidate.clone(),
                    score: i64::MAX,
                    match_type: MatchType::Exact,
                });
                continue;
            }

            // Try fuzzy match
            if let Some(score) = self.matcher.fuzzy_match(&candidate_lower, &query_lower) {
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
            if query.len() <= 3 && candidate_lower.contains(&query_lower) {
                // query.len() <= 3, so query.len() * 50 <= 150, fits in i64
                #[expect(clippy::cast_possible_wrap, reason = "small value <= 150, safe")]
                let substring_score = (query.len() * 50) as i64;
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

/// Extract the base name from a path (without extension)
fn extract_name(path: &str) -> &str {
    let path = path.trim_end_matches(".md");
    path.rsplit('/').next().unwrap_or(path)
}

// Note: allow unwrap in tests - they should panic on failure
#[allow(clippy::unwrap_used)]
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
}
