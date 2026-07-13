use crate::config::{BofaConfig, load_config};
use crate::git::backend::{GitBackend, create_backend, dry_run::DryRunBackend};
use crate::git::context::GitContext;
use crate::git::{AccountType, Error as GitError};
use crate::scanner::sensitive::SensitiveScanner;
use std::path::Path;
use thiserror::Error;
use tracing::{debug, info};

pub mod check;

#[derive(Debug, Error)]
pub enum Error {
    #[error("failed to load config: {0}")]
    Config(String),
    #[error(transparent)]
    Git(#[from] GitError),
    #[error(transparent)]
    Check(#[from] check::Error),
}

pub struct Bofa {
    config: BofaConfig,
}

impl Bofa {
    pub fn new(config: BofaConfig) -> Self {
        Self { config }
    }

    pub fn load_config(path: impl AsRef<Path>) -> Result<Self, Error> {
        let config = load_config(path.as_ref()).map_err(|err| Error::Config(err.to_string()))?;
        info!(config_path = %path.as_ref().display(), "loaded config");
        Ok(Self::new(config))
    }

    pub fn config(&self) -> &BofaConfig {
        &self.config
    }

    pub async fn authenticate_with(
        self,
        backend: Box<dyn GitBackend>,
    ) -> Result<AuthenticatedBofa, Error> {
        debug!("authenticating with provided backend");
        let backend = self.wrap_backend(backend);
        let context = GitContext::from_backend(backend);
        Ok(AuthenticatedBofa {
            config: self.config,
            context,
        })
    }

    pub async fn ensure_authenticated(self) -> Result<AuthenticatedBofa, Error> {
        info!(
            provider = ?self.config.worker.provider,
            dry_run = self.config.worker.dry_run,
            "ensuring git authentication"
        );
        let backend = create_backend(
            &self.config.credentials,
            self.config.worker.provider.clone(),
            self.config.worker.dry_run,
        )
        .await?;
        let context = GitContext::from_backend(backend);
        Ok(AuthenticatedBofa {
            config: self.config,
            context,
        })
    }

    fn wrap_backend(&self, backend: Box<dyn GitBackend>) -> Box<dyn GitBackend> {
        if self.config.worker.dry_run {
            Box::new(DryRunBackend::new(backend))
        } else {
            backend
        }
    }

    pub fn with_dry_run(mut self, dry_run: bool) -> Self {
        self.config.worker.dry_run = self.config.worker.dry_run || dry_run;
        self
    }
}

pub struct AuthenticatedBofa {
    config: BofaConfig,
    context: GitContext,
}

impl AuthenticatedBofa {
    pub fn config(&self) -> &BofaConfig {
        &self.config
    }

    pub fn context(&self) -> &GitContext {
        &self.context
    }

    pub async fn login(&self) -> Result<String, Error> {
        info!("fetching account metadata");
        let metadata = self.context.account_metadata().await?;
        let message = match metadata.account_type {
            AccountType::GitHubApp => {
                let installation = metadata
                    .installation
                    .as_ref()
                    .expect("installation metadata missing for GitHub App");
                format!(
                    "Logged in as {} (GitHub App) installed on {} ({})",
                    metadata.login, installation.login, installation.account_type
                )
            }
            _ => {
                format!(
                    "Logged in as {} ({}), id: {}",
                    metadata.login, metadata.account_type, metadata.id
                )
            }
        };
        Ok(message)
    }

    pub async fn check_pr(&self, id: u64) -> Result<check::pr::CheckPrOutput, Error> {
        let input = check::pr::PrInput::from_repository(id, &self.config.repository);
        info!(
            pr_id = id,
            owner = %input.owner,
            repo = %input.repo,
            "checking pull request"
        );
        let metadata = self
            .context
            .pull_request(&input.owner, &input.repo, input.id)
            .await?;
        let scanner_active = self.config.scanner.enabled && self.config.scanner.sensitive.enabled;
        debug!(scanner_active, "scanner active");
        let findings = if scanner_active {
            let changed_files = self
                .context
                .changed_files(&input.owner, &input.repo, input.id)
                .await?;
            info!(count = changed_files.len(), "fetched changed files");
            let scanner = SensitiveScanner::new(&self.config.scanner.sensitive)
                .map_err(check::Error::from)?;
            let findings = scanner.scan(&changed_files);
            info!(count = findings.len(), "scanner completed");
            findings
        } else {
            debug!("scanner disabled, skipping changed files");
            Vec::new()
        };
        let result = check::pr::PrCheckResult {
            metadata,
            findings,
            scanner_enabled: scanner_active,
            always_report: self.config.scanner.sensitive.always_report,
            report_template: self.config.template.scanner.sensitive.report.clone(),
            empty_report_template: self.config.template.scanner.sensitive.empty_report.clone(),
        };
        let rendered = result.render()?;
        let (posted, comment_url) = if self.config.worker.post_comments {
            if let Some(body) = &rendered {
                info!("posting comment to pull request");
                let url = self
                    .context
                    .post_comment(&input.owner, &input.repo, input.id, body)
                    .await?;
                (true, Some(url))
            } else {
                (false, None)
            }
        } else {
            (false, None)
        };
        if posted {
            info!(url = %comment_url.as_ref().unwrap(), "comment posted");
        } else if rendered.is_some() {
            info!("rendered comment, not posted");
        } else {
            info!("no sensitive changes detected");
        }
        Ok(check::pr::CheckPrOutput {
            body: rendered,
            posted,
            comment_url,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::BofaConfig;
    use crate::config::credentials::{Credentials, PersonalTokenCredentials, SecretString};
    use crate::config::scanner::sensitive::{SensitiveScannerConfig, SensitiveScannerItem};
    use crate::git::backend::mock::MockGitBackend;
    use crate::git::{ChangedFile, FileChangeStatus, PullRequestMetadata};

    fn test_config() -> BofaConfig {
        BofaConfig {
            credentials: Credentials::PersonalToken(PersonalTokenCredentials {
                token: SecretString::new("$DUMMY_TOKEN"),
            }),
            repository: crate::config::repository::RepositoryConfig {
                owner: "owner".to_string(),
                repo: "repo".to_string(),
            },
            worker: Default::default(),
            scanner: Default::default(),
            template: Default::default(),
            log: Default::default(),
        }
    }

    fn config_with_sensitive_scanner() -> BofaConfig {
        let mut config = test_config();
        config.scanner.sensitive = SensitiveScannerConfig {
            enabled: true,
            always_report: false,
            item: indexmap::indexmap! {
                "core-repo".to_string() => SensitiveScannerItem {
                    description: "Core repo".to_string(),
                    paths: vec!["/path/to/repo1/**".to_string()],
                    members: vec!["alice".to_string(), "bob".to_string()],
                },
                "other".to_string() => SensitiveScannerItem {
                    description: "Other".to_string(),
                    paths: vec!["/other/**".to_string()],
                    members: vec!["carol".to_string()],
                },
            },
        };
        config
    }

    fn changed_file(path: &str) -> ChangedFile {
        ChangedFile {
            path: path.to_string(),
            status: FileChangeStatus::Modified,
        }
    }

    #[tokio::test]
    async fn login_propagates_backend_error() {
        let backend = Box::new(MockGitBackend::with_account_metadata(|| async {
            Err(GitError::Api("boom".to_string()))
        }));
        let bofa = Bofa::new(test_config())
            .authenticate_with(backend)
            .await
            .unwrap();
        let err = bofa.login().await.unwrap_err();
        assert!(matches!(err, Error::Git(GitError::Api(_))));
    }

    #[tokio::test]
    async fn check_pr_propagates_backend_error() {
        let backend = MockGitBackend::with_account_metadata(|| async {
            Ok(crate::git::AccountMetadata {
                id: 1,
                login: "alice".to_string(),
                account_type: AccountType::User,
                installation: None,
            })
        });
        backend.set_pull_request(|| async { Err(GitError::Api("boom".to_string())) });
        let backend = Box::new(backend);
        let bofa = Bofa::new(test_config())
            .authenticate_with(backend)
            .await
            .unwrap();
        let err = bofa.check_pr(1).await.unwrap_err();
        assert!(matches!(err, Error::Git(GitError::Api(_))));
    }

    #[tokio::test]
    async fn check_pr_returns_none_when_scanner_disabled() {
        let backend = Box::new(MockGitBackend::new());
        let bofa = Bofa::new(test_config())
            .authenticate_with(backend)
            .await
            .unwrap();
        let output = bofa.check_pr(1).await.unwrap();
        assert!(output.body.is_none());
        assert!(!output.posted);
        assert!(output.comment_url.is_none());
    }

    #[tokio::test]
    async fn check_pr_reports_sensitive_files_and_related_persons() {
        let backend = MockGitBackend::new();
        backend.set_pull_request(|| async {
            Ok(PullRequestMetadata {
                number: 42,
                title: "Fix bug".to_string(),
                state: "closed".to_string(),
                author: "dave".to_string(),
                draft: false,
                url: "https://github.com/owner/repo/pull/42".to_string(),
            })
        });
        backend.set_changed_files(|| async {
            Ok(vec![
                changed_file("/path/to/repo1/src/main.rs"),
                changed_file("/other/README.md"),
            ])
        });
        let bofa = Bofa::new(config_with_sensitive_scanner())
            .authenticate_with(Box::new(backend))
            .await
            .unwrap();
        let output = bofa.check_pr(42).await.unwrap();
        let body = output.body.unwrap();
        assert!(body.contains("Core repo"));
        assert!(body.contains("/path/to/repo1/src/main.rs"));
        assert!(body.contains("alice"));
        assert!(body.contains("bob"));
        assert!(body.contains("Other"));
        assert!(body.contains("/other/README.md"));
        assert!(body.contains("carol"));
        assert!(output.posted);
        assert!(output.comment_url.is_some());
    }

    #[tokio::test]
    async fn check_pr_returns_none_when_no_sensitive_files_changed() {
        let backend = MockGitBackend::new();
        backend.set_changed_files(|| async { Ok(vec![changed_file("/unrelated/file.txt")]) });
        let bofa = Bofa::new(config_with_sensitive_scanner())
            .authenticate_with(Box::new(backend))
            .await
            .unwrap();
        let output = bofa.check_pr(42).await.unwrap();
        assert!(output.body.is_none());
        assert!(!output.posted);
        assert!(output.comment_url.is_none());
    }

    #[tokio::test]
    async fn check_pr_propagates_changed_files_error() {
        let backend = MockGitBackend::new();
        backend.set_changed_files(|| async { Err(GitError::Api("diff boom".to_string())) });
        let bofa = Bofa::new(config_with_sensitive_scanner())
            .authenticate_with(Box::new(backend))
            .await
            .unwrap();
        let err = bofa.check_pr(42).await.unwrap_err();
        assert!(matches!(err, Error::Git(GitError::Api(_))));
    }

    #[tokio::test]
    async fn dry_run_blocks_delete_branch() {
        let mut config = test_config();
        config.worker.dry_run = true;
        let bofa = Bofa::new(config)
            .authenticate_with(Box::new(MockGitBackend::new()))
            .await
            .unwrap();
        let err = bofa
            .context()
            .delete_branch("owner", "repo", "feature")
            .await
            .unwrap_err();
        assert!(matches!(err, GitError::DryRun(_)));
    }

    #[tokio::test]
    async fn dry_run_blocks_publish_release() {
        let mut config = test_config();
        config.worker.dry_run = true;
        let bofa = Bofa::new(config)
            .authenticate_with(Box::new(MockGitBackend::new()))
            .await
            .unwrap();
        let err = bofa
            .context()
            .publish_release("owner", "repo", "v1.0.0")
            .await
            .unwrap_err();
        assert!(matches!(err, GitError::DryRun(_)));
    }

    #[tokio::test]
    async fn dry_run_blocks_upload_file() {
        let mut config = test_config();
        config.worker.dry_run = true;
        let bofa = Bofa::new(config)
            .authenticate_with(Box::new(MockGitBackend::new()))
            .await
            .unwrap();
        let err = bofa
            .context()
            .upload_file("owner", "repo", "path.txt", b"content")
            .await
            .unwrap_err();
        assert!(matches!(err, GitError::DryRun(_)));
    }

    #[tokio::test]
    async fn non_fetch_actions_reach_backend_when_not_dry_run() {
        let bofa = Bofa::new(test_config())
            .authenticate_with(Box::new(MockGitBackend::new()))
            .await
            .unwrap();
        let err = bofa
            .context()
            .delete_branch("owner", "repo", "feature")
            .await
            .unwrap_err();
        assert!(matches!(err, GitError::Unsupported(_)));
    }

    #[tokio::test]
    async fn fetch_actions_work_in_dry_run() {
        let mut config = test_config();
        config.worker.dry_run = true;
        let bofa = Bofa::new(config)
            .authenticate_with(Box::new(MockGitBackend::new()))
            .await
            .unwrap();
        let output = bofa.check_pr(1).await.unwrap();
        assert!(output.body.is_none());
        assert!(!output.posted);
        assert!(output.comment_url.is_none());
    }

    #[tokio::test]
    async fn check_pr_posts_comment_when_enabled_and_sensitive_files_found() {
        let backend = MockGitBackend::new();
        backend.set_pull_request(|| async {
            Ok(PullRequestMetadata {
                number: 42,
                title: "Fix bug".to_string(),
                state: "closed".to_string(),
                author: "dave".to_string(),
                draft: false,
                url: "https://github.com/owner/repo/pull/42".to_string(),
            })
        });
        backend
            .set_changed_files(|| async { Ok(vec![changed_file("/path/to/repo1/src/main.rs")]) });
        let bofa = Bofa::new(config_with_sensitive_scanner())
            .authenticate_with(Box::new(backend))
            .await
            .unwrap();
        let output = bofa.check_pr(42).await.unwrap();
        assert!(output.body.is_some());
        assert!(output.posted);
        assert_eq!(
            output.comment_url,
            Some("https://github.com/test/repo/pull/1#issuecomment-1".to_string())
        );
    }

    #[tokio::test]
    async fn check_pr_does_not_post_comment_when_disabled() {
        let backend = MockGitBackend::new();
        backend.set_pull_request(|| async {
            Ok(PullRequestMetadata {
                number: 42,
                title: "Fix bug".to_string(),
                state: "closed".to_string(),
                author: "dave".to_string(),
                draft: false,
                url: "https://github.com/owner/repo/pull/42".to_string(),
            })
        });
        backend
            .set_changed_files(|| async { Ok(vec![changed_file("/path/to/repo1/src/main.rs")]) });
        let mut config = config_with_sensitive_scanner();
        config.worker.post_comments = false;
        let bofa = Bofa::new(config)
            .authenticate_with(Box::new(backend))
            .await
            .unwrap();
        let output = bofa.check_pr(42).await.unwrap();
        assert!(output.body.is_some());
        assert!(!output.posted);
        assert!(output.comment_url.is_none());
    }

    #[tokio::test]
    async fn check_pr_does_not_scan_when_scanner_master_disabled() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicU32, Ordering};

        let call_count = Arc::new(AtomicU32::new(0));
        let call_count_clone = Arc::clone(&call_count);
        let backend = MockGitBackend::new();
        backend.set_changed_files(move || {
            let call_count = Arc::clone(&call_count_clone);
            async move {
                call_count.fetch_add(1, Ordering::SeqCst);
                Ok(vec![changed_file("/path/to/repo1/src/main.rs")])
            }
        });
        let mut config = config_with_sensitive_scanner();
        config.scanner.enabled = false;
        let bofa = Bofa::new(config)
            .authenticate_with(Box::new(backend))
            .await
            .unwrap();
        let output = bofa.check_pr(42).await.unwrap();
        assert!(output.body.is_none());
        assert!(!output.posted);
        assert!(output.comment_url.is_none());
        assert_eq!(call_count.load(Ordering::SeqCst), 0);
    }
}
