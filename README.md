# Bofa

A GitHub pull request bot that scans PRs for changes to sensitive files and posts templated review comments.

## Features

- **Sensitive path scanning** — define groups of glob patterns (e.g. `src/auth/**`, `*.sql`) and bofa flags matching file changes in PRs
- **Per-group CC lists** — automatically mention the right owners for each sensitive area
- **Templated reports** — Markdown reports rendered via [Tera](https://keats.github.io/tera/), fully customizable without recompiling
- **Four auth modes** — GitHub App (PEM/DER), basic auth, user access token, or personal access token
- **Secrets via env vars** — config stores `$VAR_NAME` references, resolved at runtime with `.env` support
- **Dry-run mode** — `--dry-run` blocks all mutating actions; safe for testing against live data
- **Structured logging** — configurable `tracing` output (full, compact, pretty, JSON)

## Installation

```sh
cargo install bofa-cli
```

Or build from source:

```sh
cargo build --release
```

## Quick Start

1. Create a `bofa.toml` config file (see `bofa.example.toml` for the full reference):

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

1. Set your secret in the environment (or a `.env` file):

```sh
export GITHUB_TOKEN=ghp_...
```

1. Check a pull request:

```sh
bofa check pr 42
```

## CLI Commands

```
bofa config            # Parse and print the resolved config
bofa login             # Verify credentials and show account info
bofa check pr <id>     # Scan a PR and post/print the report
```

Global flags:

- `--config <PATH>` — config file path (default: `bofa.toml`)
- `--dry-run` — block all mutating actions (comment posting, etc.)

## Configuration

See [`bofa.example.toml`](bofa.example.toml) for the complete annotated reference.

Key sections:

| Section | Purpose |
|---------|---------|
| `[repository]` | Target GitHub repo (required) |
| `[credentials]` | Auth method and secrets (required) |
| `[scanner.sensitive.groups.<name>]` | Named glob groups with descriptions and CC lists |
| `[template]` | Override report/empty-report/footnote Tera templates |
| `[worker]` | Dry-run and post-comments toggles |
| `[log]` | Tracing level, format, and enable/disable |

## Workspace

| Crate | Description |
|-------|-------------|
| [`bofa-lib`](lib/) | Core library — scanner, config, GitHub backend, templates |
| [`bofa-cli`](bin/bofa-cli/) | CLI binary (`bofa`) built on top of `bofa-lib` |

## License

Licensed under either of [Apache License 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT) at your option.
