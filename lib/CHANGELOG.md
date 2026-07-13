# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/Embers-of-the-Fire/bofa/releases/tag/bofa-lib-v0.1.0) - 2026-07-13

### Added

- *(lib)* append configurable footnote to posted PR comments
- *(lib, cli)* post templated check_pr report as PR comment
- *(scanner, config)* use named map for sensitive scanner items
- *(worker, backend)* introduce dry-run and move provider to [worker]
- *(lib, cli)* add tracing logging and log config
- *(lib, scanner, git)* implement sensitive file scan and GitHub changed files backend
- *(lib, cli)* add bofa check pr command and repository config
- *(lib, cli)* add mock git backend and test login at library level
- *(lib, cli)* add action module and Bofa context
- *(cli, lib)* add login command, dotenv support, and git abstraction layer
- *(config/credentials)* add credential system with SecretString
- *(cli)* add configuration file support and config subcommand
- *(config)* add sensitive scanner configuration

### Other

- *(release)* add release-plz for crates.io and GitHub releases
- init repo and add licenses
