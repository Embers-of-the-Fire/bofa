use crate::config::Provider;
use crate::config::credentials::Credentials;

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
        let backend = super::backend::create_backend(credentials, provider).await?;
        Ok(Self { backend })
    }

    pub async fn account_metadata(&self) -> Result<super::AccountMetadata, super::Error> {
        self.backend.account_metadata().await
    }
}
