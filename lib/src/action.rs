use crate::config::{BofaConfig, load_config};
use crate::git::backend::GitBackend;
use crate::git::context::GitContext;
use crate::git::{AccountType, Error as GitError};
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
        let context = GitContext::from_backend(backend);
        Ok(AuthenticatedBofa {
            config: self.config,
            context,
        })
    }

    pub async fn ensure_authenticated(self) -> Result<AuthenticatedBofa, Error> {
        let context =
            GitContext::from_credentials(&self.config.credentials, self.config.provider.clone())
                .await?;
        Ok(AuthenticatedBofa {
            config: self.config,
            context,
        })
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
        Ok(check::pr::format_pr_metadata(&metadata))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::credentials::{Credentials, PersonalTokenCredentials, SecretString};
    use crate::config::{BofaConfig, Provider};
    use crate::git::backend::mock::MockGitBackend;

    fn test_config() -> BofaConfig {
        BofaConfig {
            provider: Provider::GitHub,
            credentials: Credentials::PersonalToken(PersonalTokenCredentials {
                token: SecretString::new("$DUMMY_TOKEN"),
            }),
            repository: crate::config::repository::RepositoryConfig {
                owner: "owner".to_string(),
                repo: "repo".to_string(),
            },
            scanner: Default::default(),
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
}
