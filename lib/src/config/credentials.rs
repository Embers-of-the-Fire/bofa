use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum Credentials {
    #[serde(rename = "app")]
    App(AppCredentials),
    #[serde(rename = "account")]
    Account(AccountCredentials),
    #[serde(rename = "user_access_token")]
    UserAccessToken(UserAccessTokenCredentials),
    #[serde(rename = "personal_token")]
    PersonalToken(PersonalTokenCredentials),
}

impl Credentials {
    pub fn describe(&self) -> String {
        match self {
            Credentials::App(c) => format!(
                "app (app_id={}, installation_id={})",
                c.app_id.name(),
                c.installation_id
                    .as_ref()
                    .map(|s| s.name())
                    .unwrap_or("none")
            ),
            Credentials::Account(c) => format!("account (username={})", c.username.name()),
            Credentials::UserAccessToken(_) => "user_access_token".to_string(),
            Credentials::PersonalToken(_) => "personal_token".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct AppCredentials {
    pub app_id: SecretString,
    pub key_type: KeyType,
    pub key: SecretString,
    #[serde(default)]
    pub installation_id: Option<SecretString>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct AccountCredentials {
    pub username: SecretString,
    pub password: SecretString,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct UserAccessTokenCredentials {
    pub token: SecretString,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct PersonalTokenCredentials {
    pub token: SecretString,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub enum KeyType {
    #[serde(rename = "DER")]
    Der,
    #[serde(rename = "PEM")]
    Pem,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(try_from = "String", into = "String")]
pub struct SecretString(String);

impl SecretString {
    pub fn new(value: impl Into<String>) -> Self {
        let value = value.into();
        Self::try_from(value).expect("invalid SecretString")
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn name(&self) -> &str {
        &self.0
    }

    pub fn resolve(&self) -> Result<String, std::env::VarError> {
        let name = self.0.strip_prefix('$').unwrap_or(&self.0);
        std::env::var(name)
    }
}

impl TryFrom<String> for SecretString {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if !value.starts_with('$') {
            return Err(format!("SecretString must start with '$', got: {value}"));
        }

        let name = &value[1..];
        if name.is_empty() {
            return Err("SecretString must contain a name after '$'".to_string());
        }

        if !name.chars().all(|c| c.is_ascii_alphabetic() || c == '_') {
            return Err(format!(
                "SecretString name must match [a-zA-Z_]+, got: {name}"
            ));
        }

        Ok(SecretString(value))
    }
}

impl From<SecretString> for String {
    fn from(value: SecretString) -> Self {
        value.0
    }
}
