# Preset Composability Design

**Date**: 2026-03-07
**Status**: Proposed

## Overview

Make presets composable: users can layer multiple presets, create partial presets that only touch specific parameters, and build inheritance chains where a preset extends a base. Today `apply_preset` does full replacement — the second preset overwrites all fields, even ones it didn't intend to set. This design introduces `Option<f32>` semantics so "not specified" is distinguishable from "explicitly set to 0.0".

## Motivation

Users expect to build editing workflows by combining focused presets:
- A base look preset (exposure, contrast, white balance)
- A color grading preset (HSL adjustments only)
- A warm tint overlay (just temperature and tint)

Today, applying a "warm tint" preset that only specifies `temperature = 30.0` also resets exposure, contrast, and everything else to 0.0 because serde deserializes missing fields as defaults. This makes multi-preset workflows impossible.

## Core Problem

Serde's `#[serde(default)]` makes missing TOML fields indistinguishable from explicitly-set-to-default values. We need a representation where `None` means "this preset doesn't touch this field" and `Some(0.0)` means "this preset explicitly sets this field to zero".

## Approach: PartialParameters with Option Fields

Introduce a `PartialParameters` type where every field is `Option<T>`. This is the deserialization target for TOML presets. The existing `Parameters` type (all fields concrete) remains the engine's working type.

```rust
/// Partial parameter set — `None` means "not specified by this preset".
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PartialParameters {
    pub exposure: Option<f32>,
    pub contrast: Option<f32>,
    pub highlights: Option<f32>,
    pub shadows: Option<f32>,
    pub whites: Option<f32>,
    pub blacks: Option<f32>,
    pub temperature: Option<f32>,
    pub tint: Option<f32>,
    pub hsl: Option<PartialHslChannels>,
}
```

### PartialHslChannels and PartialHslChannel

```rust
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PartialHslChannel {
    pub hue: Option<f32>,
    pub saturation: Option<f32>,
    pub luminance: Option<f32>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PartialHslChannels {
    pub red: Option<PartialHslChannel>,
    pub orange: Option<PartialHslChannel>,
    pub yellow: Option<PartialHslChannel>,
    pub green: Option<PartialHslChannel>,
    pub aqua: Option<PartialHslChannel>,
    pub blue: Option<PartialHslChannel>,
    pub purple: Option<PartialHslChannel>,
    pub magenta: Option<PartialHslChannel>,
}
```

### Merge Semantics: Last-Write-Wins

Merging two `PartialParameters` values uses last-write-wins: for each field, if the overlay has `Some(v)`, use `v`; otherwise keep the base's value.

```rust
impl PartialParameters {
    /// Merge `other` on top of `self`. Fields in `other` that are `Some`
    /// override the corresponding field in `self`.
    pub fn merge(&self, other: &PartialParameters) -> PartialParameters { ... }
}
```

### Materialization: PartialParameters → Parameters

To get a concrete `Parameters` for the engine, materialize with defaults:

```rust
impl PartialParameters {
    /// Convert to concrete Parameters. `None` fields become their default (0.0).
    pub fn materialize(&self) -> Parameters { ... }
}
```

## Preset Changes

### TOML Format

The TOML format is unchanged. The difference is in deserialization: `PresetRaw` now uses `Option<ToneSection>`, `Option<WhiteBalanceSection>`, etc., and the section structs use `Option<f32>` fields. Missing keys deserialize as `None` instead of `0.0`.

### Preset Struct

`Preset` gains a `partial_params: PartialParameters` field alongside `params: Parameters` (which is the materialized version). The `Preset` struct always has both: the partial for merging, the concrete for direct use.

```rust
pub struct Preset {
    pub metadata: PresetMetadata,
    pub params: Parameters,           // materialized — ready for engine
    pub partial_params: PartialParameters,  // partial — for merging
    pub lut: Option<Lut3D>,
}
```

### Inheritance: `extends` Field

A new optional `extends` field in the `[metadata]` section:

```toml
[metadata]
name = "Warm Portrait"
extends = "base-portrait.toml"

[tone]
temperature = 30.0
```

