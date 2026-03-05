# preset

## Purpose
Serialize and deserialize declarative photo-editing presets as TOML files.

## Public API
- `Preset` -- holds `metadata` (`PresetMetadata`), `params` (`Parameters`), and optional `lut` (`Lut3D`)
- `PresetMetadata` -- `name`, `version`, `author`
- `Preset::from_toml(str)` -- parse from TOML string (LUT paths not resolved)
- `Preset::to_toml()` -- serialize to TOML string
- `Preset::load_from_file(path)` -- parse from file, resolving LUT paths relative to preset directory
- `Preset::save_to_file(path)` -- serialize and write to file

## Extension Guide
To add a new adjustment parameter:
1. Add the field to `Parameters` in `engine/mod.rs`.
2. Add the field to the appropriate section struct (`ToneSection`, `WhiteBalanceSection`, or a new section).
3. Map the field in both `from_toml` and `to_toml` (the `PresetRaw` <-> `Preset` conversion).

## Does NOT
- Execute adjustments or touch pixels.
- Define what adjustment values mean -- it only stores and transfers them.
- Validate parameter ranges (the engine and adjust module own semantics).

## Key Decisions
- **Declarative, not procedural.** Presets declare parameter values, not operation sequences. This aligns with the engine's always-re-render-from-original design.
- **Missing fields default to neutral.** All serde fields use `#[serde(default)]`, so a minimal preset with just `[tone]\nexposure = 1.0` is valid.
- **LUT paths are relative to the preset file.** `load_from_file` resolves and loads the LUT; `from_toml` cannot because it has no base directory.
