# Preset Composability

**Category:** Preset
**Status:** Backlog

## Problem / Opportunity

Today `apply_preset` does full replacement — the second preset overwrites all fields from the first, even if they weren't explicitly set. Users expect to layer presets: a base look + a warm tint + high contrast. True composability requires distinguishing "explicitly set to 0.0" from "not specified" in the TOML.

## Key Considerations

- **Core problem**: Serde deserializes missing fields as defaults (via `#[serde(default)]`), making it impossible to distinguish "user set exposure = 0.0" from "preset doesn't touch exposure"
- **Approach options**:
  - `Option<f32>` for every field — `None` means "not specified", `Some(0.0)` means "explicitly zero"
  - A separate "which fields are present" bitmask alongside the parameters
  - A custom serde visitor that tracks which keys appeared during deserialization
- **Composable presets**: Apply multiple presets in order, each only overriding its specified fields
- **Partial presets**: Presets that only touch certain parameter groups (e.g., only color grading). Same underlying problem — needs explicit-vs-default distinction
- **Preset inheritance**: A preset can `extends: "base-preset.toml"` and override specific values. Requires resolving inheritance chains and detecting cycles
- Backward compatibility: existing presets (all fields present) should continue to work unchanged

## Related

- [Preset Tooling](preset-tooling.md) — versioning and validation support composability
- [Ecosystem Interop](ecosystem-interop.md) — import/export must handle partial parameter sets
