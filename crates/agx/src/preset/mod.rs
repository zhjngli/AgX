use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::engine::{
    Parameters, PartialColorGradingParams, PartialDetailParams, PartialHslChannels,
    PartialParameters, PartialToneCurve, PartialToneCurveParams, PartialVignetteParams,
};
use crate::error::{AgxError, Result};

/// Preset metadata (name, version, author).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PresetMetadata {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub author: String,
    /// Optional path to a base preset this preset extends.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extends: Option<String>,
}

/// Tone adjustment section of a preset (Option fields for composability).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
struct ToneSection {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    exposure: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    contrast: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    highlights: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    shadows: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    whites: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    blacks: Option<f32>,
}

/// White balance section of a preset (Option fields for composability).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
struct WhiteBalanceSection {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tint: Option<f32>,
}

/// LUT section of a preset.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct LutSection {
    #[serde(default)]
    path: Option<String>,
}

/// Internal TOML layout for a preset file.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct PresetRaw {
    #[serde(default)]
    metadata: PresetMetadata,
    #[serde(default)]
    tone: ToneSection,
    #[serde(default)]
    white_balance: WhiteBalanceSection,
    #[serde(default)]
    lut: LutSection,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    hsl: Option<PartialHslChannels>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    vignette: Option<PartialVignetteParams>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    color_grading: Option<PartialColorGradingParams>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tone_curve: Option<PartialToneCurveParams>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    detail: Option<PartialDetailParams>,
}

fn validate_tone_curve_params(params: &PartialToneCurveParams) -> Result<()> {
    fn validate_channel(tc: &Option<PartialToneCurve>, name: &str) -> Result<()> {
        if let Some(ref c) = tc {
            if let Some(ref pts) = c.points {
                let curve = crate::adjust::ToneCurve {
                    points: pts.clone(),
                };
                curve
                    .validate()
                    .map_err(|e| AgxError::Preset(format!("tone_curve.{name}: {e}")))?;
            }
        }
        Ok(())
    }
    validate_channel(&params.rgb, "rgb")?;
    validate_channel(&params.luma, "luma")?;
    validate_channel(&params.red, "red")?;
    validate_channel(&params.green, "green")?;
    validate_channel(&params.blue, "blue")?;
    Ok(())
}

/// Build a PartialParameters from a PresetRaw.
fn build_partial_params(raw: &PresetRaw) -> PartialParameters {
    PartialParameters {
        exposure: raw.tone.exposure,
        contrast: raw.tone.contrast,
        highlights: raw.tone.highlights,
        shadows: raw.tone.shadows,
        whites: raw.tone.whites,
        blacks: raw.tone.blacks,
        temperature: raw.white_balance.temperature,
        tint: raw.white_balance.tint,
        hsl: raw.hsl.clone(),
        vignette: raw.vignette.clone(),
        color_grading: raw.color_grading.clone(),
        tone_curve: raw.tone_curve.clone(),
        detail: raw.detail.clone(),
    }
}

/// A photo editing preset.
///
/// Presets are declarative parameter sets stored as TOML files.
/// Missing values default to neutral (no change).
///
/// `partial_params` preserves which fields were explicitly set (`Some`)
/// vs absent (`None`), enabling preset layering and composability.
/// Call `params()` to get concrete `Parameters` with defaults filled in.
#[derive(Debug, Clone, Default)]
pub struct Preset {
    pub metadata: PresetMetadata,
    pub partial_params: PartialParameters,
    pub lut: Option<Arc<crate::lut::Lut3D>>,
}

impl Preset {
    /// Materialize concrete Parameters from partial_params.
    pub fn params(&self) -> Parameters {
        self.partial_params.materialize()
    }
}

impl PartialEq for Preset {
    fn eq(&self, other: &Self) -> bool {
        self.metadata == other.metadata && self.partial_params == other.partial_params
    }
}

