# Agent Notes for bofa

## Project shape
- Rust workspace (resolver = 3) with members `lib` (package `bofa-lib`) and `bin/bofa-cli` (package `bofa-cli`).
- The built binary is named `bofa`, not `bofa-cli`; entry point is `bin/bofa-cli/src/main.rs`.
- No README; use `bofa.example.toml` and source files to learn the config shape.

## Environment
- Nix flake provides the dev shell (`.envrc` uses `use flake`, but `.envrc` is gitignored).
- CI and local checks run through `nix develop --command <cmd>` so the linker, RUSTFLAGS, and toolchain match.
- The flake sets `RUSTFLAGS="-C link-arg=-fuse-ld=lld"` and provides `rustfmt`, `clippy`, `rust-analyzer`, `gcc`, `pkg-config`, `openssl`, `lld`.

## Verification
Run these in order before finishing:
1. Format: `nix develop --command cargo fmt --all -- --check`
2. Lint: `nix develop --command cargo clippy --all-targets --all-features -- -D warnings`
3. Test: `nix develop --command cargo test --all-features`
- GitHub Actions CI runs the same three commands in separate jobs (see `.github/workflows/ci.yml`).
- No pre-commit hooks, no task runner, no Makefile/Justfile.

## Configuration and runtime
- Default config file is `bofa.toml` in the working directory; override with `--config <path>`.
- `bofa.example.toml` is the reference template; copy it to `bofa.toml` and fill in values.
- Secrets are configured as environment variable names prefixed with `$` (e.g., `$APP_ID`), loaded from `.env` via `dotenvy` and resolved at runtime by `SecretString`.
- `bofa.dev.toml` and `.env` are gitignored and contain real credentials; do not read or commit them.
- CLI commands: `bofa config`, `bofa login`, `bofa check pr <id>`.
- `login` and `check pr` hit the live GitHub API, so do not run them without valid credentials.
- Run the CLI locally with `cargo run --bin bofa -- ...` (not `--bin bofa-cli`).

## Testing
- Unit tests in `lib/src/` and `lib/tests/` use the mock Git backend (`lib/src/git/backend/mock.rs`) and do not hit the network.
- `bin/bofa-cli/tests/cli.rs` builds and runs the real `bofa` binary via `snapbox`.
- Run a single package or test with normal Cargo patterns, e.g. `cargo test --package bofa-lib -- test_name`.
