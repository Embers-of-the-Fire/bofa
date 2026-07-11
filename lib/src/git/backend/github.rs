use crate::config::credentials::{
    AccountCredentials, AppCredentials, Credentials, KeyType, PersonalTokenCredentials,
    UserAccessTokenCredentials,
};
use crate::git::{AccountMetadata, AccountType, Error as GitError};
use async_trait::async_trait;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as Base64Engine;
use octocrab::Octocrab;

pub struct GitHubBackend {
    client: Octocrab,
    credentials: Credentials,
    app_metadata: Option<AccountMetadata>,
    installation_metadata: Option<AccountMetadata>,
}

impl GitHubBackend {
    pub async fn authenticate(credentials: &Credentials) -> Result<Self, GitError> {
        match credentials {
            Credentials::App(creds) => {
                let (client, app_metadata, installation_metadata) =
                    Self::authenticate_app(creds).await?;
                Ok(Self {
                    client,
                    credentials: credentials.clone(),
                    app_metadata: Some(app_metadata),
                    installation_metadata: Some(installation_metadata),
                })
            }
            Credentials::Account(creds) => {
                let client = Self::authenticate_account(creds)?;
                Ok(Self {
                    client,
                    credentials: credentials.clone(),
                    app_metadata: None,
                    installation_metadata: None,
                })
            }
            Credentials::UserAccessToken(creds) => {
                let client = Self::authenticate_user_access_token(creds)?;
                Ok(Self {
                    client,
                    credentials: credentials.clone(),
                    app_metadata: None,
                    installation_metadata: None,
                })
            }
            Credentials::PersonalToken(creds) => {
                let client = Self::authenticate_personal_token(creds)?;
                Ok(Self {
                    client,
                    credentials: credentials.clone(),
                    app_metadata: None,
                    installation_metadata: None,
                })
            }
        }
    }

    async fn authenticate_app(
        creds: &AppCredentials,
    ) -> Result<(Octocrab, AccountMetadata, AccountMetadata), GitError> {
        let app_id = creds
            .app_id
            .resolve()
            .map_err(|e| GitError::MissingSecret(e.to_string()))?
            .parse::<u64>()
            .map_err(|e| GitError::Authentication(e.to_string()))?
            .into();
        let key_material = creds
            .key
            .resolve()
            .map_err(|e| GitError::MissingSecret(e.to_string()))?;
        let key = match creds.key_type {
            KeyType::Pem => jsonwebtoken::EncodingKey::from_rsa_pem(key_material.as_bytes())
                .map_err(|e| GitError::Authentication(e.to_string()))?,
            KeyType::Der => {
                let bytes = Base64Engine
                    .decode(key_material)
                    .map_err(|e| GitError::Authentication(e.to_string()))?;
                jsonwebtoken::EncodingKey::from_rsa_der(&bytes)
            }
        };
        let app_client = Octocrab::builder()
            .app(app_id, key)
            .build()
            .map_err(|e| GitError::Api(e.to_string()))?;

        let app: octocrab::models::App = app_client
            .get("/app", None::<&()>)
            .await
            .map_err(|e| GitError::Api(e.to_string()))?;
        let app_metadata = account_metadata_from_app(&app);

        let installation = match creds.installation_id {
            Some(ref secret) => {
                let id = secret
                    .resolve()
                    .map_err(|e| GitError::MissingSecret(e.to_string()))?
                    .parse::<u64>()
                    .map_err(|e| GitError::Authentication(e.to_string()))?;
                let route = format!("/app/installations/{id}");
                app_client
                    .get(route, None::<&()>)
                    .await
                    .map_err(|e| GitError::Api(e.to_string()))?
            }
            None => {
                let installations = app_client
                    .apps()
                    .installations()
                    .send()
                    .await
                    .map_err(|e| GitError::Api(e.to_string()))?
                    .take_items();
                installations.into_iter().next().ok_or_else(|| {
                    GitError::Authentication("no installation found for GitHub App".to_string())
                })?
            }
        };
        let installation_id = installation.id;
        let installation_metadata = account_metadata_from_installation(&installation);

        let client = app_client
            .installation(installation_id)
            .map_err(|e| GitError::Api(e.to_string()))?;
        Ok((client, app_metadata, installation_metadata))
    }

    fn authenticate_account(creds: &AccountCredentials) -> Result<Octocrab, GitError> {
        let username = creds
            .username
            .resolve()
            .map_err(|e| GitError::MissingSecret(e.to_string()))?;
        let password = creds
            .password
            .resolve()
            .map_err(|e| GitError::MissingSecret(e.to_string()))?;
        Octocrab::builder()
            .basic_auth(username, password)
            .build()
            .map_err(|e| GitError::Api(e.to_string()))
    }

    fn authenticate_user_access_token(
        creds: &UserAccessTokenCredentials,
    ) -> Result<Octocrab, GitError> {
        let token = creds
            .token
            .resolve()
            .map_err(|e| GitError::MissingSecret(e.to_string()))?;
        Octocrab::builder()
            .user_access_token(token)
            .build()
            .map_err(|e| GitError::Api(e.to_string()))
    }

    fn authenticate_personal_token(creds: &PersonalTokenCredentials) -> Result<Octocrab, GitError> {
        let token = creds
            .token
            .resolve()
            .map_err(|e| GitError::MissingSecret(e.to_string()))?;
        Octocrab::builder()
            .personal_token(token)
            .build()
            .map_err(|e| GitError::Api(e.to_string()))
    }
}

#[async_trait]
impl super::GitBackend for GitHubBackend {
    async fn account_metadata(&self) -> Result<AccountMetadata, GitError> {
        match &self.credentials {
            Credentials::App(_) => {
                let mut app_metadata = self.app_metadata.clone().ok_or_else(|| {
                    GitError::Authentication("no app metadata found for GitHub App".to_string())
                })?;
                app_metadata.installation = self.installation_metadata.clone().map(Box::new);
                Ok(app_metadata)
            }
            _ => {
                let author = self
                    .client
                    .current()
                    .user()
                    .await
                    .map_err(|e| GitError::Api(e.to_string()))?;
                let author_account_type = account_type_from_author(&author);
                Ok(AccountMetadata {
                    id: *author.id,
                    login: author.login,
                    account_type: author_account_type,
                    installation: None,
                })
            }
        }
    }
}

fn account_metadata_from_app(app: &octocrab::models::App) -> AccountMetadata {
    AccountMetadata {
        id: *app.id,
        login: app.name.clone(),
        account_type: AccountType::GitHubApp,
        installation: None,
    }
}

fn account_metadata_from_installation(
    installation: &octocrab::models::Installation,
) -> AccountMetadata {
    let account_type = account_type_from_author(&installation.account);
    AccountMetadata {
        id: *installation.account.id,
        login: installation.account.login.clone(),
        account_type,
        installation: None,
    }
}

fn account_type_from_author(author: &octocrab::models::Author) -> AccountType {
    match author.r#type.as_str() {
        "User" => AccountType::User,
        "Organization" => AccountType::Organization,
        "Bot" => AccountType::Bot,
        other => AccountType::Other(other.to_string()),
    }
}