impl Preset {
    /// Parse a preset from a TOML string.
    ///
    /// Note: LUT paths in the `[lut]` section cannot be resolved without a base
    /// directory. Use [`load_from_file`](Preset::load_from_file) to load presets
    /// with LUT references.
    pub fn from_toml(toml_str: &str) -> Result<Self> {
        let raw: PresetRaw =
            toml::from_str(toml_str).map_err(|e| AgxError::Preset(e.to_string()))?;
        let partial = build_partial_params(&raw);
        if let Some(ref tc) = partial.tone_curve {
            validate_tone_curve_params(tc)?;
        }
        Ok(Self {
            metadata: raw.metadata,
            partial_params: partial,
            lut: None,
        })
    }

    /// Serialize the preset to a TOML string.
    ///
    /// Uses `partial_params` to preserve which fields were explicitly set.
    pub fn to_toml(&self) -> Result<String> {
        let raw = PresetRaw {
            metadata: self.metadata.clone(),
            tone: ToneSection {
                exposure: self.partial_params.exposure,
                contrast: self.partial_params.contrast,
                highlights: self.partial_params.highlights,
                shadows: self.partial_params.shadows,
                whites: self.partial_params.whites,
                blacks: self.partial_params.blacks,
            },
            white_balance: WhiteBalanceSection {
                temperature: self.partial_params.temperature,
                tint: self.partial_params.tint,
            },
            lut: LutSection::default(),
            hsl: self.partial_params.hsl.clone(),
            vignette: self.partial_params.vignette.clone(),
            color_grading: self.partial_params.color_grading.clone(),
            tone_curve: self.partial_params.tone_curve.clone(),
            detail: self.partial_params.detail.clone(),
        };
        toml::to_string_pretty(&raw).map_err(|e| AgxError::Preset(e.to_string()))
    }

    /// Load a preset from a TOML file.
    ///
    /// If the preset contains a `[lut]` section with a `path`, the LUT file
    /// is resolved relative to the preset file's directory and loaded.
    ///
    /// If the preset contains an `extends` field in `[metadata]`, the base
    /// preset is loaded recursively and merged (last-write-wins). Circular
    /// inheritance chains are detected and return an error.
    pub fn load_from_file(path: &std::path::Path) -> Result<Self> {
        let mut visited = std::collections::HashSet::new();
        Self::load_from_file_with_visited(path, &mut visited)
    }

    fn load_from_file_with_visited(
        path: &std::path::Path,
        visited: &mut std::collections::HashSet<std::path::PathBuf>,
    ) -> Result<Self> {
        let canonical = path.canonicalize().map_err(AgxError::Io)?;
        if !visited.insert(canonical.clone()) {
            return Err(AgxError::Preset(format!(
                "circular extends: {} already visited",
                canonical.display()
            )));
        }

        let content = std::fs::read_to_string(path)?;
        let raw: PresetRaw =
            toml::from_str(&content).map_err(|e| AgxError::Preset(e.to_string()))?;

        let base_dir = path.parent().unwrap_or(std::path::Path::new("."));
        let this_partial = build_partial_params(&raw);

        // Resolve inheritance
        let (merged_partial, base_lut) = if let Some(extends_path) = &raw.metadata.extends {
            let extends_full = base_dir.join(extends_path);
            let base_preset = Self::load_from_file_with_visited(&extends_full, visited)?;
            let merged = base_preset.partial_params.merge(&this_partial);
            (merged, base_preset.lut)
        } else {
            (this_partial.clone(), None)
        };

        // Load this preset's LUT (overrides base LUT if present)
        let lut = if let Some(lut_path_str) = &raw.lut.path {
            let lut_path = base_dir.join(lut_path_str);
            Some(Arc::new(crate::lut::Lut3D::from_cube_file(&lut_path)?))
        } else {
            base_lut
        };

        Ok(Self {
            metadata: raw.metadata,
            partial_params: merged_partial,
            lut,
        })
    }

