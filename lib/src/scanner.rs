pub mod sensitive;
pub mod triage;

use crate::git::ChangedFile;
use glob::{MatchOptions, Pattern};

const MATCH_OPTIONS: MatchOptions = MatchOptions {
    case_sensitive: true,
    require_literal_separator: true,
    require_literal_leading_dot: false,
};

pub(crate) fn compile_glob(pattern: &str) -> Result<Pattern, String> {
    Pattern::new(pattern).map_err(|err| format!("{pattern}: {err}"))
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
