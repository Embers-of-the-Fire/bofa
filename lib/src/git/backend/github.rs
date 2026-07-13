use crate::config::credentials::{
    AccountCredentials, AppCredentials, Credentials, KeyType, PersonalTokenCredentials,
    UserAccessTokenCredentials,
};
use crate::git::{
    AccountMetadata, AccountType, ChangedFile, Error as GitError, FileChangeStatus, IssueComment,
    PullRequestMetadata,
};
use async_trait::async_trait;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as Base64Engine;
use octocrab::Octocrab;
use octocrab::models::repos::{DiffEntry, DiffEntryStatus};
use tracing::{info, instrument, warn};

pub struct GitHubBackend {
    client: Octocrab,
    credentials: Credentials,
    app_metadata: Option<AccountMetadata>,
    installation_metadata: Option<AccountMetadata>,
}

impl GitHubBackend {
    #[instrument(skip(credentials), err)]
    pub async fn authenticate(credentials: &Credentials) -> Result<Self, GitError> {
        info!(
            credentials = credentials.describe(),
            "authenticating with GitHub"
        );
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

    #[instrument(skip(creds), err)]
    async fn authenticate_app(
        creds: &AppCredentials,
    ) -> Result<(Octocrab, AccountMetadata, AccountMetadata), GitError> {
        info!(
            app_id = %creds.app_id.name(),
            installation_id = creds.installation_id.as_ref().map(|s| s.name()),
            "authenticating GitHub App"
        );
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
            .map_err(|e| GitError::Api(format!("{e:?}")))?;

        let app: octocrab::models::App = app_client
            .get("/app", None::<&()>)
            .await
            .map_err(|e| GitError::Api(format!("{e:?}")))?;
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
                    .map_err(|e| GitError::Api(format!("{e:?}")))?
            }
            None => {
                let installations = app_client
                    .apps()
                    .installations()
                    .send()
                    .await
                    .map_err(|e| GitError::Api(format!("{e:?}")))?
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
            .map_err(|e| GitError::Api(format!("{e:?}")))?;
        Ok((client, app_metadata, installation_metadata))
    }

    #[instrument(skip(creds), err)]
    fn authenticate_account(creds: &AccountCredentials) -> Result<Octocrab, GitError> {
        info!(username = %creds.username.name(), "authenticating GitHub account");
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
            .map_err(|e| GitError::Api(format!("{e:?}")))
    }

    #[instrument(skip(creds), err)]
    fn authenticate_user_access_token(
        creds: &UserAccessTokenCredentials,
    ) -> Result<Octocrab, GitError> {
        info!("authenticating with GitHub user access token");
        let token = creds
            .token
            .resolve()
            .map_err(|e| GitError::MissingSecret(e.to_string()))?;
        Octocrab::builder()
            .user_access_token(token)
            .build()
            .map_err(|e| GitError::Api(format!("{e:?}")))
    }

    #[instrument(skip(creds), err)]
    fn authenticate_personal_token(creds: &PersonalTokenCredentials) -> Result<Octocrab, GitError> {
        info!("authenticating with GitHub personal token");
        let token = creds
            .token
            .resolve()
            .map_err(|e| GitError::MissingSecret(e.to_string()))?;
        Octocrab::builder()
            .personal_token(token)
            .build()
            .map_err(|e| GitError::Api(format!("{e:?}")))
    }
}

#[async_trait]
impl super::GitBackend for GitHubBackend {
    #[instrument(skip(self), err)]
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
                info!("fetching GitHub account metadata");
                let author = self
                    .client
                    .current()
                    .user()
                    .await
                    .map_err(|e| GitError::Api(format!("{e:?}")))?;
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