    /// Save the preset to a TOML file.
    pub fn save_to_file(&self, path: &std::path::Path) -> Result<()> {
        let toml_str = self.to_toml()?;
        std::fs::write(path, toml_str)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_preset_is_neutral() {
        let preset = Preset::default();
        assert_eq!(preset.params(), Parameters::default());
        assert_eq!(preset.metadata.name, "");
    }

    #[test]
    fn serialize_contains_expected_keys() {
        let mut preset = Preset::default();
        preset.metadata.name = "Test".into();
        preset.partial_params.exposure = Some(1.5);
        preset.partial_params.temperature = Some(30.0);

        let toml_str = preset.to_toml().unwrap();
        assert!(toml_str.contains("name = \"Test\""));
        assert!(toml_str.contains("exposure = 1.5"));
        assert!(toml_str.contains("temperature = 30.0"));
    }

    #[test]
    fn deserialize_parses_values() {
        let toml_str = r#"
[metadata]
name = "Golden Hour"
version = "1.0"
author = "test"

[tone]
exposure = 0.5
contrast = 15.0
highlights = -30.0
shadows = 25.0

[white_balance]
temperature = 6200.0
tint = 10.0
"#;
        let preset = Preset::from_toml(toml_str).unwrap();
        assert_eq!(preset.metadata.name, "Golden Hour");
        assert_eq!(preset.params().exposure, 0.5);
        assert_eq!(preset.params().contrast, 15.0);
        assert_eq!(preset.params().highlights, -30.0);
        assert_eq!(preset.params().shadows, 25.0);
        assert_eq!(preset.params().temperature, 6200.0);
        assert_eq!(preset.params().tint, 10.0);
    }

    #[test]
    fn roundtrip_serialize_deserialize() {
        let mut preset = Preset::default();
        preset.metadata.name = "Roundtrip".into();
        preset.partial_params.exposure = Some(2.0);
        preset.partial_params.contrast = Some(-10.0);
        preset.partial_params.temperature = Some(50.0);
        preset.partial_params.tint = Some(-5.0);

        let toml_str = preset.to_toml().unwrap();
        let parsed = Preset::from_toml(&toml_str).unwrap();
        assert_eq!(preset, parsed);
    }

    #[test]
    fn missing_fields_default_to_zero() {
        let toml_str = r#"
[metadata]
name = "Minimal"

[tone]
exposure = 1.0
"#;
        let preset = Preset::from_toml(toml_str).unwrap();
        assert_eq!(preset.params().exposure, 1.0);
        assert_eq!(preset.params().contrast, 0.0);
        assert_eq!(preset.params().highlights, 0.0);
        assert_eq!(preset.params().temperature, 0.0);
    }

    #[test]
    fn invalid_toml_returns_error() {
        let result = Preset::from_toml("this is not valid toml {{{{");
        assert!(result.is_err());
    }

    #[test]
    fn file_save_and_load_roundtrip() {
        let temp_path = std::env::temp_dir().join("agx_test_preset.toml");

        let mut preset = Preset::default();
        preset.metadata.name = "File Test".into();
        preset.partial_params.exposure = Some(1.5);
        preset.partial_params.contrast = Some(20.0);

        preset.save_to_file(&temp_path).unwrap();
        let loaded = Preset::load_from_file(&temp_path).unwrap();
        assert_eq!(preset, loaded);

        let _ = std::fs::remove_file(&temp_path);
    }

    #[test]
    fn load_nonexistent_file_returns_error() {
        let result = Preset::load_from_file(std::path::Path::new("/nonexistent/preset.toml"));
        assert!(result.is_err());
    }

    #[test]
    fn preset_with_lut_path_loads_lut() {
        let temp_dir = std::env::temp_dir();
        let cube_path = temp_dir.join("agx_preset_test.cube");
        let preset_path = temp_dir.join("agx_preset_test.toml");

        std::fs::write(
            &cube_path,
            "\
LUT_3D_SIZE 2
0.0 0.0 0.0
1.0 0.0 0.0
0.0 1.0 0.0
1.0 1.0 0.0
0.0 0.0 1.0
1.0 0.0 1.0
0.0 1.0 1.0
1.0 1.0 1.0
",
        )
        .unwrap();

        let toml_content = format!(
            "[metadata]\nname = \"LUT Test\"\n\n[tone]\nexposure = 0.5\n\n[lut]\npath = \"{}\"\n",
            cube_path.file_name().unwrap().to_str().unwrap()
        );
        std::fs::write(&preset_path, &toml_content).unwrap();

        let preset = Preset::load_from_file(&preset_path).unwrap();
        assert_eq!(preset.params().exposure, 0.5);
        assert!(preset.lut.is_some());
        assert_eq!(preset.lut.as_ref().unwrap().size, 2);

        let _ = std::fs::remove_file(&cube_path);
        let _ = std::fs::remove_file(&preset_path);
    }

    #[test]
    fn preset_without_lut_section_has_no_lut() {
        let toml_str = "[metadata]\nname = \"No LUT\"\n\n[tone]\nexposure = 1.0\n";
        let preset = Preset::from_toml(toml_str).unwrap();
        assert!(preset.lut.is_none());
    }

    #[test]
    fn preset_hsl_roundtrip() {
        use crate::engine::{PartialHslChannel, PartialHslChannels};
        let mut preset = Preset::default();
        preset.partial_params.hsl = Some(PartialHslChannels {
            red: Some(PartialHslChannel {
                hue: Some(15.0),
                saturation: None,
                luminance: None,
            }),
            green: Some(PartialHslChannel {
                hue: None,
                saturation: Some(-30.0),
                luminance: None,
            }),
            blue: Some(PartialHslChannel {
                hue: None,
                saturation: None,
                luminance: Some(20.0),
            }),
            ..Default::default()
        });

        let toml_str = preset.to_toml().unwrap();
        let parsed = Preset::from_toml(&toml_str).unwrap();
        assert_eq!(preset.params().hsl, parsed.params().hsl);
    }

    #[test]
    fn preset_missing_hsl_defaults_to_zero() {
        let toml_str = "[metadata]\nname = \"No HSL\"\n\n[tone]\nexposure = 1.0\n";
        let preset = Preset::from_toml(toml_str).unwrap();
        assert!(preset.params().hsl.is_default());
    }

    #[test]
    fn preset_partial_hsl_channels_default() {
        let toml_str = r#"
[metadata]
name = "Partial HSL"

[hsl.red]
hue = 10.0
"#;
        let preset = Preset::from_toml(toml_str).unwrap();
        assert_eq!(preset.params().hsl.red.hue, 10.0);
        assert_eq!(preset.params().hsl.red.saturation, 0.0);
        assert!(preset.params().hsl.green == crate::engine::HslChannel::default());
    }

    #[test]
    fn preset_with_missing_lut_file_returns_error() {
        let temp_dir = std::env::temp_dir();
        let preset_path = temp_dir.join("agx_missing_lut_test.toml");
        std::fs::write(
            &preset_path,
            "[metadata]\nname = \"Bad\"\n\n[lut]\npath = \"nonexistent.cube\"\n",
        )
        .unwrap();

        let result = Preset::load_from_file(&preset_path);
        assert!(result.is_err());

        let _ = std::fs::remove_file(&preset_path);
    }

    // --- Partial parameters tests ---

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
        assert_eq!(preset.params().exposure, 1.0);
        assert_eq!(preset.params().contrast, 20.0);
        assert_eq!(preset.params().temperature, 30.0);
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
        assert_eq!(preset.params().exposure, 1.0);
        assert_eq!(preset.params().contrast, 0.0);
    }

