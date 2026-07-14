# bofa-cli

Command-line interface for the [bofa](https://github.com/Embers-of-the-Fire/bofa) GitHub PR scanner.

Scans pull requests for changes to sensitive files (configured via glob patterns) and posts templated review comments.

## Installation

```sh
cargo install bofa-cli
```

The binary is named `bofa`.

## Usage

```
bofa config            # Parse and print the resolved config
bofa login             # Verify credentials and show account info
bofa check pr <id>     # Scan a PR and post/print the report
```

### Flags

- `--config <PATH>` — config file path (default: `bofa.toml`)
- `--dry-run` — block all mutating actions (comment posting, etc.)

## Configuration

Create a `bofa.toml` file. See [`bofa.example.toml`](https://github.com/Embers-of-the-Fire/bofa/blob/main/bofa.example.toml) for the full annotated reference.

Minimal example:

```toml
[repository]
owner = "my-org"
repo = "my-repo"

[credentials]
type = "personal_token"
token = "$GITHUB_TOKEN"

[scanner.sensitive.groups.auth]
description = "Authentication code"
paths = ["src/auth/**"]
members = ["@alice", "@bob"]
```

Secrets are referenced as `$VAR_NAME` and resolved from environment variables at runtime (with `.env` file support via `dotenvy`).

## License

Licensed under either of [Apache License 2.0](https://github.com/Embers-of-the-Fire/bofa/blob/main/LICENSE-APACHE) or [MIT license](https://github.com/Embers-of-the-Fire/bofa/blob/main/LICENSE-MIT) at your option.
