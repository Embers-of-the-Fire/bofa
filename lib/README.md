# bofa-lib

Core library for the [bofa](https://github.com/Embers-of-the-Fire/bofa) GitHub PR scanner bot.

Provides the building blocks for scanning pull requests against configurable glob-based sensitive-path rules and posting templated comment reports.

## Overview

| Module | Purpose |
|--------|---------|
| `action` | High-level use cases (`Bofa` → `AuthenticatedBofa`, `login`, `check_pr`) |
| `config` | Serde-deserializable TOML configuration |
| `git` | Provider-agnostic VCS abstraction (`GitBackend` trait) |
| `git::backend` | Implementations: `GitHubBackend` (real), `MockGitBackend` (test), `DryRunBackend` (safety wrapper) |
| `scanner` | Glob-based sensitive-path matching |
| `templates` | Embedded Tera templates for report rendering |
| `logging` | `tracing_subscriber` initialization from config |

## Usage

```rust,no_run
use bofa_lib::action::Bofa;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bofa = Bofa::load_config("bofa.toml")?;
    let auth = bofa.ensure_authenticated().await?;

    let output = auth.check_pr(42).await?;
    println!("{:#?}", output);

    Ok(())
}
```

## Extending

The `GitBackend` trait defines fetch actions (`account_metadata`, `pull_request`, `changed_files`) and mutating actions (`post_comment`, `delete_branch`, `publish_release`, `upload_file`). Implement the trait to add support for other git providers (GitLab, Bitbucket, etc.).

## License

Licensed under either of [Apache License 2.0](../LICENSE-APACHE) or [MIT license](../LICENSE-MIT) at your option.
