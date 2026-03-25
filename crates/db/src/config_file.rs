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
    /// Convenience: get stdlib_path from settings.
    pub fn stdlib_path(&self) -> Option<&str> {
        self.settings.as_ref()?.stdlib_path.as_deref()
    }

    /// Convenience: check if stdlib is disabled.
    pub fn disable_stdlib(&self) -> bool {
        self.settings.as_ref().is_some_and(|s| s.disable_stdlib.unwrap_or(false))
    }

    /// Convenience: get output config from settings.
    pub fn output(&self) -> Option<&OutputConfig> {
        self.settings.as_ref()?.output.as_ref()
    }
}

#[derive(Default, Clone, Debug, PartialEq, Eq, Hash, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SettingsConfig {
    pub stdlib_path: Option<String>,
    pub disable_stdlib: Option<bool>,
    pub output: Option<OutputConfig>,
    /// WASM optimization level: 0-4, "s" (size), "z" (aggressive size).
    /// Requires wasm-opt. Default: no optimization.
    pub opt_level: Option<String>,
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
    /// Per-rule toggles keyed by rule name.
    /// Rules not listed default to enabled.
    /// Example: `{ unused-variable = false }` disables that lint.
    pub rules: Option<BTreeMap<String, bool>>,
}

impl Hash for LinterConfig {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
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
    pub fn is_enabled(&self, name: &str) -> bool {
        match &self.rules {
            None => true,
            Some(rules) => rules.get(name).copied().unwrap_or(true),
        }
    }
}

/// Parses a TOML string into a `Config`, keeping the toml error for its
/// `.span()`.
pub fn parse_config(content: &str) -> Result<Config, toml::de::Error> {
    toml::from_str(content)
}

#[salsa::tracked(returns(ref))]
pub fn get_config<'db>(db: &'db dyn WorkspaceDataBase) -> Config {
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

[settings]
stdlib_path = "/custom/stdlib"
disable_stdlib = false

[settings.output]
directory = "build"
"#
    }

    #[test]
    fn valid_full_config() {
        let config: Config = parse_config(full_config()).unwrap();
        assert_eq!(config.project.name, "Lisa");
        assert_eq!(config.project.version, "1");
        assert_eq!(config.stdlib_path(), Some("/custom/stdlib"));
        assert!(!config.disable_stdlib());
        assert_eq!(config.output().unwrap().directory, "build");
    }

    #[test]
    fn valid_without_stdlib_path() {
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
        assert_eq!(config.stdlib_path(), None);
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
        assert!(!config.disable_stdlib());
    }

    #[test]
    fn disable_stdlib_enabled() {
        let config: Config = parse_config(
            r#"
[project]
name = "Test"
version = "1"

[settings]
disable_stdlib = true
"#,
        )
        .unwrap();
        assert!(config.disable_stdlib());
    }

    #[test]
    fn missing_project_section() {
        let err = parse_config(
            r#"
[settings]
disable_stdlib = true
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
        assert!(!linter.is_enabled("unused-variable"));
        assert!(linter.is_enabled("shadowing-variable"));
        // Unlisted rules default to enabled
        assert!(linter.is_enabled("duplicate-var-section"));
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
        // All rules enabled by default
        assert!(linter.is_enabled("unused-variable"));
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
        assert!(config.linter.is_none());
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
}