When loading, the loader:
1. Reads the TOML into `PartialParameters`
2. If `extends` is present, recursively loads the base preset
3. Merges: base partial → this partial (last-write-wins)
4. Materializes the merged result into `Parameters`

Cycle detection: track visited file paths (canonicalized) during the recursive load. If a path appears twice, return `OxirawError::Preset("circular extends: ...")`.

## Engine Changes

### apply_preset (unchanged semantics)

`apply_preset` continues to do full replacement — it uses the materialized `params` from the preset. This is backward compatible.

### layer_preset (new)

New method that merges a preset's `partial_params` on top of the engine's current state:

```rust
impl Engine {
    /// Layer a preset on top of current parameters.
    /// Only fields specified in the preset are overridden.
    pub fn layer_preset(&mut self, preset: &Preset) {
        let current_partial = PartialParameters::from(&self.params);
        let merged = current_partial.merge(&preset.partial_params);
        self.params = merged.materialize();
        if preset.lut.is_some() {
            self.lut = preset.lut.clone();
        }
    }
}
```

## CLI Changes

### apply subcommand: multi-preset support

The `apply` subcommand gets a new `--presets` flag (comma-separated paths) alongside the existing `--preset` flag:

```bash
# Single preset (existing, unchanged)
oxiraw apply -i photo.jpg -o out.jpg --preset base.toml

# Multiple presets (new)
oxiraw apply -i photo.jpg -o out.jpg --presets base.toml,warm.toml,contrast.toml
```

When `--presets` is used, presets are loaded and layered left-to-right using `layer_preset`. The `--preset` and `--presets` flags are mutually exclusive.

## Dependency Changes

No new module dependencies. `PartialParameters`, `PartialHslChannel`, and `PartialHslChannels` live in `engine/mod.rs` alongside `Parameters`. The preset module already imports from engine. No architecture rule changes needed.

## Backward Compatibility

- Existing presets (all fields present) work unchanged — all `Option` fields will be `Some`, materializing to the same `Parameters` as before.
- `apply_preset` behavior is unchanged (full replacement).
- `from_toml` and `load_from_file` continue to return a `Preset` with a valid `params` field.
- The `--preset` CLI flag works exactly as before.

## Testing Strategy

1. **Unit tests for PartialParameters**: merge semantics (None + Some, Some + Some, None + None), materialize with defaults
2. **Unit tests for PartialHslChannels**: merge at channel and sub-channel level
3. **Preset deserialization tests**: partial TOML → PartialParameters with correct None/Some
4. **Inheritance tests**: single-level extends, multi-level chain, cycle detection error
5. **Engine layer_preset tests**: verify only specified fields change
6. **CLI integration tests**: --presets flag with multiple presets

## Scope

**In scope:**
- `PartialParameters`, `PartialHslChannel`, `PartialHslChannels` types
- Merge (last-write-wins) and materialize operations
- Preset deserialization to `PartialParameters`
- `extends` field with recursive loading and cycle detection
- `Engine::layer_preset` method
- CLI `--presets` comma-separated flag
- Tests for all of the above

**Out of scope:**
- Additive merge strategies (sum values)
- Preset marketplace or remote preset loading
- Preset validation CLI command
- Variables / shortcuts in presets
- Schema versioning

## Key Decisions

| Decision | Rationale |
|----------|-----------|
| `Option<f32>` over bitmask | Idiomatic Rust, works naturally with serde, no manual bookkeeping. Bitmask is fragile when fields are added. |
| `PartialParameters` separate from `Parameters` | Engine always works with concrete values. No `Option` unwrapping in hot render path. Clean separation of concerns. |
| Last-write-wins merge | Simple, predictable, matches CSS cascade. Additive merge is hard to reason about and can exceed valid ranges. |
| `extends` in `[metadata]` section | Natural place for preset-level metadata. Doesn't pollute parameter sections. |
| Comma-separated `--presets` flag | Simple and intuitive CLI syntax. Rare edge case with commas in paths is acceptable. |
| Cycle detection via visited paths set | Straightforward for local files. Can be extended for remote presets later. |
