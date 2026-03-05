# Preset Tooling

**Category:** Preset
**Status:** Backlog

## Problem / Opportunity

As the preset format evolves, users need tools for versioning, validation, and authoring shortcuts. Schema versioning ensures old presets work with new software. Validation catches errors before processing. Variables and shortcuts reduce repetition in preset authoring.

## Key Considerations

- **Versioning**: Add a schema version field to presets for forward/backward compatibility. Not urgent while all changes are additive (`#[serde(default)]` handles missing fields). Becomes necessary on first breaking change (field rename, value range change, or structural reorganization)
- **Validation**: CLI command (`oxiraw validate preset.toml`) to check a preset against the current schema — report unknown fields, out-of-range values, missing required fields
- **Variables / shortcuts**: Named shortcuts for common parameter combinations (e.g., `$warm-tone` expands to temperature + tint values). Could use TOML's native table references or a simple variable substitution layer
- Versioning strategy: semver-style (major.minor) where major bumps indicate breaking changes and minor bumps indicate additive changes
- Migration tooling: automatic preset migration between schema versions

## Related

- [Preset Composability](preset-composability.md) — versioning is critical when presets can inherit/compose
- [Ecosystem Interop](ecosystem-interop.md) — validation helps when importing from other formats
