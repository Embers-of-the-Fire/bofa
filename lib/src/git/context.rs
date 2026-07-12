use crate::config::Provider;
use crate::config::credentials::Credentials;
use tracing::{info, instrument};

pub struct GitContext {
    backend: Box<dyn super::backend::GitBackend>,
}

impl GitContext {
    pub fn from_backend(backend: Box<dyn super::backend::GitBackend>) -> Self {
        Self { backend }
    }

    pub async fn from_credentials(
        credentials: &Credentials,
        provider: Provider,
    ) -> Result<Self, super::Error> {
        info!(
            credentials = credentials.describe(),
            provider = ?provider,
            "creating git context from credentials"
        );
        let backend = super::backend::create_backend(credentials, provider).await?;
        Ok(Self { backend })
    }

    #[instrument(skip(self), err)]
    pub async fn account_metadata(&self) -> Result<super::AccountMetadata, super::Error> {
        self.backend.account_metadata().await
    }

    #[instrument(skip(self), fields(owner, repo, id), err)]
    pub async fn pull_request(
        &self,
        owner: &str,
        repo: &str,
        id: u64,
    ) -> Result<super::PullRequestMetadata, super::Error> {
        self.backend.pull_request(owner, repo, id).await
    }

    #[instrument(skip(self), fields(owner, repo, id), err)]
    pub async fn changed_files(
        &self,
        owner: &str,
        repo: &str,
        id: u64,
    ) -> Result<Vec<super::ChangedFile>, super::Error> {
        self.backend.changed_files(owner, repo, id).await
    }
}
