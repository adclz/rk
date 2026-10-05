// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use std::{collections::BTreeMap, hash::Hash, path::Path};

use serde::Deserialize;

use crate::{WorkspaceDataBase, workspace::Workspace};

/// Parsed and validated workspace configuration from `config.toml`; serde
/// rejects unknown keys.
#[derive(Default, Clone, Debug, PartialEq, Eq, Hash, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub project: ProjectInfo,
    pub settings: Option<SettingsConfig>,
    pub linter: Option<LinterConfig>,
}

impl Config {
    /// Convenience: get output config from settings.
    pub fn output(&self) -> Option<&OutputConfig> {
        self.settings.as_ref()?.output.as_ref()
    }
}

#[derive(Default, Clone, Debug, PartialEq, Eq, Hash, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SettingsConfig {
    pub output: Option<OutputConfig>,
    /// WASM optimization level: 0-4, "s" (size), "z" (aggressive size).
    /// Requires wasm-opt. Default: no optimization.
    pub opt_level: Option<String>,
    /// The size of the stack recursive calls push their frames on, in bytes:
    /// `1048576`. Without it, the stack holds the largest frame and 64 KiB
    /// more.
    pub stack_size: Option<StackSize>,
}

/// A stack size from `config.toml`: a number of bytes, more than none and
/// at most the 4 GiB a module can address.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StackSize(u64);

impl StackSize {
    /// The memory a module can address.
    pub const MAX: u64 = 4 << 30;

    pub fn bytes(self) -> u64 {
        self.0
    }
}

impl std::fmt::Display for StackSize {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} bytes", self.0)
    }
}

impl<'de> Deserialize<'de> for StackSize {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Size;
        impl serde::de::Visitor<'_> for Size {
            type Value = StackSize;

            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a number of bytes, like 1048576")
            }

            fn visit_i64<E: serde::de::Error>(self, bytes: i64) -> Result<StackSize, E> {
                match u64::try_from(bytes) {
                    Ok(bytes) => self.visit_u64(bytes),
                    Err(_) => Err(E::custom("the stack cannot be a negative number of bytes")),
                }
            }

            fn visit_u64<E: serde::de::Error>(self, bytes: u64) -> Result<StackSize, E> {
                match bytes {
                    0 => Err(E::custom("the stack cannot be empty")),
                    _ if bytes <= StackSize::MAX => Ok(StackSize(bytes)),
                    _ => Err(E::custom(
                        "the stack can be at most 4294967296 bytes, the 4 GiB a module can address",
                    )),
                }
            }
        }
        deserializer.deserialize_u64(Size)
    }
}

#[derive(Default, Clone, Debug, PartialEq, Eq, Hash, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectInfo {
    pub name: String,
    pub version: String,
}

#[derive(Default, Clone, Debug, PartialEq, Eq, Hash, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutputConfig {
    pub directory: String,
}

#[derive(Default, Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LinterConfig {
    /// Which rules run before `rules` is consulted; absent means
    /// [`Select::Recommended`].
    pub select: Option<Select>,
    /// Per-rule overrides keyed by rule name, applied on top of `select`.
    /// Example: `{ unused-variable = false }` disables that lint, and
    /// `{ yoda-condition = true }` opts one in that `select` left out.
    pub rules: Option<BTreeMap<String, bool>>,
}

/// The baseline set of rules, before per-rule overrides.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Select {
    /// Every rule the linter has.
    All,
    /// The rules that report probable bugs rather than style — the default.
    #[default]
    Recommended,
    /// No rule runs unless `rules` names it explicitly.
    None,
}

impl Hash for LinterConfig {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.select.hash(state);
        match &self.rules {
            None => 0u8.hash(state),
            Some(rules) => {
                1u8.hash(state);
                rules.len().hash(state);
                for (k, v) in rules {
                    k.hash(state);
                    v.hash(state);
                }
            }
        }
    }
}

