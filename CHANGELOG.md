# Changelog

All notable changes to this project are documented in this file.

The format is based on Keep a Changelog and this project follows Semantic Versioning.

## [Unreleased]

### Added
- GitLab CI/CD pipeline (`.gitlab-ci.yml`) with backend checks, frontend checks, Docker build, and tag-based container publish.

### Changed
- Config path handling now resolves storage paths relative to the loaded config file directory.

## [0.1.0] - 2026-04-03

### Added
- Rust/Axum backend API for repository registration, browsing, refs, blobs, commits, archives, and raw downloads.
- SvelteKit frontend for repository overview, tree navigation, blob rendering, commits list/detail, and error pages.
- Repository registry stored in JSON with file locking.
- On-request and periodic git fetch support, with in-memory caching for tree/language data.
- Docker multi-stage build and `docker-compose` setup.
- Security, contribution, support, conduct, DCO, and contribution license documents.
- Backend CLI repository commands:
  - `githree repo add`
  - `githree repo remove`
  - `githree repo fetch`
  - `githree repo list`

### Changed
- Frontend command generation is Docker-first for repository management when web management is disabled.
- Metadata synchronization keeps commit stats and repository size more consistent with fetched repository state.
- SEO baseline improved with default document title.

### Fixed
- Robust default-branch detection for generated repository add commands.
- Fail-fast behavior for CLI command generation when clone/authentication fails.
- Relative path inconsistencies between different local working directories.
