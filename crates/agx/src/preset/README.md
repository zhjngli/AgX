# preset

## Purpose
Serialize and deserialize declarative photo-editing presets as TOML files. Supports partial presets, preset layering, and inheritance via `extends`.

## Public API
- `Preset` -- holds `metadata` (`PresetMetadata`), `params` (`Parameters`), `partial_params` (`PartialParameters`), and optional `lut` (`Lut3D`)
- `PresetMetadata` -- `name`, `version`, `author`, `extends` (optional path to base preset)
- `Preset::from_toml(str)` -- parse from TOML string (LUT paths not resolved)
- `Preset::to_toml()` -- serialize to TOML string (preserves partial field distinction)
- `Preset::load_from_file(path)` -- parse from file, resolving LUT paths and `extends` chains
- `Preset::save_to_file(path)` -- serialize and write to file

## Extension Guide
To add a new adjustment parameter:
1. Add the field to `Parameters` in `engine/mod.rs`.
2. Add the corresponding `Option<f32>` field to `PartialParameters` in `engine/mod.rs`.
3. Add the field to the appropriate section struct (`ToneSection`, `WhiteBalanceSection`, or a new section) as `Option<f32>`.
4. Map the field in `build_partial_params()` and update `from_toml` / `to_toml`.

### Preset inheritance
Presets can extend a base preset via `extends` in the `[metadata]` section:
```toml
[metadata]
name = "Warm Portrait"
extends = "base-portrait.toml"

[tone]
temperature = 30.0
```
The `extends` path is resolved relative to the preset file's directory. Multi-level chains are supported. Circular inheritance is detected and returns an error.

## Does NOT
- Execute adjustments or touch pixels.
- Define what adjustment values mean -- it only stores and transfers them.
- Validate parameter ranges (the engine and adjust module own semantics).

## Key Decisions
- **Declarative, not procedural.** Presets declare parameter values, not operation sequences. This aligns with the engine's always-re-render-from-original design.
- **Option fields distinguish "not specified" from "set to zero".** `partial_params` uses `Option<f32>` so preset layering only overrides explicitly set fields. `params` is the materialized version with defaults filled in.
- **LUT paths are relative to the preset file.** `load_from_file` resolves and loads the LUT; `from_toml` cannot because it has no base directory.
- **Last-write-wins merge.** When layering presets, later presets override earlier ones for any field they specify.
