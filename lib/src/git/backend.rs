pub mod github;
pub mod mock;

use crate::config::Provider;
use crate::config::credentials::Credentials;
use async_trait::async_trait;

#[async_trait]
pub trait GitBackend: Send + Sync {
    async fn account_metadata(&self) -> Result<super::AccountMetadata, super::Error>;
}

pub async fn create_backend(
    credentials: &Credentials,
    provider: Provider,
) -> Result<Box<dyn GitBackend>, super::Error> {
    match provider {
        Provider::GitHub => {
            let backend = github::GitHubBackend::authenticate(credentials).await?;
            Ok(Box::new(backend))
        }
    }
}
