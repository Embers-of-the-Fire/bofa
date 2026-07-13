use crate::config::scanner::sensitive::SensitiveScannerConfig;
use crate::git::ChangedFile;
use glob::{MatchOptions, Pattern};
use serde::Serialize;

const MATCH_OPTIONS: MatchOptions = MatchOptions {
    case_sensitive: true,
    require_literal_separator: true,
    require_literal_leading_dot: false,
};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SensitiveFinding {
    pub name: String,
    pub description: String,
    pub matched_paths: Vec<String>,
    pub related_persons: Vec<String>,
}

#[derive(Debug, Clone)]
struct CompiledItem {
    name: String,
    description: String,
    patterns: Vec<Pattern>,
    members: Vec<String>,
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum Error {
    #[error("invalid glob pattern: {0}")]
    InvalidGlob(String),
}

#[derive(Debug)]
pub struct SensitiveScanner {
    items: Vec<CompiledItem>,
}

impl SensitiveScanner {
    pub fn new(config: &SensitiveScannerConfig) -> Result<Self, Error> {
        let mut items = Vec::new();
        for (name, item) in &config.item {
            let mut patterns = Vec::new();
            for path in &item.paths {
                let pattern = Pattern::new(path)
                    .map_err(|err| Error::InvalidGlob(format!("{}: {}", path, err)))?;
                patterns.push(pattern);
            }
            items.push(CompiledItem {
                name: name.clone(),
                description: item.description.clone(),
                patterns,
                members: item.members.clone(),
            });
        }
        Ok(Self { items })
    }

    pub fn scan(&self, files: &[ChangedFile]) -> Vec<SensitiveFinding> {
        let mut findings = Vec::new();
        for item in &self.items {
            let matched: Vec<String> = files
                .iter()
                .filter(|file| {
                    item.patterns
                        .iter()
                        .any(|pattern| pattern.matches_with(&file.path, MATCH_OPTIONS))
                })
                .map(|file| file.path.clone())
                .collect();
            if !matched.is_empty() {
                findings.push(SensitiveFinding {
                    name: item.name.clone(),
                    description: item.description.clone(),
                    matched_paths: matched,
                    related_persons: item.members.clone(),
                });
            }
        }
        findings
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::scanner::sensitive::SensitiveScannerItem;
    use crate::git::FileChangeStatus;
    use indexmap::IndexMap;

    fn changed_file(path: &str) -> ChangedFile {
        ChangedFile {
            path: path.to_string(),
            status: FileChangeStatus::Modified,
        }
    }

    fn config_with_items(items: IndexMap<String, SensitiveScannerItem>) -> SensitiveScannerConfig {
        SensitiveScannerConfig {
            enabled: true,
            always_report: false,
            item: items,
        }
    }

    #[test]
    fn exact_match() {
        let config = config_with_items(indexmap::indexmap! {
            "config".to_string() => SensitiveScannerItem {
                description: "config".to_string(),
                paths: vec!["src/config.rs".to_string()],
                members: vec!["alice".to_string()],
            },
        });
        let scanner = SensitiveScanner::new(&config).unwrap();
        let findings = scanner.scan(&[changed_file("src/config.rs"), changed_file("src/main.rs")]);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].name, "config");
        assert_eq!(findings[0].matched_paths, vec!["src/config.rs"]);
        assert_eq!(findings[0].related_persons, vec!["alice"]);
    }

    #[test]
    fn glob_star_matches_within_component() {
        let config = config_with_items(indexmap::indexmap! {
            "rust-sources".to_string() => SensitiveScannerItem {
                description: "rust sources".to_string(),
                paths: vec!["src/*.rs".to_string()],
                members: vec!["bob".to_string()],
            },
        });
        let scanner = SensitiveScanner::new(&config).unwrap();
        let findings = scanner.scan(&[
            changed_file("src/main.rs"),
            changed_file("src/lib.rs"),
            changed_file("src/deep/nested.rs"),
        ]);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].name, "rust-sources");
        assert_eq!(findings[0].matched_paths, vec!["src/main.rs", "src/lib.rs"]);
    }

    #[test]
    fn glob_recursive_star_matches_across_directories() {
        let config = config_with_items(indexmap::indexmap! {
            "all-rust".to_string() => SensitiveScannerItem {
                description: "all rust".to_string(),
                paths: vec!["**/*.rs".to_string()],
                members: vec!["carol".to_string()],
            },
        });
        let scanner = SensitiveScanner::new(&config).unwrap();
        let findings = scanner.scan(&[
            changed_file("src/main.rs"),
            changed_file("src/deep/nested.rs"),
            changed_file("tests/integration.rs"),
            changed_file("README.md"),
        ]);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].name, "all-rust");
        assert_eq!(
            findings[0].matched_paths,
            vec!["src/main.rs", "src/deep/nested.rs", "tests/integration.rs",]
        );
    }

    #[test]
    fn prefix_glob_matches_directory_contents() {
        let config = config_with_items(indexmap::indexmap! {
            "core".to_string() => SensitiveScannerItem {
                description: "core".to_string(),
                paths: vec!["/path/to/repo1/**".to_string()],
                members: vec!["alice".to_string(), "bob".to_string()],
            },
        });
        let scanner = SensitiveScanner::new(&config).unwrap();
        let findings = scanner.scan(&[
            changed_file("/path/to/repo1/src/main.rs"),
            changed_file("/path/to/repo10/src/main.rs"),
            changed_file("/path/to/repo1/README.md"),
        ]);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].name, "core");
        assert_eq!(
            findings[0].matched_paths,
            vec!["/path/to/repo1/src/main.rs", "/path/to/repo1/README.md"]
        );
        assert_eq!(findings[0].related_persons, vec!["alice", "bob"]);
    }

    #[test]
    fn no_match_returns_empty_findings() {
        let config = config_with_items(indexmap::indexmap! {
            "config".to_string() => SensitiveScannerItem {
                description: "config".to_string(),
                paths: vec!["src/config.rs".to_string()],
                members: vec!["alice".to_string()],
            },
        });
        let scanner = SensitiveScanner::new(&config).unwrap();
        let findings = scanner.scan(&[changed_file("src/main.rs")]);
        assert!(findings.is_empty());
    }

    #[test]
    fn multiple_items_produce_multiple_findings() {
        let config = config_with_items(indexmap::indexmap! {
            "config".to_string() => SensitiveScannerItem {
                description: "config".to_string(),
                paths: vec!["src/config.rs".to_string()],
                members: vec!["alice".to_string()],
            },
            "main".to_string() => SensitiveScannerItem {
                description: "main".to_string(),
                paths: vec!["src/main.rs".to_string()],
                members: vec!["bob".to_string()],
            },
        });
        let scanner = SensitiveScanner::new(&config).unwrap();
        let findings = scanner.scan(&[changed_file("src/config.rs"), changed_file("src/main.rs")]);
        assert_eq!(findings.len(), 2);
    }

    #[test]
    fn invalid_glob_returns_error() {
        let config = config_with_items(indexmap::indexmap! {
            "bad".to_string() => SensitiveScannerItem {
                description: "bad".to_string(),
                paths: vec!["src/[[*.rs".to_string()],
                members: vec!["alice".to_string()],
            },
        });
        let err = SensitiveScanner::new(&config).unwrap_err();
        assert!(matches!(err, Error::InvalidGlob(_)));
    }
}
