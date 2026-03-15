# agx-cli

## Purpose
Thin CLI wrapper that exposes the agx library's functionality as command-line subcommands.

## Public API (CLI interface)
- `agx apply --input <path> --preset <path> --output <path> [--quality N] [--format fmt]` -- apply a TOML preset to an image
- `agx edit --input <path> --output <path> [--exposure N] [--contrast N] [--highlights N] [--shadows N] [--whites N] [--blacks N] [--temperature N] [--tint N] [--lut path] [--quality N] [--format fmt]` -- edit with inline parameters

Both subcommands extract metadata from the input, decode, process, render, and encode with metadata preservation.

## Extension Guide
To add a new adjustment parameter to the CLI:
1. Add the field to `Parameters` in the library.
2. Add a `#[arg]` field to the `Edit` variant of `Commands`.
3. Pass it through in `run_edit` to `engine.params_mut()`.

To add a new subcommand:
1. Add a variant to the `Commands` enum with clap `#[arg]` fields.
2. Add a `run_*` function implementing the workflow.
3. Wire it in the `match cli.command` block in `main()`.

## Does NOT
- Contain image processing logic -- delegates everything to the agx library.
- Define its own error types -- uses `agx::Result` and `agx::AgxError`.

## Key Decisions
- **Two subcommands, not flags.** `apply` (preset-driven) and `edit` (parameter-driven) are separate workflows with distinct argument sets, avoiding confusion.
- **Metadata preservation is automatic.** Both subcommands extract metadata from the input and inject it into the output with no user flags required.