    // --- Vignette preset tests ---

    #[test]
    fn preset_vignette_roundtrip() {
        let mut preset = Preset::default();
        preset.partial_params.vignette = Some(crate::engine::PartialVignetteParams {
            amount: Some(-30.0),
            shape: Some(crate::adjust::VignetteShape::Circular),
        });

        let toml_str = preset.to_toml().unwrap();
        let parsed = Preset::from_toml(&toml_str).unwrap();
        assert_eq!(preset.params().vignette, parsed.params().vignette);
    }

    #[test]
    fn preset_vignette_from_toml() {
        let toml_str = r#"
[metadata]
name = "Vignette Test"

[vignette]
amount = -30.0
shape = "circular"
"#;
        let preset = Preset::from_toml(toml_str).unwrap();
        assert_eq!(preset.params().vignette.amount, -30.0);
        assert_eq!(
            preset.params().vignette.shape,
            crate::adjust::VignetteShape::Circular
        );
    }

    #[test]
    fn preset_vignette_default_shape() {
        let toml_str = r#"
[metadata]
name = "Vignette Default Shape"

[vignette]
amount = -20.0
"#;
        let preset = Preset::from_toml(toml_str).unwrap();
        assert_eq!(preset.params().vignette.amount, -20.0);
        assert_eq!(
            preset.params().vignette.shape,
            crate::adjust::VignetteShape::Elliptical
        );
    }

