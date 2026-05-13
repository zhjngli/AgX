# Install

AgX is published on [crates.io](https://crates.io/crates/agx-cli) and installs with a single `cargo` command.

## System prerequisites

The CLI binary links against LibRaw (for raw decoding) and libheif (for HEIC/HEIF decoding). Library users who enable the `raw` or `heic` features also need the corresponding system library:

```bash
# macOS
brew install libraw libheif

# Debian/Ubuntu
sudo apt install libraw-dev libheif-dev
```

If you skip these, `cargo install agx-cli` fails at link time with a missing-library error.

If you only want to build the `agx-photo` library without raw or HEIC support, you can opt out of those features in your `Cargo.toml` — see [Use as a Rust library](#use-as-a-rust-library) below.

## Install the CLI

```bash
cargo install agx-cli
```

This builds the CLI and its dependencies (including the GPU render path via `wgpu`) and places the `agx` binary in `~/.cargo/bin/`. Make sure that directory is on your `PATH` — Cargo prints a one-line warning if it is not.

The first build can take a few minutes because the GPU stack is large. Build caches help on subsequent installs.

## Verify the install

```bash
agx --help
```

Expected output: a usage banner that lists the `apply`, `edit`, `batch-apply`, `batch-edit`, and `multi-apply` subcommands.

If `cargo install` succeeds but `agx --help` says "command not found", your shell hasn't picked up `~/.cargo/bin/`. Add it to your `PATH` (in `~/.bashrc` or `~/.zshrc`) and start a new shell.

## Use as a Rust library

AgX is also published as a library crate, under the name `agx-photo` on crates.io (the bare `agx` name is taken by an unrelated crate). The Rust crate name remains `agx`, so existing `use agx::...` imports work unchanged.

Add it to your `Cargo.toml`:

```toml
[dependencies]
agx-photo = "0.1"
```

With raw format support:

```toml
[dependencies]
agx-photo = { version = "0.1", features = ["raw"] }
```

With HEIC/HEIF format support:

```toml
[dependencies]
agx-photo = { version = "0.1", features = ["heic"] }
```

To enable both, list them together: `features = ["raw", "heic"]`.

Library API documentation is at [docs.rs/agx-photo](https://docs.rs/agx-photo).

## Next steps

- [Getting Started tutorial](tutorials/getting-started.md) — edit your first photo with AgX in under 10 minutes.
- [CLI reference](reference/cli.md) — every subcommand and flag.
