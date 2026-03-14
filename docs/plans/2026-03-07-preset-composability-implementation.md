# Preset Composability Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make presets composable with `Option<f32>` semantics, last-write-wins merge, preset inheritance via `extends`, and CLI `--presets` support.

**Architecture:** `PartialParameters` with `Option<T>` fields lives in `engine/mod.rs` alongside `Parameters`. Preset module deserializes to `PartialParameters`, then materializes to `Parameters`. Engine gets `layer_preset`. No new module dependencies.

**Tech Stack:** serde (Option deserialization), toml, clap 4

---

## Context

Design doc: `docs/plans/2026-03-07-preset-composability-design.md`

Key constraint: `Parameters` stays concrete (no Options) for the render path. `PartialParameters` is a separate type used for deserialization and merging.

## Critical Files

| File | Purpose |
|------|---------|
| `crates/oxiraw/src/engine/mod.rs` | Add PartialParameters, PartialHslChannel, PartialHslChannels, conversions, layer_preset |
| `crates/oxiraw/src/preset/mod.rs` | Switch deserialization to Option fields, add extends support, populate partial_params |
| `crates/oxiraw/src/lib.rs` | Add re-exports for new types |
| `crates/oxiraw-cli/src/main.rs` | Add --presets flag to apply subcommand |
| `crates/oxiraw/src/engine/README.md` | Document new types and layer_preset |
| `crates/oxiraw/src/preset/README.md` | Document extends and partial preset support |

---

## Phase 1: Partial Data Model

### Task 1.1: PartialHslChannel and PartialHslChannels

**Files:**
- Modify: `crates/oxiraw/src/engine/mod.rs`

**Step 1: Write failing tests**

Add to the existing `tests` module in `engine/mod.rs`:

