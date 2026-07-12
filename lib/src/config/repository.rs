use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct RepositoryConfig {
    pub owner: String,
    pub repo: String,
}

impl RepositoryConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.owner.is_empty() {
            return Err("repository.owner must not be empty".to_string());
        }
        if self.repo.is_empty() {
            return Err("repository.repo must not be empty".to_string());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_owner() {
        let config = RepositoryConfig {
            owner: "".to_string(),
            repo: "repo".to_string(),
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn rejects_empty_repo() {
        let config = RepositoryConfig {
            owner: "owner".to_string(),
            repo: "".to_string(),
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn accepts_valid_owner_and_repo() {
        let config = RepositoryConfig {
            owner: "owner".to_string(),
            repo: "repo".to_string(),
        };
        assert!(config.validate().is_ok());
    }
}
