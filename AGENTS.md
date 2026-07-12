# Agent Notes for bofa

## Project shape
- Rust workspace with two members: `lib` (`bofa-lib`) and `bin/bofa-cli`.
- CLI binary is named `bofa`, not `bofa-cli`; entry point is `bin/bofa-cli/src/main.rs`.
- No README in the repo; inspect `bofa.example.toml` and the source files to learn the config shape.

## Environment
- Nix flake provides the dev shell (`.envrc` uses `use flake`).
- CI and local checks should run via `nix develop --command <cmd>`.
- The flake sets `RUSTFLAGS="-C link-arg=-fuse-ld=lld"` and provides `rustfmt`, `clippy`, `rust-analyzer`, `gcc`, `pkg-config`, `openssl`, and `lld`.

## Verification commands
- Format: `nix develop --command cargo fmt --all -- --check`
- Lint: `nix develop --command cargo clippy --all-targets --all-features -- -D warnings`
- Test: `nix develop --command cargo test --all-features`
- CI runs these three in separate jobs; run all three locally before finishing.
- No pre-commit hooks, no task runner files, no `Makefile`/`Justfile`.

## Configuration and runtime
- Default config file is `bofa.toml` in the working directory; override with `--config <path>`.
- `bofa.example.toml` is the reference template; copy it to `bofa.toml` and fill in values.
- Secrets are configured as environment-variable names prefixed with `$` (e.g., `$APP_ID`), loaded via `dotenvy` from `.env` and resolved at runtime by `SecretString`.
- `bofa.dev.toml` is present and gitignored; it contains real credentials and must not be committed.
- `.env` is also gitignored and contains secrets; do not read or expose it.
- The CLI commands are: `bofa config`, `bofa login`, and `bofa check pr <id>`.

## Testing
- Unit tests use a mock Git backend in `lib/src/git/backend/mock.rs`; these do not hit the network.
- Integration tests are in `lib/tests/` and `bin/bofa-cli/tests/`; `bin/bofa-cli/tests/cli.rs` builds and runs the real `bofa` binary via `snapbox`.
- Run a single package or test with normal Cargo patterns, e.g., `cargo test --package bofa-lib --features ...`.

## Git / secrets
- Keep `.env`, `bofa.dev.toml`, and anything under `.direnv/` out of commits; they are gitignored by default.
