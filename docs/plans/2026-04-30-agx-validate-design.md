# `agx validate` and Apply-Time Preset Warnings

**Status:** approved 2026-04-30

## Goal

Add `agx validate` as a strict, image-free correctness check for preset
authors and preset-library CI. Add unknown-field warnings to `agx apply` (and
its `multi-apply` / `batch-apply` siblings) so end users see typos surfaced as
they happen rather than silently dropped. Both ship in one PR — they share
field-detection infrastructure.

## Motivation

Today `agx apply broken-preset.toml -i img -o out.png` silently ignores
unknown fields and tables. A typo like `[tone_curves]` (vs. the real
`[tone_curve]`) parses to "no tone curve set" without warning, and the user
gets unexpected output with no indication anything went wrong. The 2026-04-27
adversarial review of the preset format (referenced in
`docs/backlog/preset-tooling.md`) flagged this as a real correctness gap.

Preset authors today have no way to confirm a preset is well-formed without
actually running `apply` against an image. Preset-library maintainers have no
way to validate a tree of presets in CI.

`agx validate` and apply-time warnings address both gaps with one shared
mechanism.

## Non-goals

- **LUT bundling** for marketplace distribution (preset + LUT in one
  artifact). Future work; depends on what "bundle" means once a preset
  marketplace exists.
- **Linter-style style hints** (warn on `exposure = 0.0` no-ops, deprecated
  field names, `extends` chains longer than 3, etc.). Defer to a separate
  `agx lint` tool if it's ever wanted.
- **Schema versioning / migration tooling.** Tracked separately in
  `docs/backlog/preset-tooling.md`. Becomes relevant on first breaking
  schema change.
- **Auto-fix** (`agx validate --fix`). YAGNI for v1.
- **Validating preset output** (no decode/render involved). Validate is
  schema/semantic, not pixel-correctness.
- **`--no-preset-warnings`** suppression flag on `apply`. Add later if
  users complain about CI noise.

## Approach

### Audience and surface

`agx validate` is a tool for preset authors and preset-library maintainers,
not part of the existing apply workflow. It's like `cargo check` versus
`cargo build` — separate command, optional, used to confirm correctness
without doing the expensive work.

`apply` (and friends) get a small UX upgrade: unknown fields surface as
warnings on stderr, but apply continues. Out-of-range values, missing LUT
files, and `extends` problems are NOT warned at apply time — they either
silently clamp (out-of-range) or already fail (LUT/extends). Apply-time
warnings are scoped to "things that silently parsed but shouldn't have."

### What `validate` checks vs. what `apply` warns on

| Check | `validate` | `apply` (after this PR) |
|---|---|---|
| Unknown fields/tables | error | warning |
| Type mismatch (`exposure = "high"`) | error | error (already — serde fails parse) |
| Out-of-range (`exposure = 99`) | error | silent clamp (engine behavior, no change) |
| Missing required field | error | error (already) |
| Missing LUT file | error | error (already, at apply time) |
| `extends` chain cycle | error | error (already) |
| `extends` chain references missing file | error | error (already) |
| Clean preset | exit 0, "ok" | render image |

### CLI surface

```bash
# Single file
agx validate looks/portra.toml

# Multiple files (shell glob expansion, not in-process)
agx validate looks/*.toml

# Quiet — only print files with errors (skip "ok" lines)
agx validate --quiet looks/*.toml

# JSON output for CI
agx validate --format=json looks/*.toml
```

Flags:

- `--quiet`, `-q` — omit "ok" lines for clean files.
- `--format <human|json>` — defaults to `human`.

Positional args: one or more `<preset.toml>` paths. No glob expansion in
our code; rely on the shell.

Exit codes:

- `0` — all files clean.
- `1` — one or more files have validation errors.
- `2` — invocation error (no files given, file not readable, etc.).

### Output format

Human-readable (default):

