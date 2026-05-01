# Changelog

All notable changes to this crate are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

## [0.2.0] - 2026-04-29

### Changed

- Renamed the installed binary from `agx-cli` to `agx`. The crates.io package remains `agx-cli`, so `cargo install agx-cli` continues to work — only the binary on PATH changes. The subcommand interface and flag set are unchanged.

### Migration

Scripts referring to `agx-cli` by name on PATH (e.g., `agx-cli apply ...`) need updating to `agx apply ...`. After `cargo install --force agx-cli`, the old `agx-cli` binary is removed and the new `agx` binary is installed in its place.

## [0.1.0] - 2026-04-26

First public release of `agx-cli` to crates.io.

### Added

- **Subcommands.** `edit` (apply ad-hoc adjustments to a single image), `apply` (apply a preset to a single image), `multi-apply` (decode once, render the same image with multiple presets), `batch-apply` (apply one preset to a directory of images), and `batch-edit` (apply ad-hoc adjustments to a directory).
- **GPU opt-in.** `--gpu` flag on all render commands routes through the wgpu compute pipeline.
- **Parallel batch.** `batch-apply` and `batch-edit` use `--jobs N` to render multiple images concurrently (default: auto-detect CPU cores). `multi-apply` accepts `--jobs N` to run multiple preset renders concurrently for the same image (default: serial).
- **Output formats.** JPEG, PNG, TIFF.

[Unreleased]: https://github.com/zhjngli/AgX/compare/agx-cli-v0.2.0...HEAD
[0.2.0]: https://github.com/zhjngli/AgX/releases/tag/agx-cli-v0.2.0
[0.1.0]: https://github.com/zhjngli/AgX/releases/tag/agx-cli-v0.1.0
