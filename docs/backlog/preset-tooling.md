# Preset Tooling

Schema versioning, validation, and authoring shortcuts for AgX presets.

## Sub-tasks

- [x] **CLI validation command** — `agx validate preset.toml` ships with structural / semantic / filesystem passes covering unknown fields/tables, type mismatches, missing required fields, out-of-range values, LUT existence, `extends` chain validity, and TOML syntax errors. Human + JSON output. Design: [`docs/plans/2026-04-30-agx-validate-design.md`](../plans/2026-04-30-agx-validate-design.md).
- [x] **Apply-time warnings on unknown fields** — `agx apply` / `multi-apply` / `batch-apply` now emit stderr warnings for unknown fields/tables (top-level and nested) before loading the preset. Apply continues — output is still produced. Resolves the "silent ignore on typos" gap from the 2026-04-27 adversarial review without flipping `#[serde(deny_unknown_fields)]` globally; lenient apply + strict validate covers the same surface without breaking existing presets.
- [ ] **Schema versioning** — add a schema version field to presets for forward/backward compatibility. Not urgent while all changes are additive (`#[serde(default)]` handles missing fields). Becomes necessary on the first breaking change — or sooner if a preset marketplace ships, since distributed presets need a version authors and consumers can trust (see [Platform and Distribution](platform-and-distribution.md)).
- [ ] **Migration tooling** — automatic preset migration between schema versions when breaking changes occur.
- [ ] **Variables / shortcuts** — named shortcuts for common parameter combinations (e.g., `$warm-tone` expands to temperature + tint values). Could use TOML's native table references or a simple variable substitution layer.

## Considerations

- Versioning strategy: semver-style (major.minor) where major bumps indicate breaking changes and minor bumps indicate additive changes.
- Validation is the most immediately useful sub-task — catches errors before processing, helps preset authors iterate.
- Variables are lower priority — the `extends` mechanism already handles most composition needs.

## Related

- [Ecosystem Interop](ecosystem-interop.md) — validation helps catch import errors from other formats
- [Platform and Distribution](platform-and-distribution.md) — a preset marketplace makes schema versioning urgent