    #[instrument(skip(self), fields(owner, repo, id), err)]
    async fn pull_request(
        &self,
        owner: &str,
        repo: &str,
        id: u64,
    ) -> Result<PullRequestMetadata, GitError> {
        info!(owner, repo, id, "fetching GitHub pull request");
        let pr = self
            .client
            .pulls(owner, repo)
            .get(id)
            .await
            .map_err(|e| GitError::Api(format!("{e:?}")))?;
        let state = pr
            .state
            .map(|s| match s {
                octocrab::models::IssueState::Open => "open".to_string(),
                octocrab::models::IssueState::Closed => "closed".to_string(),
                _ => "unknown".to_string(),
            })
            .unwrap_or_else(|| "unknown".to_string());
        let author = pr
            .user
            .as_ref()
            .map(|user| user.login.clone())
            .unwrap_or_else(|| "unknown".to_string());
        let title = pr.title.unwrap_or_default();
        let url = pr.html_url.map(|url| url.to_string()).unwrap_or(pr.url);
        info!(
            number = pr.number,
            title = %title,
            state = %state,
            "fetched GitHub pull request"
        );
        Ok(PullRequestMetadata {
            number: pr.number,
            title,
            state,
            author,
            draft: pr.draft.unwrap_or(false),
            url,
        })
    }

    #[instrument(skip(self), fields(owner, repo, id), err)]
    async fn changed_files(
        &self,
        owner: &str,
        repo: &str,
        id: u64,
    ) -> Result<Vec<ChangedFile>, GitError> {
        info!(owner, repo, id, "fetching GitHub changed files");
        let first_page = self
            .client
            .pulls(owner, repo)
            .list_files(id)
            .await
            .map_err(|e| GitError::Api(format!("{e:?}")))?;

        // FIXME: GitHub's PR files endpoint returns at most 3000 files. If a PR
        // exceeds that limit, we silently truncate here. Defer a workaround
        // (e.g. commit comparison or GraphQL) until it becomes necessary.
        let entries = self
            .client
            .all_pages(first_page)
            .await
            .map_err(|e| GitError::Api(format!("{e:?}")))?;

        info!(count = entries.len(), "fetched GitHub changed files");
        Ok(entries
            .into_iter()
            .map(changed_file_from_diff_entry)
            .collect())
    }

    #[instrument(skip(self, body), fields(owner, repo, id), err)]
    async fn post_comment(
        &self,
        owner: &str,
        repo: &str,
        id: u64,
        body: &str,
    ) -> Result<String, GitError> {
        info!(owner, repo, id, "posting comment on pull request");
        let comment = self
            .client
            .issues(owner, repo)
            .create_comment(id, body)
            .await
            .map_err(|e| GitError::Api(format!("{e:?}")))?;
        let url = comment.html_url.to_string();
        info!(url = %url, "posted comment on pull request");
        Ok(url)
    }

    #[instrument(skip(self), fields(owner, repo, id), err)]
    async fn list_comments(
        &self,
        owner: &str,
        repo: &str,
        id: u64,
    ) -> Result<Vec<IssueComment>, GitError> {
        info!(owner, repo, id, "listing comments on pull request");
        let first_page = self
            .client
            .issues(owner, repo)
            .list_comments(id)
            .per_page(100)
            .send()
            .await
            .map_err(|e| GitError::Api(format!("{e:?}")))?;
        let comments = self
            .client
            .all_pages(first_page)
            .await
            .map_err(|e| GitError::Api(format!("{e:?}")))?;
        info!(count = comments.len(), "listed comments on pull request");
        Ok(comments.into_iter().map(issue_comment_from).collect())
    }

    #[instrument(skip(self, body), fields(owner, repo, comment_id), err)]
    async fn update_comment(
        &self,
        owner: &str,
        repo: &str,
        comment_id: u64,
        body: &str,
    ) -> Result<String, GitError> {
        info!(owner, repo, comment_id, "updating comment on pull request");
        let comment = self
            .client
            .issues(owner, repo)
            .update_comment(comment_id.into(), body)
            .await
            .map_err(|e| GitError::Api(format!("{e:?}")))?;
        let url = comment.html_url.to_string();
        info!(url = %url, "updated comment on pull request");
        Ok(url)
    }

