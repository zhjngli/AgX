# Preset Tooling

Schema versioning, validation, and authoring shortcuts for AgX presets.

## Sub-tasks

- [ ] **CLI validation command** — `agx-cli validate preset.toml` to check a preset against the current schema: report unknown fields, out-of-range values, missing required fields
- [ ] **`#[serde(deny_unknown_fields)]` on preset structs** — fail preset loading loudly on unknown table or field names so typos like `[tone_curves]` (vs the real `[tone_curve]`) or `lut.amount` (no such field) surface as errors rather than being silently ignored. Surfaced by the 2026-04-27 adversarial review. Behaviour change: any preset in the wild with forward-compatible unknown fields breaks. Needs a migration story — when do we want strict-by-default vs. add `#[serde(deny_unknown_fields)]` only after `agx-cli validate` lands and warns first?
- [ ] **Schema versioning** — add a schema version field to presets for forward/backward compatibility. Not urgent while all changes are additive (`#[serde(default)]` handles missing fields). Becomes necessary on first breaking change
- [ ] **Migration tooling** — automatic preset migration between schema versions when breaking changes occur
- [ ] **Variables / shortcuts** — named shortcuts for common parameter combinations (e.g., `$warm-tone` expands to temperature + tint values). Could use TOML's native table references or a simple variable substitution layer

## Considerations

- Versioning strategy: semver-style (major.minor) where major bumps indicate breaking changes and minor bumps indicate additive changes.
- Validation is the most immediately useful sub-task — catches errors before processing, helps preset authors iterate.
- Variables are lower priority — the `extends` mechanism already handles most composition needs.

## Related

- [Ecosystem Interop](ecosystem-interop.md) — validation helps catch import errors from other formats