```
looks/broken-preset.toml: 2 problems
  error: unknown table `tone_curves` (line 12) — did you mean `tone_curve`?
  error: `tone.exposure` value 99.0 outside range [-5.0, 5.0] (line 5)

looks/clean-preset.toml: ok

2 files checked, 1 with errors
```

JSON (`--format=json`):

```json
{
  "files": [
    {
      "path": "looks/broken-preset.toml",
      "status": "error",
      "diagnostics": [
        {
          "severity": "error",
          "code": "unknown-table",
          "message": "unknown table `tone_curves` (did you mean `tone_curve`?)",
          "location": {"line": 12, "column": 1, "field": "tone_curves"}
        },
        {
          "severity": "error",
          "code": "out-of-range",
          "message": "`tone.exposure` value 99.0 outside range [-5.0, 5.0]",
          "location": {"line": 5, "column": 1, "field": "tone.exposure"}
        }
      ]
    },
    {"path": "looks/clean-preset.toml", "status": "ok", "diagnostics": []}
  ],
  "summary": {"total": 2, "ok": 1, "errors": 1}
}
```

Diagnostic codes (stable for the JSON format):

- `unknown-table`, `unknown-field`
- `type-mismatch`
- `missing-required`
- `out-of-range`
- `lut-not-found`
- `extends-not-found`, `extends-cycle`

Apply-time warnings format:

```
warning: looks/preset.toml:12: unknown table `tone_curves` (did you mean `tone_curve`?)
warning: looks/preset.toml:18: unknown field `lut.amount` in section `[lut]`
```

Printed to stderr. Apply continues. No JSON variant for apply-time warnings
in v1 — they're informational; CI workflows that need structured output
should run `agx validate` separately.

### Validation tiers (Tier 3)

| Category | Check | Source of truth |
|---|---|---|
| Structure | Unknown fields/tables | derived from preset structs (schemars) |
| Structure | Type mismatches | serde parse error |
| Structure | Missing required fields | schemars `required` |
| Semantic | Out-of-range numeric values | `schemars(range(min, max))` annotations on engine structs |
| Filesystem | LUT path resolves to existing file | `[lut] path` resolved relative to preset dir, then `Path::exists()` |
| Filesystem | `extends` chain validity | walk chain, confirm each file exists, detect cycles |

The engine already carries `#[cfg_attr(feature = "docgen", schemars(range(min, max)))]`
annotations on numeric parameters. `agx-docgen` uses these for the preset
reference markdown. **`agx validate` reuses the same annotations as the source
of truth for range checks. No duplication of bounds.** During implementation,
audit numeric fields to confirm every one has either a `schemars(range)`
annotation or an explicit "no bounds" decision; add missing annotations as
part of this PR.

### Implementation: two-pass parse

Validate parses each preset twice:

1. **Structural pass:** parse to a position-preserving representation
   (`toml_edit::DocumentMut` or `toml::de::Deserializer` with `Spanned<T>` —
   final choice deferred to implementation). Preserves source positions
   (line/column for every field) and accepts unknown fields without erroring.
   Produces unknown-field diagnostics with line numbers.

2. **Semantic pass:** validate the structure against a schemars-derived JSON
   Schema using a JSON Schema validator crate (e.g., `jsonschema`). Catches
   type mismatches, missing required fields, and `schemars(range)` violations
   against numeric values.

Each diagnostic from the semantic validator is enriched with the line number
from the structural pass via the field path.

**Why not just flip `deny_unknown_fields` on the existing structs?**