    #[test]
    fn preset_missing_vignette_defaults_to_neutral() {
        let toml_str = r#"
[metadata]
name = "No Vignette"

[tone]
exposure = 1.0
"#;
        let preset = Preset::from_toml(toml_str).unwrap();
        assert_eq!(preset.params().vignette.amount, 0.0);
    }

    #[test]
    fn preset_vignette_partial_only_amount() {
        let toml_str = r#"
[metadata]
name = "Partial"

[vignette]
amount = -15.0
"#;
        let preset = Preset::from_toml(toml_str).unwrap();
        let vig = preset.partial_params.vignette.as_ref().unwrap();
        assert_eq!(vig.amount, Some(-15.0));
        assert_eq!(vig.shape, None);
    }

    // --- Color grading preset tests ---

    #[test]
    fn color_grading_preset_round_trip() {
        let toml_str = r#"
[metadata]
name = "Color Grading Test"

[color_grading]
balance = -10.0

[color_grading.shadows]
hue = 200.0
saturation = 30.0
luminance = -5.0

[color_grading.highlights]
hue = 30.0
saturation = 25.0
"#;
        let preset = Preset::from_toml(toml_str).unwrap();
        let cg = preset.partial_params.color_grading.as_ref().unwrap();
        assert_eq!(cg.balance, Some(-10.0));
        let shadows = cg.shadows.as_ref().unwrap();
        assert_eq!(shadows.hue, Some(200.0));
        assert_eq!(shadows.saturation, Some(30.0));
        assert_eq!(shadows.luminance, Some(-5.0));
        let highlights = cg.highlights.as_ref().unwrap();
        assert_eq!(highlights.hue, Some(30.0));
        assert_eq!(highlights.saturation, Some(25.0));
        assert_eq!(highlights.luminance, None);
        assert!(cg.midtones.is_none());
    }

    #[test]
    fn preset_missing_color_grading_defaults_to_neutral() {
        let toml_str = r#"
[metadata]
name = "No CG"

[tone]
exposure = 1.0
"#;
        let preset = Preset::from_toml(toml_str).unwrap();
        assert!(preset.params().color_grading.is_default());
    }

    // --- Preset inheritance tests ---

    #[test]
    fn preset_extends_single_level() {
        let temp_dir = std::env::temp_dir();
        let base_path = temp_dir.join("agx_extends_base.toml");
        let child_path = temp_dir.join("agx_extends_child.toml");

        std::fs::write(
            &base_path,
            r#"
[metadata]
name = "Base"

[tone]
exposure = 1.0
contrast = 20.0
"#,
        )
        .unwrap();

        std::fs::write(
            &child_path,
            format!(
                r#"
[metadata]
name = "Child"
extends = "{}"

[tone]
contrast = 50.0
"#,
                base_path.file_name().unwrap().to_str().unwrap()
            ),
        )
        .unwrap();

        let preset = Preset::load_from_file(&child_path).unwrap();
        assert_eq!(preset.metadata.name, "Child");
        assert_eq!(preset.params().exposure, 1.0);
        assert_eq!(preset.params().contrast, 50.0);

        let _ = std::fs::remove_file(&base_path);
        let _ = std::fs::remove_file(&child_path);
    }

