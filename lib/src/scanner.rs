pub mod sensitive;
pub mod triage;

use crate::git::ChangedFile;
use glob::{MatchOptions, Pattern};

const MATCH_OPTIONS: MatchOptions = MatchOptions {
    case_sensitive: true,
    require_literal_separator: true,
    require_literal_leading_dot: false,
};

const TITLE_MATCH_OPTIONS: MatchOptions = MatchOptions {
    case_sensitive: false,
    require_literal_separator: true,
    require_literal_leading_dot: false,
};

pub(crate) fn compile_glob(pattern: &str) -> Result<Pattern, String> {
    Pattern::new(pattern).map_err(|err| format!("{pattern}: {err}"))
}

pub(crate) fn title_ignored(title: &str, patterns: &[Pattern]) -> bool {
    patterns
        .iter()
        .any(|pattern| pattern.matches_with(title, TITLE_MATCH_OPTIONS))
}

pub(crate) fn matching_paths(files: &[ChangedFile], patterns: &[Pattern]) -> Vec<String> {
    files
        .iter()
        .filter(|file| {
            patterns
                .iter()
                .any(|pattern| pattern.matches_with(&file.path, MATCH_OPTIONS))
        })
        .map(|file| file.path.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compile(patterns: &[&str]) -> Vec<Pattern> {
        patterns
            .iter()
            .map(|pattern| compile_glob(pattern).unwrap())
            .collect()
    }

    #[test]
    fn title_matches_literal_pattern_case_insensitive() {
        let patterns = compile(&["Fix Auth BUG"]);
        assert!(title_ignored("fix auth bug", &patterns));
        assert!(title_ignored("Fix Auth Bug", &patterns));
        assert!(!title_ignored("fix auth issue", &patterns));
    }

    #[test]
    fn title_matches_wildcard_pattern() {
        let patterns = compile(&["chore(deps):*"]);
        assert!(title_ignored("chore(deps): bump serde", &patterns));
        assert!(title_ignored("chore(deps): update tokio", &patterns));
        assert!(!title_ignored("feat: add login", &patterns));
    }

    #[test]
    fn title_matches_any_pattern() {
        let patterns = compile(&["dependabot*", "draft:*"]);
        assert!(title_ignored("Dependabot update", &patterns));
        assert!(title_ignored("draft: work in progress", &patterns));
        assert!(!title_ignored("feat: add feature", &patterns));
    }

    #[test]
    fn empty_patterns_never_ignore() {
        let patterns: Vec<Pattern> = Vec::new();
        assert!(!title_ignored("anything", &patterns));
    }
}
