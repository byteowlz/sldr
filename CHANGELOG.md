# Changelog

All notable changes to this project will be documented in this file.

## [0.2.0] - 2026-02-17

### Added
- JSON schema generation for all config files (config.toml, flavor.toml, skeleton.toml)
- Auto-generated example configs with inline documentation and $schema references
- `schema-gen` binary for regenerating schemas and examples from Rust structs
- `just schemas` and `just copy-schemas` recipes
- Schema copy script for publishing to byteowlz/schemas repository
- `JsonSchema` derive on all config, flavor, skeleton, and slidev structs
- IDE autocompletion support via Even Better TOML extension
- `slides derive` command for AI-assisted slide generation from input files
- JSON output support (`--json` flag) for CLI commands
- Skeleton management commands (create, add, remove slides)
- Enhanced fuzzy matching with configurable resolution order
- Bundled default templates
- Background image copying for flavors during presentation build
- GitHub Actions release workflow with Homebrew and AUR publishing
- AGENTS_USE.md usage documentation

### Changed
- Workspace Cargo.toml now explicitly lists crate members
- Improved `ls` command with richer output for slides, skeletons, and flavors
- Better search with metadata and tag filtering

### Fixed
- Correct slidev frontmatter generation for per-slide layouts
- Background image paths resolved correctly during flavor asset copying

## [0.1.1] - 2025-01-30

### Added
- sldr-server crate with HTTP API endpoints for slides, skeletons, flavors, builds, and previews
- Slidev session manager with preview and template edit support
