use crate::config::{BofaConfig, load_config};
use crate::git::backend::{GitBackend, create_backend, dry_run::DryRunBackend};
use crate::git::context::GitContext;
use crate::git::{AccountType, Error as GitError};
use crate::scanner::sensitive::SensitiveScanner;
use std::path::Path;
use thiserror::Error;

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
        let config = load_config(path).map_err(|err| Error::Config(err.to_string()))?;
        Ok(Self::new(config))
    }

    pub fn config(&self) -> &BofaConfig {
        &self.config
    }

    pub async fn authenticate_with(
        self,
        backend: Box<dyn GitBackend>,
    ) -> Result<AuthenticatedBofa, Error> {
        let backend = self.wrap_backend(backend);
        let context = GitContext::from_backend(backend);
        Ok(AuthenticatedBofa {
            config: self.config,
            context,
        })
    }

    pub async fn ensure_authenticated(self) -> Result<AuthenticatedBofa, Error> {
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

    pub async fn check_pr(&self, id: u64) -> Result<String, Error> {
        let input = check::pr::PrInput::from_repository(id, &self.config.repository);
        let metadata = self
            .context
            .pull_request(&input.owner, &input.repo, input.id)
            .await?;
        let scanner_enabled = self.config.scanner.sensitive.enabled;
        let findings = if scanner_enabled {
            let changed_files = self
                .context
                .changed_files(&input.owner, &input.repo, input.id)
                .await?;
            let scanner = SensitiveScanner::new(&self.config.scanner.sensitive)
                .map_err(check::Error::from)?;
            scanner.scan(&changed_files)
        } else {
            Vec::new()
        };
        let result = check::pr::PrCheckResult {
            metadata,
            findings,
            scanner_enabled,
        };
        Ok(result.to_string())
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
            log: Default::default(),
        }
    }

    fn config_with_sensitive_scanner() -> BofaConfig {
        let mut config = test_config();
        config.scanner.sensitive = SensitiveScannerConfig {
            enabled: true,
            item: vec![
                SensitiveScannerItem {
                    description: "Core repo".to_string(),
                    paths: vec!["/path/to/repo1/**".to_string()],
                    members: vec!["alice".to_string(), "bob".to_string()],
                },
                SensitiveScannerItem {
                    description: "Other".to_string(),
                    paths: vec!["/other/**".to_string()],
                    members: vec!["carol".to_string()],
                },
            ],
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
    async fn check_pr_formats_metadata() {
        let backend = Box::new(MockGitBackend::new());
        let bofa = Bofa::new(test_config())
            .authenticate_with(backend)
            .await
            .unwrap();
        let output = bofa.check_pr(1).await.unwrap();
        assert!(output.contains("#1"));
        assert!(output.contains("Test PR"));
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
        assert!(output.contains("#42 Fix bug by dave [closed]"));
        assert!(output.contains("Core repo"));
        assert!(output.contains("/path/to/repo1/src/main.rs"));
        assert!(output.contains("alice"));
        assert!(output.contains("bob"));
        assert!(output.contains("Other"));
        assert!(output.contains("/other/README.md"));
        assert!(output.contains("carol"));
    }

    #[tokio::test]
    async fn check_pr_reports_no_sensitive_files_changed() {
        let backend = MockGitBackend::new();
        backend.set_changed_files(|| async { Ok(vec![changed_file("/unrelated/file.txt")]) });
        let bofa = Bofa::new(config_with_sensitive_scanner())
            .authenticate_with(Box::new(backend))
            .await
            .unwrap();
        let output = bofa.check_pr(42).await.unwrap();
        assert!(output.contains("#1 Test PR by octocat [open]"));
        assert!(output.contains("No sensitive files changed."));
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
        assert!(output.contains("#1"));
    }
}