```rust
#[test]
fn partial_hsl_channel_default_is_all_none() {
    let ch = super::PartialHslChannel::default();
    assert_eq!(ch.hue, None);
    assert_eq!(ch.saturation, None);
    assert_eq!(ch.luminance, None);
}

#[test]
fn partial_hsl_channels_default_is_all_none() {
    let hsl = super::PartialHslChannels::default();
    assert_eq!(hsl.red, None);
    assert_eq!(hsl.green, None);
    assert_eq!(hsl.blue, None);
}

#[test]
fn partial_hsl_channel_merge_overlay_wins() {
    let base = super::PartialHslChannel { hue: Some(10.0), saturation: Some(20.0), luminance: None };
    let overlay = super::PartialHslChannel { hue: Some(30.0), saturation: None, luminance: Some(5.0) };
    let merged = base.merge(&overlay);
    assert_eq!(merged.hue, Some(30.0));
    assert_eq!(merged.saturation, Some(20.0));
    assert_eq!(merged.luminance, Some(5.0));
}

#[test]
fn partial_hsl_channels_merge_channel_level() {
    let mut base = super::PartialHslChannels::default();
    base.red = Some(super::PartialHslChannel { hue: Some(10.0), saturation: None, luminance: None });
    let mut overlay = super::PartialHslChannels::default();
    overlay.red = Some(super::PartialHslChannel { hue: None, saturation: Some(20.0), luminance: None });
    overlay.green = Some(super::PartialHslChannel { hue: Some(5.0), saturation: None, luminance: None });
    let merged = base.merge(&overlay);
    // Red: base hue + overlay saturation
    assert_eq!(merged.red.as_ref().unwrap().hue, Some(10.0));
    assert_eq!(merged.red.as_ref().unwrap().saturation, Some(20.0));
    // Green: only from overlay
    assert_eq!(merged.green.as_ref().unwrap().hue, Some(5.0));
    // Blue: untouched
    assert_eq!(merged.blue, None);
}

#[test]
fn partial_hsl_channel_materialize() {
    let partial = super::PartialHslChannel { hue: Some(15.0), saturation: None, luminance: Some(-10.0) };
    let concrete = partial.materialize();
    assert_eq!(concrete.hue, 15.0);
    assert_eq!(concrete.saturation, 0.0);
    assert_eq!(concrete.luminance, -10.0);
}

#[test]
fn partial_hsl_channels_materialize() {
    let mut partial = super::PartialHslChannels::default();
    partial.red = Some(super::PartialHslChannel { hue: Some(15.0), saturation: None, luminance: None });
    let concrete = partial.materialize();
    assert_eq!(concrete.red.hue, 15.0);
    assert_eq!(concrete.red.saturation, 0.0);
    assert_eq!(concrete.green, super::HslChannel::default());
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p oxiraw engine::tests::partial_hsl`
Expected: FAIL (types don't exist yet)

**Step 3: Write implementation**

Add after `HslChannels` impl block in `engine/mod.rs`:

```rust
/// Partial per-channel HSL adjustment — `None` means "not specified".
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PartialHslChannel {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hue: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub saturation: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub luminance: Option<f32>,
}

impl PartialHslChannel {
    /// Merge overlay on top of self (last-write-wins).
    pub fn merge(&self, overlay: &Self) -> Self {
        Self {
            hue: overlay.hue.or(self.hue),
            saturation: overlay.saturation.or(self.saturation),
            luminance: overlay.luminance.or(self.luminance),
        }
    }

    /// Convert to concrete HslChannel. None fields become 0.0.
    pub fn materialize(&self) -> HslChannel {
        HslChannel {
            hue: self.hue.unwrap_or(0.0),
            saturation: self.saturation.unwrap_or(0.0),
            luminance: self.luminance.unwrap_or(0.0),
        }
    }
}

impl From<&HslChannel> for PartialHslChannel {
    fn from(ch: &HslChannel) -> Self {
        Self {
            hue: Some(ch.hue),
            saturation: Some(ch.saturation),
            luminance: Some(ch.luminance),
        }
    }
}

/// Partial HSL adjustments for all 8 channels — `None` means channel not specified.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PartialHslChannels {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub red: Option<PartialHslChannel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub orange: Option<PartialHslChannel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub yellow: Option<PartialHslChannel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub green: Option<PartialHslChannel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aqua: Option<PartialHslChannel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blue: Option<PartialHslChannel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub purple: Option<PartialHslChannel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub magenta: Option<PartialHslChannel>,
}

impl PartialHslChannels {
    fn merge_channel(base: &Option<PartialHslChannel>, overlay: &Option<PartialHslChannel>) -> Option<PartialHslChannel> {
        match (base, overlay) {
            (None, None) => None,
            (Some(b), None) => Some(b.clone()),
            (None, Some(o)) => Some(o.clone()),
            (Some(b), Some(o)) => Some(b.merge(o)),
        }
    }

    /// Merge overlay on top of self (last-write-wins per field).
    pub fn merge(&self, overlay: &Self) -> Self {
        Self {
            red: Self::merge_channel(&self.red, &overlay.red),
            orange: Self::merge_channel(&self.orange, &overlay.orange),
            yellow: Self::merge_channel(&self.yellow, &overlay.yellow),
            green: Self::merge_channel(&self.green, &overlay.green),
            aqua: Self::merge_channel(&self.aqua, &overlay.aqua),
            blue: Self::merge_channel(&self.blue, &overlay.blue),
            purple: Self::merge_channel(&self.purple, &overlay.purple),
            magenta: Self::merge_channel(&self.magenta, &overlay.magenta),
        }
    }

    /// Convert to concrete HslChannels. None channels/fields become default (0.0).
    pub fn materialize(&self) -> HslChannels {
        HslChannels {
            red: self.red.as_ref().map(|c| c.materialize()).unwrap_or_default(),
            orange: self.orange.as_ref().map(|c| c.materialize()).unwrap_or_default(),
            yellow: self.yellow.as_ref().map(|c| c.materialize()).unwrap_or_default(),
            green: self.green.as_ref().map(|c| c.materialize()).unwrap_or_default(),
            aqua: self.aqua.as_ref().map(|c| c.materialize()).unwrap_or_default(),
            blue: self.blue.as_ref().map(|c| c.materialize()).unwrap_or_default(),
            purple: self.purple.as_ref().map(|c| c.materialize()).unwrap_or_default(),
            magenta: self.magenta.as_ref().map(|c| c.materialize()).unwrap_or_default(),
        }
    }
}

impl From<&HslChannels> for PartialHslChannels {
    fn from(hsl: &HslChannels) -> Self {
        Self {
            red: Some(PartialHslChannel::from(&hsl.red)),
            orange: Some(PartialHslChannel::from(&hsl.orange)),
            yellow: Some(PartialHslChannel::from(&hsl.yellow)),
            green: Some(PartialHslChannel::from(&hsl.green)),
            aqua: Some(PartialHslChannel::from(&hsl.aqua)),
            blue: Some(PartialHslChannel::from(&hsl.blue)),
            purple: Some(PartialHslChannel::from(&hsl.purple)),
            magenta: Some(PartialHslChannel::from(&hsl.magenta)),
        }
    }
}
```

**Step 4: Run tests**

Run: `cargo test -p oxiraw engine::tests::partial_hsl`
Expected: PASS

**Step 5: Stage**

```bash
git add crates/oxiraw/src/engine/mod.rs
git commit -m "feat: add PartialHslChannel and PartialHslChannels types"
```

---

### Task 1.2: PartialParameters type

**Files:**
- Modify: `crates/oxiraw/src/engine/mod.rs`

**Step 1: Write failing tests**

```rust
#[test]
fn partial_parameters_default_is_all_none() {
    let p = super::PartialParameters::default();
    assert_eq!(p.exposure, None);
    assert_eq!(p.contrast, None);
    assert_eq!(p.hsl, None);
}

#[test]
fn partial_parameters_merge_overlay_wins() {
    let base = super::PartialParameters {
        exposure: Some(1.0),
        contrast: Some(20.0),
        ..Default::default()
    };
    let overlay = super::PartialParameters {
        exposure: Some(2.0),
        highlights: Some(-30.0),
        ..Default::default()
    };
    let merged = base.merge(&overlay);
    assert_eq!(merged.exposure, Some(2.0));   // overlay wins
    assert_eq!(merged.contrast, Some(20.0));  // base kept
    assert_eq!(merged.highlights, Some(-30.0)); // overlay added
    assert_eq!(merged.shadows, None);          // neither specified
}

#[test]
fn partial_parameters_materialize_defaults() {
    let partial = super::PartialParameters {
        exposure: Some(1.5),
        ..Default::default()
    };
    let params = partial.materialize();
    assert_eq!(params.exposure, 1.5);
    assert_eq!(params.contrast, 0.0);
    assert_eq!(params.temperature, 0.0);
    assert!(params.hsl.is_default());
}

#[test]
fn partial_parameters_from_parameters_all_some() {
    let params = Parameters {
        exposure: 1.0,
        contrast: 20.0,
        ..Default::default()
    };
    let partial = super::PartialParameters::from(&params);
    assert_eq!(partial.exposure, Some(1.0));
    assert_eq!(partial.contrast, Some(20.0));
    assert_eq!(partial.highlights, Some(0.0));
}

#[test]
fn partial_parameters_merge_with_hsl() {
    let base = super::PartialParameters {
        exposure: Some(1.0),
        ..Default::default()
    };
    let mut hsl = super::PartialHslChannels::default();
    hsl.red = Some(super::PartialHslChannel { hue: Some(10.0), saturation: None, luminance: None });
    let overlay = super::PartialParameters {
        hsl: Some(hsl),
        ..Default::default()
    };
    let merged = base.merge(&overlay);
    assert_eq!(merged.exposure, Some(1.0));
    assert!(merged.hsl.is_some());
    assert_eq!(merged.hsl.as_ref().unwrap().red.as_ref().unwrap().hue, Some(10.0));
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p oxiraw engine::tests::partial_parameters`
Expected: FAIL

**Step 3: Write implementation**

Add after `PartialHslChannels` impl block:

```rust
/// Partial parameter set — `None` means "not specified by this preset".
///
/// Used for preset deserialization and merging. Convert to concrete
/// `Parameters` via `materialize()` for the engine.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PartialParameters {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exposure: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contrast: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub highlights: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadows: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub whites: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blacks: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tint: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hsl: Option<PartialHslChannels>,
}

impl PartialParameters {
    /// Merge `other` on top of `self` (last-write-wins).
    pub fn merge(&self, other: &Self) -> Self {
        Self {
            exposure: other.exposure.or(self.exposure),
            contrast: other.contrast.or(self.contrast),
            highlights: other.highlights.or(self.highlights),
            shadows: other.shadows.or(self.shadows),
            whites: other.whites.or(self.whites),
            blacks: other.blacks.or(self.blacks),
            temperature: other.temperature.or(self.temperature),
            tint: other.tint.or(self.tint),
            hsl: match (&self.hsl, &other.hsl) {
                (None, None) => None,
                (Some(b), None) => Some(b.clone()),
                (None, Some(o)) => Some(o.clone()),
                (Some(b), Some(o)) => Some(b.merge(o)),
            },
        }
    }

    /// Convert to concrete Parameters. `None` fields become their default (0.0).
    pub fn materialize(&self) -> Parameters {
        Parameters {
            exposure: self.exposure.unwrap_or(0.0),
            contrast: self.contrast.unwrap_or(0.0),
            highlights: self.highlights.unwrap_or(0.0),
            shadows: self.shadows.unwrap_or(0.0),
            whites: self.whites.unwrap_or(0.0),
            blacks: self.blacks.unwrap_or(0.0),
            temperature: self.temperature.unwrap_or(0.0),
            tint: self.tint.unwrap_or(0.0),
            hsl: self.hsl.as_ref().map(|h| h.materialize()).unwrap_or_default(),
        }
    }
}

impl From<&Parameters> for PartialParameters {
    fn from(params: &Parameters) -> Self {
        Self {
            exposure: Some(params.exposure),
            contrast: Some(params.contrast),
            highlights: Some(params.highlights),
            shadows: Some(params.shadows),
            whites: Some(params.whites),
            blacks: Some(params.blacks),
            temperature: Some(params.temperature),
            tint: Some(params.tint),
            hsl: Some(PartialHslChannels::from(&params.hsl)),
        }
    }
}
```

**Step 4: Run tests**

Run: `cargo test -p oxiraw engine::tests::partial_parameters`
Expected: PASS

**Step 5: Stage**

```bash
git add crates/oxiraw/src/engine/mod.rs
git commit -m "feat: add PartialParameters with merge and materialize"
```

---

## Phase 2: Preset Deserialization

### Task 2.1: Switch PresetRaw to Option fields and populate partial_params

**Files:**
- Modify: `crates/oxiraw/src/preset/mod.rs`

**Step 1: Write failing tests**

Add to preset tests:

```rust
#[test]
fn preset_partial_only_specified_fields_are_some() {
    let toml_str = r#"
[metadata]
name = "Warm"

[tone]
exposure = 1.0
"#;
    let preset = Preset::from_toml(toml_str).unwrap();
    assert_eq!(preset.partial_params.exposure, Some(1.0));
    assert_eq!(preset.partial_params.contrast, None);
    assert_eq!(preset.partial_params.temperature, None);
}

#[test]
fn preset_partial_explicit_zero_is_some() {
    let toml_str = r#"
[metadata]
name = "Zero"

[tone]
exposure = 0.0
contrast = 0.0
"#;
    let preset = Preset::from_toml(toml_str).unwrap();
    assert_eq!(preset.partial_params.exposure, Some(0.0));
    assert_eq!(preset.partial_params.contrast, Some(0.0));
    assert_eq!(preset.partial_params.highlights, None);
}

#[test]
fn preset_partial_hsl_only_specified() {
    let toml_str = r#"
[metadata]
name = "HSL"

[hsl.red]
hue = 10.0
"#;
    let preset = Preset::from_toml(toml_str).unwrap();
    let hsl = preset.partial_params.hsl.as_ref().unwrap();
    assert!(hsl.red.is_some());
    assert_eq!(hsl.red.as_ref().unwrap().hue, Some(10.0));
    assert_eq!(hsl.red.as_ref().unwrap().saturation, None);
    assert!(hsl.green.is_none());
}

#[test]
fn preset_materialized_params_match_legacy_behavior() {
    let toml_str = r#"
[metadata]
name = "Full"

[tone]
exposure = 1.0
contrast = 20.0
highlights = -10.0
shadows = 15.0
whites = 5.0
blacks = -5.0

[white_balance]
temperature = 30.0
tint = -5.0
"#;
    let preset = Preset::from_toml(toml_str).unwrap();
    assert_eq!(preset.params.exposure, 1.0);
    assert_eq!(preset.params.contrast, 20.0);
    assert_eq!(preset.params.temperature, 30.0);
}

#[test]
fn preset_roundtrip_preserves_partial() {
    let toml_str = r#"
[metadata]
name = "Partial"

[tone]
exposure = 1.0
"#;
    let preset = Preset::from_toml(toml_str).unwrap();
    assert_eq!(preset.partial_params.exposure, Some(1.0));
    assert_eq!(preset.partial_params.contrast, None);
    // Materialized params should fill defaults
    assert_eq!(preset.params.exposure, 1.0);
    assert_eq!(preset.params.contrast, 0.0);
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p oxiraw preset::tests`
Expected: FAIL (partial_params field doesn't exist on Preset yet)

**Step 3: Write implementation**

Switch `ToneSection` and `WhiteBalanceSection` to use `Option<f32>` fields. Update `PresetRaw` to use `Option<PartialHslChannels>` for HSL. Add `partial_params` to `Preset`. Build both `partial_params` and `params` during deserialization.

Key changes:
- `ToneSection` fields become `Option<f32>` (remove `#[serde(default)]`)
- `WhiteBalanceSection` fields become `Option<f32>`
- `PresetRaw.hsl` becomes `Option<PartialHslChannels>`
- `Preset` gains `partial_params: PartialParameters`
- `from_toml` builds `PartialParameters` from the raw fields, then materializes to `Parameters`
- `to_toml` serializes from `partial_params` to preserve None vs Some(0.0) distinction
- `load_from_file` follows the same pattern

**Step 4: Run tests**

Run: `cargo test -p oxiraw preset::tests`
Expected: PASS (all existing tests + new tests)

**Step 5: Stage**

```bash
git add crates/oxiraw/src/preset/mod.rs
git commit -m "feat: switch preset deserialization to Option fields with PartialParameters"
```

---

## Phase 3: Engine Integration

### Task 3.1: Add layer_preset to Engine

**Files:**
- Modify: `crates/oxiraw/src/engine/mod.rs`

**Step 1: Write failing tests**

```rust
#[test]
fn layer_preset_only_overrides_specified_fields() {
    let img = make_test_image(0.5, 0.5, 0.5);
    let mut engine = Engine::new(img);
    engine.params_mut().exposure = 1.0;
    engine.params_mut().contrast = 20.0;

    // Create a preset that only specifies contrast
    let mut preset = crate::preset::Preset::default();
    preset.partial_params.contrast = Some(50.0);
    preset.params = preset.partial_params.materialize();

    engine.layer_preset(&preset);
    assert_eq!(engine.params().exposure, 1.0);   // untouched
    assert_eq!(engine.params().contrast, 50.0);  // overridden
}

#[test]
fn layer_preset_preserves_unspecified_hsl() {
    let img = make_test_image(0.5, 0.5, 0.5);
    let mut engine = Engine::new(img);
    engine.params_mut().hsl.red.hue = 15.0;

    // Preset only touches green HSL
    let mut preset = crate::preset::Preset::default();
    let mut partial_hsl = PartialHslChannels::default();
    partial_hsl.green = Some(PartialHslChannel { hue: Some(10.0), saturation: None, luminance: None });
    preset.partial_params.hsl = Some(partial_hsl);
    preset.params = preset.partial_params.materialize();

    engine.layer_preset(&preset);
    assert_eq!(engine.params().hsl.red.hue, 15.0);   // untouched
    assert_eq!(engine.params().hsl.green.hue, 10.0);  // set by preset
}

#[test]
fn layer_multiple_presets_last_wins() {
    let img = make_test_image(0.5, 0.5, 0.5);
    let mut engine = Engine::new(img);

    let mut preset1 = crate::preset::Preset::default();
    preset1.partial_params.exposure = Some(1.0);
    preset1.partial_params.contrast = Some(20.0);
    preset1.params = preset1.partial_params.materialize();

    let mut preset2 = crate::preset::Preset::default();
    preset2.partial_params.exposure = Some(2.0);
    preset2.params = preset2.partial_params.materialize();

    engine.layer_preset(&preset1);
    engine.layer_preset(&preset2);

    assert_eq!(engine.params().exposure, 2.0);   // preset2 wins
    assert_eq!(engine.params().contrast, 20.0);  // preset1 kept
}

#[test]
fn apply_preset_still_does_full_replacement() {
    let img = make_test_image(0.5, 0.5, 0.5);
    let mut engine = Engine::new(img);
    engine.params_mut().exposure = 1.0;
    engine.params_mut().contrast = 20.0;

    // Preset that only specifies exposure
    let mut preset = crate::preset::Preset::default();
    preset.partial_params.exposure = Some(0.5);
    preset.params = preset.partial_params.materialize();

    engine.apply_preset(&preset);
    assert_eq!(engine.params().exposure, 0.5);
    assert_eq!(engine.params().contrast, 0.0); // reset to default (full replacement)
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p oxiraw engine::tests::layer_preset`
Expected: FAIL

**Step 3: Write implementation**

Add to `Engine`:

```rust
/// Layer a preset on top of current parameters.
/// Only fields specified in the preset (Some values in partial_params)
/// are overridden. Unspecified fields keep their current values.
pub fn layer_preset(&mut self, preset: &crate::preset::Preset) {
    let current_partial = PartialParameters::from(&self.params);
    let merged = current_partial.merge(&preset.partial_params);
    self.params = merged.materialize();
    if preset.lut.is_some() {
        self.lut = preset.lut.clone();
    }
}
```

**Step 4: Run tests**

Run: `cargo test -p oxiraw engine::tests`
Expected: PASS

**Step 5: Stage**

```bash
git add crates/oxiraw/src/engine/mod.rs
git commit -m "feat: add Engine::layer_preset for composable preset application"
```

---

## Phase 4: Preset Inheritance

### Task 4.1: Add extends support to preset loading

**Files:**
- Modify: `crates/oxiraw/src/preset/mod.rs`

**Step 1: Write failing tests**

```rust
#[test]
fn preset_extends_single_level() {
    let temp_dir = std::env::temp_dir();
    let base_path = temp_dir.join("oxiraw_extends_base.toml");
    let child_path = temp_dir.join("oxiraw_extends_child.toml");

    std::fs::write(&base_path, r#"
[metadata]
name = "Base"

[tone]
exposure = 1.0
contrast = 20.0
"#).unwrap();

    std::fs::write(&child_path, format!(r#"
[metadata]
name = "Child"
extends = "{}"

[tone]
contrast = 50.0
"#, base_path.file_name().unwrap().to_str().unwrap())).unwrap();

    let preset = Preset::load_from_file(&child_path).unwrap();
    assert_eq!(preset.metadata.name, "Child");
    assert_eq!(preset.params.exposure, 1.0);   // inherited from base
    assert_eq!(preset.params.contrast, 50.0);  // overridden by child

    let _ = std::fs::remove_file(&base_path);
    let _ = std::fs::remove_file(&child_path);
}

#[test]
fn preset_extends_multi_level() {
    let temp_dir = std::env::temp_dir();
    let grandparent = temp_dir.join("oxiraw_extends_gp.toml");
    let parent = temp_dir.join("oxiraw_extends_parent.toml");
    let child = temp_dir.join("oxiraw_extends_child2.toml");

    std::fs::write(&grandparent, r#"
[metadata]
name = "Grandparent"

[tone]
exposure = 1.0
contrast = 10.0
highlights = -20.0
"#).unwrap();

    std::fs::write(&parent, format!(r#"
[metadata]
name = "Parent"
extends = "{}"

[tone]
contrast = 30.0
"#, grandparent.file_name().unwrap().to_str().unwrap())).unwrap();

    std::fs::write(&child, format!(r#"
[metadata]
name = "Child"
extends = "{}"

[tone]
highlights = 10.0
"#, parent.file_name().unwrap().to_str().unwrap())).unwrap();

    let preset = Preset::load_from_file(&child).unwrap();
    assert_eq!(preset.params.exposure, 1.0);     // from grandparent
    assert_eq!(preset.params.contrast, 30.0);    // from parent
    assert_eq!(preset.params.highlights, 10.0);  // from child

    let _ = std::fs::remove_file(&grandparent);
    let _ = std::fs::remove_file(&parent);
    let _ = std::fs::remove_file(&child);
}

#[test]
fn preset_extends_cycle_detection() {
    let temp_dir = std::env::temp_dir();
    let a_path = temp_dir.join("oxiraw_cycle_a.toml");
    let b_path = temp_dir.join("oxiraw_cycle_b.toml");

    std::fs::write(&a_path, format!(r#"
[metadata]
name = "A"
extends = "{}"
"#, b_path.file_name().unwrap().to_str().unwrap())).unwrap();

    std::fs::write(&b_path, format!(r#"
[metadata]
name = "B"
extends = "{}"
"#, a_path.file_name().unwrap().to_str().unwrap())).unwrap();

    let result = Preset::load_from_file(&a_path);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("circular"), "Expected circular error, got: {err_msg}");

    let _ = std::fs::remove_file(&a_path);
    let _ = std::fs::remove_file(&b_path);
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p oxiraw preset::tests::preset_extends`
Expected: FAIL

**Step 3: Write implementation**

Add `extends` field to `PresetMetadata`:
```rust
#[serde(default, skip_serializing_if = "Option::is_none")]
pub extends: Option<String>,
```

Add a private recursive loader:
```rust
fn load_from_file_with_visited(
    path: &std::path::Path,
    visited: &mut std::collections::HashSet<std::path::PathBuf>,
) -> Result<(PartialParameters, Option<crate::lut::Lut3D>, PresetMetadata)> {
    let canonical = path.canonicalize().map_err(|e| OxirawError::Io(e))?;
    if !visited.insert(canonical.clone()) {
        return Err(OxirawError::Preset(format!(
            "circular extends: {} already visited",
            canonical.display()
        )));
    }

    let content = std::fs::read_to_string(path)?;
    let raw: PresetRaw = toml::from_str(&content).map_err(|e| OxirawError::Preset(e.to_string()))?;
    let base_dir = path.parent().unwrap_or(std::path::Path::new("."));

    // Build this preset's partial params
    let this_partial = build_partial_params(&raw);

    // Resolve extends
    let (merged_partial, base_lut) = if let Some(extends_path) = &raw.metadata.extends {
        let extends_full = base_dir.join(extends_path);
        let (base_partial, base_lut, _) = load_from_file_with_visited(&extends_full, visited)?;
        (base_partial.merge(&this_partial), base_lut)
    } else {
        (this_partial, None)
    };

    // Load this preset's LUT (overrides base LUT if present)
    let lut = if let Some(lut_path_str) = &raw.lut.path {
        let lut_path = base_dir.join(lut_path_str);
        Some(crate::lut::Lut3D::from_cube_file(&lut_path)?)
    } else {
        base_lut
    };

    Ok((merged_partial, lut, raw.metadata))
}
```

Update `load_from_file` to call this recursive loader, then materialize.

**Step 4: Run tests**

Run: `cargo test -p oxiraw preset::tests`
Expected: PASS

**Step 5: Stage**

```bash
git add crates/oxiraw/src/preset/mod.rs
git commit -m "feat: add preset inheritance via extends with cycle detection"
```

---

## Phase 5: CLI Multi-Preset Support

### Task 5.1: Add --presets flag to apply subcommand

**Files:**
- Modify: `crates/oxiraw-cli/src/main.rs`

**Step 1: Write failing test**

Add to `crates/oxiraw-cli/tests/integration.rs`:

```rust
#[test]
fn cli_apply_multiple_presets() {
    let temp_dir = std::env::temp_dir();
    let input = temp_dir.join("oxiraw_cli_multi_in.png");
    let output = temp_dir.join("oxiraw_cli_multi_out.png");
    let preset1 = temp_dir.join("oxiraw_cli_multi_p1.toml");
    let preset2 = temp_dir.join("oxiraw_cli_multi_p2.toml");

    let img: image::ImageBuffer<image::Rgb<u8>, Vec<u8>> =
        image::ImageBuffer::from_pixel(4, 4, image::Rgb([128u8, 128, 128]));
    img.save(&input).unwrap();

    std::fs::write(&preset1, "[metadata]\nname = \"P1\"\n\n[tone]\nexposure = 1.0\n").unwrap();
    std::fs::write(&preset2, "[metadata]\nname = \"P2\"\n\n[tone]\ncontrast = 20.0\n").unwrap();

    let bin = env!("CARGO_BIN_EXE_oxiraw-cli");
    let status = std::process::Command::new(bin)
        .args([
            "apply",
            "-i", input.to_str().unwrap(),
            "-o", output.to_str().unwrap(),
            "--presets", &format!("{},{}", preset1.display(), preset2.display()),
        ])
        .status()
        .unwrap();
    assert!(status.success(), "CLI should succeed with --presets");
    assert!(output.exists(), "Output file should exist");

    let _ = std::fs::remove_file(&input);
    let _ = std::fs::remove_file(&output);
    let _ = std::fs::remove_file(&preset1);
    let _ = std::fs::remove_file(&preset2);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p oxiraw-cli cli_apply_multiple_presets`
Expected: FAIL

**Step 3: Write implementation**

Add `--presets` flag to `Commands::Apply`:

```rust
/// Comma-separated list of preset TOML files to layer (left-to-right, last-write-wins)
#[arg(long, conflicts_with = "preset")]
presets: Option<String>,
```

Make `preset` optional:
```rust
#[arg(short, long)]
preset: Option<PathBuf>,
```

Update `run_apply` to handle both modes. When `--presets` is used, load each preset and use `engine.layer_preset()` in order.

**Step 4: Run tests**

Run: `cargo test -p oxiraw-cli`
Expected: PASS

**Step 5: Stage**

```bash
git add crates/oxiraw-cli/src/main.rs crates/oxiraw-cli/tests/integration.rs
git commit -m "feat: add --presets flag for multi-preset layering in CLI"
```

---

## Phase 6: Documentation and Exports

### Task 6.1: Update docs, exports, and ARCHITECTURE.md

**Files:**
- Modify: `crates/oxiraw/src/lib.rs` — add re-exports
- Modify: `crates/oxiraw/src/engine/README.md` — document new types and layer_preset
- Modify: `crates/oxiraw/src/preset/README.md` — document extends and partial_params
- Modify: `ARCHITECTURE.md` — add design doc to table

**Step 1: Update lib.rs**

Add re-exports:
```rust
pub use engine::{PartialHslChannel, PartialHslChannels, PartialParameters};
```

**Step 2: Update engine/README.md**

Add to Public API section:
- `PartialParameters` — partial parameter set with `Option<T>` fields for preset composability
- `PartialHslChannel` / `PartialHslChannels` — partial HSL types
- `PartialParameters::merge(&self, other)` — last-write-wins merge
- `PartialParameters::materialize(&self)` — convert to concrete `Parameters`
- `Engine::layer_preset(preset)` — layer a preset on top of current parameters

**Step 3: Update preset/README.md**

Add to Public API section:
- `Preset.partial_params` — the partial parameter set preserving None vs Some distinction
- `PresetMetadata.extends` — optional path to a base preset for inheritance

Add to Extension Guide:
- Preset inheritance via `extends` field, resolved relative to preset file directory
- Cycle detection for inheritance chains

**Step 4: Update ARCHITECTURE.md**

Add to Plans table:
```markdown
| 2026-03-07 | [Preset Composability Design](docs/plans/2026-03-07-preset-composability-design.md)                  |
| 2026-03-07 | [Preset Composability Implementation](docs/plans/2026-03-07-preset-composability-implementation.md)  |
```

**Step 5: Stage**

```bash
git add crates/oxiraw/src/lib.rs crates/oxiraw/src/engine/README.md crates/oxiraw/src/preset/README.md ARCHITECTURE.md
git commit -m "docs: update exports, READMEs, and architecture docs for preset composability"
```

---

## Verification

Run the full verification suite:

```bash
./scripts/verify.sh
```

All 5 checks must pass:
1. Format (`cargo fmt`)
2. Clippy (`cargo clippy`)
3. Library tests (`cargo test -p oxiraw`)
4. CLI tests (`cargo test -p oxiraw-cli`)
5. Documentation links

If all pass, no further commits needed. If formatting/clippy fixes required:

```bash
cargo fmt
git add -A
git commit -m "chore: format and lint fixes for preset composability"
```