    #[test]
    fn preset_extends_multi_level() {
        let temp_dir = std::env::temp_dir();
        let grandparent = temp_dir.join("agx_extends_gp.toml");
        let parent = temp_dir.join("agx_extends_parent.toml");
        let child = temp_dir.join("agx_extends_child2.toml");

        std::fs::write(
            &grandparent,
            r#"
[metadata]
name = "Grandparent"

[tone]
exposure = 1.0
contrast = 10.0
highlights = -20.0
"#,
        )
        .unwrap();

        std::fs::write(
            &parent,
            format!(
                r#"
[metadata]
name = "Parent"
extends = "{}"

[tone]
contrast = 30.0
"#,
                grandparent.file_name().unwrap().to_str().unwrap()
            ),
        )
        .unwrap();

        std::fs::write(
            &child,
            format!(
                r#"
[metadata]
name = "Child"
extends = "{}"

[tone]
highlights = 10.0
"#,
                parent.file_name().unwrap().to_str().unwrap()
            ),
        )
        .unwrap();

        let preset = Preset::load_from_file(&child).unwrap();
        assert_eq!(preset.params().exposure, 1.0);
        assert_eq!(preset.params().contrast, 30.0);
        assert_eq!(preset.params().highlights, 10.0);

        let _ = std::fs::remove_file(&grandparent);
        let _ = std::fs::remove_file(&parent);
        let _ = std::fs::remove_file(&child);
    }

    #[test]
    fn preset_extends_cycle_detection() {
        let temp_dir = std::env::temp_dir();
        let a_path = temp_dir.join("agx_cycle_a.toml");
        let b_path = temp_dir.join("agx_cycle_b.toml");

        std::fs::write(
            &a_path,
            format!(
                r#"
[metadata]
name = "A"
extends = "{}"
"#,
                b_path.file_name().unwrap().to_str().unwrap()
            ),
        )
        .unwrap();

        std::fs::write(
            &b_path,
            format!(
                r#"
[metadata]
name = "B"
extends = "{}"
"#,
                a_path.file_name().unwrap().to_str().unwrap()
            ),
        )
        .unwrap();

        let result = Preset::load_from_file(&a_path);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("circular"),
            "Expected circular error, got: {err_msg}"
        );

        let _ = std::fs::remove_file(&a_path);
        let _ = std::fs::remove_file(&b_path);
    }

    #[test]
    fn tone_curve_preset_round_trip() {
        let toml_str = r#"
[metadata]
name = "Test Tone Curve"

[tone_curve.rgb]
points = [[0.0, 0.0], [0.25, 0.15], [0.75, 0.85], [1.0, 1.0]]

[tone_curve.red]
points = [[0.0, 0.0], [0.5, 0.6], [1.0, 1.0]]
"#;
        let preset = Preset::from_toml(toml_str).unwrap();
        let params = preset.partial_params.tone_curve.as_ref().unwrap();
        let rgb_pts = params.rgb.as_ref().unwrap().points.as_ref().unwrap();
        assert_eq!(rgb_pts.len(), 4);
        assert_eq!(rgb_pts[1], (0.25, 0.15));

        // Round-trip
        let serialized = preset.to_toml().unwrap();
        let preset2 = Preset::from_toml(&serialized).unwrap();
        let params2 = preset2.partial_params.tone_curve.as_ref().unwrap();
        let rgb_pts2 = params2.rgb.as_ref().unwrap().points.as_ref().unwrap();
        assert_eq!(rgb_pts, rgb_pts2);
    }

    #[test]
    fn preset_missing_tone_curve_defaults_to_neutral() {
        let toml_str = r#"
[metadata]
name = "No curves"
"#;
        let preset = Preset::from_toml(toml_str).unwrap();
        let materialized = preset.partial_params.materialize();
        assert!(materialized.tone_curve.is_default());
    }

    #[test]
    fn preset_tone_curve_invalid_points_rejected() {
        let toml_str = r#"
[metadata]
name = "Bad curve"

[tone_curve.rgb]
points = [[0.0, 0.0], [0.8, 0.5], [0.3, 0.7], [1.0, 1.0]]
"#;
        let result = Preset::from_toml(toml_str);
        assert!(result.is_err(), "non-increasing x should be rejected");
    }
}
