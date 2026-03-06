use serde::{Deserialize, Serialize};

use crate::engine::{HslChannels, Parameters};
use crate::error::{OxirawError, Result};

/// Preset metadata (name, version, author).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PresetMetadata {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub author: String,
}

/// Tone adjustment section of a preset.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
struct ToneSection {
    #[serde(default)]
    exposure: f32,
    #[serde(default)]
    contrast: f32,
    #[serde(default)]
    highlights: f32,
    #[serde(default)]
    shadows: f32,
    #[serde(default)]
    whites: f32,
    #[serde(default)]
    blacks: f32,
}

/// White balance section of a preset.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
struct WhiteBalanceSection {
    #[serde(default)]
    temperature: f32,
    #[serde(default)]
    tint: f32,
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
    #[serde(default)]
    hsl: HslChannels,
}

/// A photo editing preset.
///
/// Presets are declarative parameter sets stored as TOML files.
/// Missing values default to neutral (no change).
#[derive(Debug, Clone, Default)]
pub struct Preset {
    pub metadata: PresetMetadata,
    pub params: Parameters,
    pub lut: Option<crate::lut::Lut3D>,
}

impl PartialEq for Preset {
    fn eq(&self, other: &Self) -> bool {
        self.metadata == other.metadata && self.params == other.params
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
            toml::from_str(toml_str).map_err(|e| OxirawError::Preset(e.to_string()))?;
        Ok(Self {
            metadata: raw.metadata,
            params: Parameters {
                exposure: raw.tone.exposure,
                contrast: raw.tone.contrast,
                highlights: raw.tone.highlights,
                shadows: raw.tone.shadows,
                whites: raw.tone.whites,
                blacks: raw.tone.blacks,
                temperature: raw.white_balance.temperature,
                tint: raw.white_balance.tint,
                hsl: raw.hsl,
            },
            lut: None,
        })
    }

    /// Serialize the preset to a TOML string.
    pub fn to_toml(&self) -> Result<String> {
        let raw = PresetRaw {
            metadata: self.metadata.clone(),
            tone: ToneSection {
                exposure: self.params.exposure,
                contrast: self.params.contrast,
                highlights: self.params.highlights,
                shadows: self.params.shadows,
                whites: self.params.whites,
                blacks: self.params.blacks,
            },
            white_balance: WhiteBalanceSection {
                temperature: self.params.temperature,
                tint: self.params.tint,
            },
            lut: LutSection::default(),
            hsl: self.params.hsl.clone(),
        };
        toml::to_string_pretty(&raw).map_err(|e| OxirawError::Preset(e.to_string()))
    }

    /// Load a preset from a TOML file.
    ///
    /// If the preset contains a `[lut]` section with a `path`, the LUT file
    /// is resolved relative to the preset file's directory and loaded.
    pub fn load_from_file(path: &std::path::Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let raw: PresetRaw =
            toml::from_str(&content).map_err(|e| OxirawError::Preset(e.to_string()))?;

        let base_dir = path.parent().unwrap_or(std::path::Path::new("."));

        let lut = if let Some(lut_path_str) = &raw.lut.path {
            let lut_path = base_dir.join(lut_path_str);
            Some(crate::lut::Lut3D::from_cube_file(&lut_path)?)
        } else {
            None
        };

        Ok(Self {
            metadata: raw.metadata,
            params: Parameters {
                exposure: raw.tone.exposure,
                contrast: raw.tone.contrast,
                highlights: raw.tone.highlights,
                shadows: raw.tone.shadows,
                whites: raw.tone.whites,
                blacks: raw.tone.blacks,
                temperature: raw.white_balance.temperature,
                tint: raw.white_balance.tint,
                hsl: raw.hsl,
            },
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
        assert_eq!(preset.params, Parameters::default());
        assert_eq!(preset.metadata.name, "");
    }

    #[test]
    fn serialize_contains_expected_keys() {
        let mut preset = Preset::default();
        preset.metadata.name = "Test".into();
        preset.params.exposure = 1.5;
        preset.params.temperature = 30.0;

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
        assert_eq!(preset.params.exposure, 0.5);
        assert_eq!(preset.params.contrast, 15.0);
        assert_eq!(preset.params.highlights, -30.0);
        assert_eq!(preset.params.shadows, 25.0);
        assert_eq!(preset.params.temperature, 6200.0);
        assert_eq!(preset.params.tint, 10.0);
    }

    #[test]
    fn roundtrip_serialize_deserialize() {
        let mut preset = Preset::default();
        preset.metadata.name = "Roundtrip".into();
        preset.params.exposure = 2.0;
        preset.params.contrast = -10.0;
        preset.params.temperature = 50.0;
        preset.params.tint = -5.0;

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
        assert_eq!(preset.params.exposure, 1.0);
        assert_eq!(preset.params.contrast, 0.0);
        assert_eq!(preset.params.highlights, 0.0);
        assert_eq!(preset.params.temperature, 0.0);
    }

    #[test]
    fn invalid_toml_returns_error() {
        let result = Preset::from_toml("this is not valid toml {{{{");
        assert!(result.is_err());
    }

    #[test]
    fn file_save_and_load_roundtrip() {
        let temp_path = std::env::temp_dir().join("oxiraw_test_preset.toml");

        let mut preset = Preset::default();
        preset.metadata.name = "File Test".into();
        preset.params.exposure = 1.5;
        preset.params.contrast = 20.0;

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
        let cube_path = temp_dir.join("oxiraw_preset_test.cube");
        let preset_path = temp_dir.join("oxiraw_preset_test.toml");

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
        assert_eq!(preset.params.exposure, 0.5);
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
        let mut preset = Preset::default();
        preset.params.hsl.red.hue = 15.0;
        preset.params.hsl.green.saturation = -30.0;
        preset.params.hsl.blue.luminance = 20.0;
        let toml_str = preset.to_toml().unwrap();
        let parsed = Preset::from_toml(&toml_str).unwrap();
        assert_eq!(preset.params.hsl, parsed.params.hsl);
    }

    #[test]
    fn preset_missing_hsl_defaults_to_zero() {
        let toml_str = "[metadata]\nname = \"No HSL\"\n\n[tone]\nexposure = 1.0\n";
        let preset = Preset::from_toml(toml_str).unwrap();
        assert!(preset.params.hsl.is_default());
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
        assert_eq!(preset.params.hsl.red.hue, 10.0);
        assert_eq!(preset.params.hsl.red.saturation, 0.0);
        assert!(preset.params.hsl.green == crate::engine::HslChannel::default());
    }

    #[test]
    fn preset_with_missing_lut_file_returns_error() {
        let temp_dir = std::env::temp_dir();
        let preset_path = temp_dir.join("oxiraw_missing_lut_test.toml");
        std::fs::write(
            &preset_path,
            "[metadata]\nname = \"Bad\"\n\n[lut]\npath = \"nonexistent.cube\"\n",
        )
        .unwrap();

        let result = Preset::load_from_file(&preset_path);
        assert!(result.is_err());

        let _ = std::fs::remove_file(&preset_path);
    }
}