impl LinterConfig {
    /// The explicit override for `name`, if the workspace stated one.
    pub fn rule_override(&self, name: &str) -> Option<bool> {
        self.rules.as_ref()?.get(name).copied()
    }

    pub fn select(&self) -> Select {
        self.select.unwrap_or_default()
    }
}

/// Parses a TOML string into a `Config`, keeping the toml error for its
/// `.span()`.
pub fn parse_config(content: &str) -> Result<Config, toml::de::Error> {
    toml::from_str(content)
}

#[salsa::tracked(returns(ref))]
pub fn get_config(db: &dyn WorkspaceDataBase) -> Config {
    let Some(config) = Workspace::try_get(db) else {
        return Config::default();
    };
    let Some(path) = config.config_file(db) else {
        return Config::default();
    };

    match load_config(path) {
        Ok(config) => config,
        Err(e) => {
            eprintln!("Failed to parse config.toml: {}", e);
            Config::default()
        }
    }
}

fn load_config(path: &Path) -> Result<Config, Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(path)?;
    let config = parse_config(&content)?;
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn full_config() -> &'static str {
        r#"
[project]
name = "Lisa"
version = "1"

[settings.output]
directory = "build"
"#
    }

    #[test]
    fn valid_full_config() {
        let config: Config = parse_config(full_config()).unwrap();
        assert_eq!(config.project.name, "Lisa");
        assert_eq!(config.project.version, "1");
        assert_eq!(config.output().unwrap().directory, "build");
    }

    #[test]
    fn valid_with_output_only() {
        let config: Config = parse_config(
            r#"
[project]
name = "Test"
version = "2"

[settings.output]
directory = "out"
"#,
        )
        .unwrap();
        assert_eq!(config.project.name, "Test");
        assert_eq!(config.project.version, "2");
        assert_eq!(config.output().unwrap().directory, "out");
    }

    #[test]
    fn valid_without_settings_section() {
        let config: Config = parse_config(
            r#"
[project]
name = "Test"
version = "1"
"#,
        )
        .unwrap();
        assert_eq!(config.project.name, "Test");
        assert!(config.output().is_none());
    }

    /// Library acquisition is selected by `RK_STDLIB_PATH` alone; both former
    /// keys are rejected so a stale config fails loudly.
    #[test]
    fn removed_stdlib_keys_are_rejected() {
        for key in ["disable_stdlib = true", "stdlib_path = \"/x\""] {
            let err = parse_config(&format!(
                "[project]\nname = \"T\"\nversion = \"1\"\n\n[settings]\n{key}\n"
            ))
            .unwrap_err();
            assert!(err.message().contains("unknown field"), "{}", err.message());
        }
    }

    #[test]
    fn missing_project_section() {
        let err = parse_config(
            r#"
[settings]
opt_level = "2"
"#,
        )
        .unwrap_err();
        assert!(err.message().contains("missing field `project`"));
    }

    #[test]
    fn missing_project_name() {
        let err = parse_config(
            r#"
[project]
version = "1"
"#,
        )
        .unwrap_err();
        assert!(err.message().contains("missing field `name`"));
    }

    #[test]
    fn missing_project_version() {
        let err = parse_config(
            r#"
[project]
name = "Test"
"#,
        )
        .unwrap_err();
        assert!(err.message().contains("missing field `version`"));
    }

    #[test]
    fn wrong_type_name_is_integer() {
        let err = parse_config(
            r#"
[project]
name = 123
version = "1"
"#,
        )
        .unwrap_err();
        assert!(err.message().contains("invalid type"));
    }

    #[test]
    fn unknown_field_rejected() {
        let err = parse_config(
            r#"
[project]
name = "Test"
version = "1"
unknown_key = true
"#,
        )
        .unwrap_err();
        assert!(err.message().contains("unknown field"));
    }

    #[test]
    fn unknown_top_level_field_rejected() {
        let err = parse_config(
            r#"
[project]
name = "Test"
version = "1"

output = "nope"
"#,
        )
        .unwrap_err();
        assert!(err.message().contains("unknown field"));
    }

    #[test]
    fn empty_config_fails() {
        let err = parse_config("").unwrap_err();
        assert!(err.message().contains("missing field"));
    }

    #[test]
    fn valid_linter_config() {
        let config: Config = parse_config(
            r#"
[project]
name = "Test"
version = "1"

[linter]

[linter.rules]
unused-variable = false
shadowing-variable = true
"#,
        )
        .unwrap();
        let linter = config.linter.unwrap();
        assert_eq!(linter.rule_override("unused-variable"), Some(false));
        assert_eq!(linter.rule_override("shadowing-variable"), Some(true));
        // An unlisted rule states nothing here; `select` decides it, and only
        // the linter crate knows which rules that covers.
        assert_eq!(linter.rule_override("duplicate-var-section"), None);
        assert_eq!(linter.select(), Select::Recommended);
    }

    #[test]
    fn linter_section_without_rules() {
        let config: Config = parse_config(
            r#"
[project]
name = "Test"
version = "1"

[linter]
"#,
        )
        .unwrap();
        let linter = config.linter.unwrap();
        // A bare section tunes nothing: the baseline stays the default.
        assert_eq!(linter.select(), Select::Recommended);
        assert_eq!(linter.rule_override("unused-variable"), None);
    }

    #[test]
    fn no_linter_section() {
        let config: Config = parse_config(
            r#"
[project]
name = "Test"
version = "1"
"#,
        )
        .unwrap();
        // Absent means "not tuned", NOT "off": `collect_diagnostics` reads
        // this as the default config so the recommended rules still run.
        assert!(config.linter.is_none());
        assert_eq!(
            config.linter.unwrap_or_default().select(),
            Select::Recommended
        );
    }

    #[test]
    fn select_is_parsed_kebab_case() {
        for (text, want) in [
            ("all", Select::All),
            ("recommended", Select::Recommended),
            ("none", Select::None),
        ] {
            let config: Config = parse_config(&format!(
                "[project]\nname = \"T\"\nversion = \"1\"\n[linter]\nselect = \"{text}\"\n"
            ))
            .unwrap();
            assert_eq!(config.linter.unwrap().select(), want);
        }
    }

    #[test]
    fn an_unknown_select_is_refused() {
        // A typo must not silently fall back to a baseline nobody asked for.
        let err = parse_config(
            "[project]\nname = \"T\"\nversion = \"1\"\n[linter]\nselect = \"bogus\"\n",
        )
        .unwrap_err();
        assert!(err.to_string().contains("unknown variant"), "{err}");
    }

    #[test]
    fn error_has_span() {
        // name is a String field; providing an integer triggers a type error with a span
        let input = r#"
[project]
name = 123
version = "1"

[output]
directory = "build"
"#;
        let err = parse_config(input).unwrap_err();
        assert!(err.span().is_some());
    }

    /// `stack_size` is a number of bytes, more than none and at most 4 GiB.
    #[test]
    fn stack_size_is_a_number_of_bytes() {
        let size = |value: &str| {
            parse_config(&format!(
                "[project]\nname = \"T\"\nversion = \"1\"\n\n[settings]\nstack_size = {value}\n"
            ))
            .map(|config| config.settings.unwrap().stack_size.unwrap().bytes())
            .map_err(|e| e.message().to_string())
        };
        assert_eq!(size("65536"), Ok(65536));
        assert_eq!(size("4294967296"), Ok(4 << 30));
        for (value, message) in [
            ("4294967297", "at most 4294967296 bytes"),
            ("0", "cannot be empty"),
            ("-1", "negative"),
            ("\"1 MiB\"", "expected a number of bytes"),
            ("1.5", "expected a number of bytes"),
        ] {
            let err = size(value).unwrap_err();
            assert!(err.contains(message), "{value}: {err}");
        }
    }
}
