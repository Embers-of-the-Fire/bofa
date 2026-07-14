use crate::config::scanner::triage::TriageConfig;
use crate::git::ChangedFile;
use glob::Pattern;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TriageFinding {
    pub name: String,
    pub description: String,
    pub matched_paths: Vec<String>,
    pub labels: Vec<String>,
}

#[derive(Debug, Clone)]
struct CompiledGroup {
    name: String,
    description: String,
    patterns: Vec<Pattern>,
    labels: Vec<String>,
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum Error {
    #[error("invalid glob pattern: {0}")]
    InvalidGlob(String),
}

#[derive(Debug)]
pub struct TriageScanner {
    groups: Vec<CompiledGroup>,
}

impl TriageScanner {
    pub fn new(config: &TriageConfig) -> Result<Self, Error> {
        let mut groups = Vec::new();
        for (name, group) in &config.groups {
            let mut patterns = Vec::new();
            for path in &group.paths {
                let pattern = super::compile_glob(path).map_err(Error::InvalidGlob)?;
                patterns.push(pattern);
            }
            groups.push(CompiledGroup {
                name: name.clone(),
                description: group.description.clone(),
                patterns,
                labels: group.labels.clone(),
            });
        }
        Ok(Self { groups })
    }

    pub fn scan(&self, files: &[ChangedFile]) -> Vec<TriageFinding> {
        let mut findings = Vec::new();
        for group in &self.groups {
            let matched = super::matching_paths(files, &group.patterns);
            if !matched.is_empty() {
                findings.push(TriageFinding {
                    name: group.name.clone(),
                    description: group.description.clone(),
                    matched_paths: matched,
                    labels: group.labels.clone(),
                });
            }
        }
        findings
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::scanner::triage::TriageGroup;
    use crate::git::FileChangeStatus;
    use indexmap::IndexMap;

    fn changed_file(path: &str) -> ChangedFile {
        ChangedFile {
            path: path.to_string(),
            status: FileChangeStatus::Modified,
        }
    }

    fn config_with_groups(groups: IndexMap<String, TriageGroup>) -> TriageConfig {
        TriageConfig {
            enabled: true,
            post_comment: false,
            groups,
        }
    }

    #[test]
    fn exact_match() {
        let config = config_with_groups(indexmap::indexmap! {
            "config".to_string() => TriageGroup {
                description: "config".to_string(),
                paths: vec!["src/config.rs".to_string()],
                labels: vec!["triage-config".to_string()],
            },
        });
        let scanner = TriageScanner::new(&config).unwrap();
        let findings = scanner.scan(&[changed_file("src/config.rs"), changed_file("src/main.rs")]);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].name, "config");
        assert_eq!(findings[0].matched_paths, vec!["src/config.rs"]);
        assert_eq!(findings[0].labels, vec!["triage-config"]);
    }

    #[test]
    fn no_match_returns_empty_findings() {
        let config = config_with_groups(indexmap::indexmap! {
            "config".to_string() => TriageGroup {
                description: "config".to_string(),
                paths: vec!["src/config.rs".to_string()],
                labels: vec!["triage-config".to_string()],
            },
        });
        let scanner = TriageScanner::new(&config).unwrap();
        let findings = scanner.scan(&[changed_file("src/main.rs")]);
        assert!(findings.is_empty());
    }

    #[test]
    fn multiple_groups_produce_multiple_findings() {
        let config = config_with_groups(indexmap::indexmap! {
            "config".to_string() => TriageGroup {
                description: "config".to_string(),
                paths: vec!["src/config.rs".to_string()],
                labels: vec!["triage-config".to_string()],
            },
            "main".to_string() => TriageGroup {
                description: "main".to_string(),
                paths: vec!["src/main.rs".to_string()],
                labels: vec!["triage-main".to_string()],
            },
        });
        let scanner = TriageScanner::new(&config).unwrap();
        let findings = scanner.scan(&[changed_file("src/config.rs"), changed_file("src/main.rs")]);
        assert_eq!(findings.len(), 2);
    }

    #[test]
    fn invalid_glob_returns_error() {
        let config = config_with_groups(indexmap::indexmap! {
            "bad".to_string() => TriageGroup {
                description: "bad".to_string(),
                paths: vec!["src/[[*.rs".to_string()],
                labels: vec!["bad".to_string()],
            },
        });
        let err = TriageScanner::new(&config).unwrap_err();
        assert!(matches!(err, Error::InvalidGlob(_)));
    }
}