    #[instrument(skip(self), fields(owner, repo, branch), err)]
    async fn delete_branch(
        &self,
        _owner: &str,
        _repo: &str,
        _branch: &str,
    ) -> Result<(), GitError> {
        warn!("delete_branch is not supported by the GitHub backend");
        Err(GitError::Unsupported("delete_branch".to_string()))
    }

    #[instrument(skip(self), fields(owner, repo, tag), err)]
    async fn publish_release(&self, _owner: &str, _repo: &str, _tag: &str) -> Result<(), GitError> {
        warn!("publish_release is not supported by the GitHub backend");
        Err(GitError::Unsupported("publish_release".to_string()))
    }

    #[instrument(skip(self, _content), fields(owner, repo, path), err)]
    async fn upload_file(
        &self,
        _owner: &str,
        _repo: &str,
        _path: &str,
        _content: &[u8],
    ) -> Result<(), GitError> {
        warn!("upload_file is not supported by the GitHub backend");
        Err(GitError::Unsupported("upload_file".to_string()))
    }
}

fn account_metadata_from_app(app: &octocrab::models::App) -> AccountMetadata {
    AccountMetadata {
        id: *app.id,
        login: app_comment_author_login(app.slug.as_deref(), &app.name),
        account_type: AccountType::GitHubApp,
        installation: None,
    }
}

fn app_comment_author_login(slug: Option<&str>, name: &str) -> String {
    match slug {
        Some(slug) => format!("{slug}[bot]"),
        None => name.to_string(),
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

fn issue_comment_from(comment: octocrab::models::issues::Comment) -> IssueComment {
    IssueComment {
        id: comment.id.into_inner(),
        body: comment.body.unwrap_or_default(),
        author_login: comment.user.login,
        url: comment.html_url.to_string(),
    }
}

fn changed_file_from_diff_entry(entry: DiffEntry) -> ChangedFile {
    ChangedFile {
        path: entry.filename,
        status: file_change_status_from(&entry.status),
    }
}

fn file_change_status_from(status: &DiffEntryStatus) -> FileChangeStatus {
    match status {
        DiffEntryStatus::Added => FileChangeStatus::Added,
        DiffEntryStatus::Removed => FileChangeStatus::Removed,
        DiffEntryStatus::Modified => FileChangeStatus::Modified,
        DiffEntryStatus::Renamed | DiffEntryStatus::Copied | DiffEntryStatus::Changed => {
            FileChangeStatus::Modified
        }
        DiffEntryStatus::Unchanged => FileChangeStatus::Unknown,
        _ => FileChangeStatus::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_diff_entry_statuses() {
        assert_eq!(
            file_change_status_from(&DiffEntryStatus::Added),
            FileChangeStatus::Added
        );
        assert_eq!(
            file_change_status_from(&DiffEntryStatus::Removed),
            FileChangeStatus::Removed
        );
        assert_eq!(
            file_change_status_from(&DiffEntryStatus::Modified),
            FileChangeStatus::Modified
        );
        assert_eq!(
            file_change_status_from(&DiffEntryStatus::Renamed),
            FileChangeStatus::Modified
        );
        assert_eq!(
            file_change_status_from(&DiffEntryStatus::Copied),
            FileChangeStatus::Modified
        );
        assert_eq!(
            file_change_status_from(&DiffEntryStatus::Changed),
            FileChangeStatus::Modified
        );
        assert_eq!(
            file_change_status_from(&DiffEntryStatus::Unchanged),
            FileChangeStatus::Unknown
        );
    }

    #[test]
    fn app_comment_author_login_uses_bot_slug() {
        assert_eq!(
            app_comment_author_login(Some("bofa"), "Bofa App"),
            "bofa[bot]"
        );
    }

    #[test]
    fn app_comment_author_login_falls_back_to_name_without_slug() {
        assert_eq!(app_comment_author_login(None, "Bofa App"), "Bofa App");
    }
}