- `agx apply` would also fail loudly, breaking the lenient-apply behavior we
  want (apply warns, doesn't fail). Two struct definitions or a feature flag
  would be needed to support both. More moving parts.
- No line numbers. `serde::de::Error` carries column hints sometimes but
  isn't reliable.
- No control over diagnostic codes. Serde produces opaque error strings;
  structuring them for JSON output requires regex parsing of the error
  message. Brittle.

The two-pass approach is more code but cleaner separation: parsing
infrastructure is shared between validate and apply; the strict checks happen
separately.

### Apply-time warnings

`apply` and its siblings (`multi-apply`, `batch-apply`) call the structural
pass alongside the existing lenient parse, extract unknown-field
diagnostics, and print them to stderr. The structural pass becomes a shared
library function: `preset::detect_unknown_fields(toml_str: &str) -> Vec<Diagnostic>`.

### Where the code lives

| Component | Crate | Reason |
|---|---|---|
| Diagnostic types (`Diagnostic`, `DiagnosticCode`, `Location`, `Severity`) | `agx-photo` (lib) | External library consumers can use the validator API too |
| Structural pass (unknown field detection + position tracking) | `agx-photo` | Shared between validate and apply |
| Semantic pass (type/required/range via schemars) | `agx-photo` | Single source of truth lives in the lib |
| Filesystem pass (LUT existence, extends chain) | `agx-photo` | Already reads filesystem in `Preset::load` |
| `validate` subcommand wrapper | `agx-cli` | Thin CLI wrapper per project convention |
| Apply-time warning printing | `agx-cli` | CLI's job to print to stderr |
| Output formatters (human, JSON) | `agx-cli` | Output format is a CLI concern |

Library exposes `Preset::validate(path) -> ValidationReport`. CLI calls it,
formats output.

### New dependencies

- **`schemars`** — already a dev-only dep gated behind the `docgen` feature.
  Decision: add a new `validate` feature on `agx-photo` that pulls in
  `schemars` and `jsonschema`. CLI enables it by default. External library
  consumers can opt out if they don't want the schemars dep.
- **`jsonschema`** — new dep on `agx-photo`, gated behind `validate`.
- **`toml_edit`** vs. existing `toml` with `Spanned<T>` — choice deferred to
  implementation; `toml_edit` is more capable but heavier. Either works for
  the structural pass.

### File structure

```
crates/agx/src/
  preset/
    mod.rs                    # existing; minor changes (expose validate API)
    validate/
      mod.rs                  # public API: Preset::validate, ValidationReport
      diagnostic.rs           # Diagnostic, DiagnosticCode, Location, Severity types
      structural.rs           # position-preserving unknown-field detection
      semantic.rs             # schemars + jsonschema-based checks
      filesystem.rs           # LUT existence, extends chain validity
      tests/
        fixtures/             # broken-preset-*.toml + clean-preset.toml

crates/agx-cli/src/
  validate.rs                 # `agx validate` subcommand impl
  main.rs                     # add subcommand dispatch
  apply.rs (existing)         # add: detect_unknown_fields call, print warnings
  multi_apply.rs              # same warning addition (shared helper)
  batch_apply.rs              # same warning addition (shared helper)
  output/                     # NEW directory
    human.rs                  # human-readable formatter
    json.rs                   # JSON formatter (serde_derive on Diagnostic types)
```

`preset/validate/` is a sub-module to keep validate's internals contained
without bloating `preset/mod.rs` (already 1255 lines). Separation by concern
(structural / semantic / filesystem) keeps each file focused.

## Implementation order

Single PR on `feat/agx-validate`. Commits in order so each stands alone:

1. `chore(agx): add validate feature gate + dep skeleton` —
   `[features] validate = ["dep:schemars", "dep:jsonschema"]` in
   `agx-photo/Cargo.toml`. No code yet.
2. `feat(agx): preset validation diagnostic types` — `Diagnostic`,
   `DiagnosticCode`, `Location`, `Severity`, `ValidationReport` under
   `preset/validate/`. Unit tests for serde round-trips. No checks yet.
3. `feat(agx): structural pass — unknown field detection with line numbers` —
   implement `structural.rs`. Tests with fixture presets.
4. `feat(agx): semantic pass — type/required/range checks via schemars` —
   implement `semantic.rs`. Tests covering each diagnostic kind.
5. `feat(agx): filesystem pass — LUT existence + extends chain validity` —
   implement `filesystem.rs`. Tests with fixture preset trees.
6. `feat(agx): public Preset::validate API` — wire structural + semantic +
   filesystem passes together. Top-level integration tests.
7. `feat(agx-cli): agx validate subcommand` — clap subcommand, human + JSON
   formatters, `--quiet` and `--format` flags.
8. `feat(agx-cli): warn on unknown preset fields at apply time` — modify
   apply / multi-apply / batch-apply to call `detect_unknown_fields` and
   print to stderr.
9. `docs: agx validate reference and how-to` — new how-to page; CLI
   reference auto-regenerates via `agx-docgen`.

Each commit passes `verify.sh`. Each leaves the codebase in a working state.

After all 9 implementation commits land:

10. **Adversarial review loop.** Dispatch a fresh subagent (Agent tool, opus
    model, general-purpose subagent_type) over the full branch with context
    of what was implemented and what's already been considered. For each
    round of findings: fix all worth-fixing in a **single commit per round**
    titled `chore: round-N corrections to agx-validate`. Re-dispatch the
    reviewer after each fix commit. Continue until the review returns no
    Critical/High/Medium findings (Low/cosmetic findings can be explicitly
    accepted). One commit per round keeps the PR history readable.

After PR merges to main:

11. **Backlog cleanup.** Small follow-up commit on `main` updating
    `docs/backlog/preset-tooling.md` (mark "CLI validation command" sub-task
    `[x]`; amend the `deny_unknown_fields` bullet to reflect the
    apply-warning approach we picked — we're NOT flipping serde to strict;
    warnings happen at apply time) and `docs/backlog/README.md` (move Preset
    Tooling out of priority #1 since validate is the headline sub-task and
    the remaining ones — schema versioning, variables — are lower urgency).
    Same pattern as the PR #44 cleanup.

## Documentation impact

| File | Change | When |
|---|---|---|
| `docs/book/src/how-to/validate-preset.md` | NEW page — "Validate a preset before distributing" recipe | Commit 9 |
| `docs/book/src/SUMMARY.md` (or how-to index) | Add link to new page | Commit 9 |
| `docs/book/src/reference/cli.md` | `agx validate` entry appears | Auto-regenerated by `agx-docgen` during `verify.sh` |
| `docs/book/src/reference/preset.md` | No change | Schema unchanged |
| `docs/backlog/preset-tooling.md` | Mark sub-task done; amend `deny_unknown_fields` bullet | Post-merge (commit 11) |
| `docs/backlog/README.md` | Move Preset Tooling out of priority #1 | Post-merge (commit 11) |

Files explicitly NOT touched:

- `README.md` — intentionally trim per recent prose-explanations work; validate isn't a headline user feature.
- `ARCHITECTURE.md` — no module/dep/invariant changes; `preset/validate/` is a sub-module of existing `preset/`.
- `CLAUDE.md` — no convention changes.
- `docs/contributing/developer-workflow.md` — no workflow change.
- `docs/contributing/release-process.md` — validate doesn't change the release process.

## Testing strategy

- **Unit tests per module** in `preset/validate/tests/`. Fixture presets
  cover each diagnostic kind. Round-trip the diagnostic types through serde
  JSON to lock the output schema.
- **Integration tests** in `crates/agx-cli/tests/` invoke `agx validate`
  against fixture files and assert exit codes + output formatting (both
  human and JSON).
- **No new e2e tests** — validate doesn't render. The existing e2e suite
  isn't affected by this PR (it only validates that valid presets still
  apply correctly, which they do).

## Out of scope

Restated for clarity:

- LUT bundling for marketplace distribution.
- Linter-style style hints.
- Schema versioning and migration tooling.
- Auto-fix (`--fix`).
- Pre-commit hook scaffolding.
- `--no-preset-warnings` apply flag.

## References

- [Preset Tooling backlog](../backlog/preset-tooling.md) — origin of this work
- [Documentation Initiative](../backlog/documentation-initiative.md) — `agx-docgen` schema-driven reference
- [Release Process Design](2026-04-30-release-process-design.md) — same pattern of post-merge backlog cleanup follow-up
