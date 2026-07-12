use crate::config::{BofaConfig, load_config};
use crate::git::context::GitContext;
use crate::git::{AccountType, Error as GitError};
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("failed to load config: {0}")]
    Config(String),
    #[error(transparent)]
    Git(#[from] GitError),
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
}
