pub mod dry_run;
pub mod github;
pub mod mock;

use crate::config::Provider;
use crate::config::credentials::Credentials;
use async_trait::async_trait;
use tracing::info;

#[async_trait]
pub trait GitBackend: Send + Sync {
    async fn account_metadata(&self) -> Result<super::AccountMetadata, super::Error>;
    async fn pull_request(
        &self,
        owner: &str,
        repo: &str,
        id: u64,
    ) -> Result<super::PullRequestMetadata, super::Error>;
    async fn changed_files(
        &self,
        owner: &str,
        repo: &str,
        id: u64,
    ) -> Result<Vec<super::ChangedFile>, super::Error>;
    async fn delete_branch(
        &self,
        owner: &str,
        repo: &str,
        branch: &str,
    ) -> Result<(), super::Error>;
    async fn publish_release(&self, owner: &str, repo: &str, tag: &str)
    -> Result<(), super::Error>;
    async fn upload_file(
        &self,
        owner: &str,
        repo: &str,
        path: &str,
        content: &[u8],
    ) -> Result<(), super::Error>;
}

pub async fn create_backend(
    credentials: &Credentials,
    provider: Provider,
    dry_run: bool,
) -> Result<Box<dyn GitBackend>, super::Error> {
    info!(provider = ?provider, dry_run = dry_run, "creating git backend");
    let backend: Box<dyn GitBackend> = match provider {
        Provider::GitHub => {
            let backend = github::GitHubBackend::authenticate(credentials).await?;
            Box::new(backend)
        }
    };
    if dry_run {
        Ok(Box::new(dry_run::DryRunBackend::new(backend)))
    } else {
        Ok(backend)
    }
}
