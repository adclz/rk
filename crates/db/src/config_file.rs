use std::path::Path;

use serde::Deserialize;

use crate::{workspace::Workspace, WorkspaceDataBase};

/// Parsed and validated workspace configuration from `config.toml`; serde
/// rejects unknown keys.
#[derive(Default, Clone, Debug, PartialEq, Eq, Hash, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub project: ProjectInfo,
    pub stdlib_path: Option<String>,
    pub output: OutputConfig,
}

#[derive(Default, Clone, Debug, PartialEq, Eq, Hash, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectInfo {
    pub name: String,
    pub version: u32,
}
 
#[derive(Default, Clone, Debug, PartialEq, Eq, Hash, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutputConfig {
    pub directory: String,
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
stdlib_path = "/custom/stdlib"

[project]
name = "Lisa"
version = 1

[output]
directory = "build"
"#
    }

    #[test]
    fn valid_full_config() {
        let config: Config = parse_config(full_config()).unwrap();
        assert_eq!(config.project.name, "Lisa");
        assert_eq!(config.project.version, 1);
        assert_eq!(config.stdlib_path.as_deref(), Some("/custom/stdlib"));
        assert_eq!(config.output.directory, "build");
    }

    #[test]
    fn valid_without_stdlib_path() {
        let config: Config = parse_config(
            r#"
[project]
name = "Test"
version = 2

[output]
directory = "out"
"#,
        )
        .unwrap();
        assert_eq!(config.project.name, "Test");
        assert_eq!(config.project.version, 2);
        assert_eq!(config.stdlib_path, None);
        assert_eq!(config.output.directory, "out");
    }

    #[test]
    fn missing_project_section() {
        let err = parse_config(
            r#"
[output]
directory = "build"
"#,
        )
        .unwrap_err();
        assert!(err.message().contains("missing field `project`"));
    }

    #[test]
    fn missing_output_section() {
        let err = parse_config(
            r#"
[project]
name = "Test"
version = 1
"#,
        )
        .unwrap_err();
        assert!(err.message().contains("missing field `output`"));
    }

    #[test]
    fn missing_project_name() {
        let err = parse_config(
            r#"
[project]
version = 1

[output]
directory = "build"
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

[output]
directory = "build"
"#,
        )
        .unwrap_err();
        assert!(err.message().contains("missing field `version`"));
    }

    #[test]
    fn wrong_type_version_is_string() {
        let err = parse_config(
            r#"
[project]
name = "Test"
version = "one"

[output]
directory = "build"
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
version = 1
unknown_key = true

[output]
directory = "build"
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
version = 1
extra = "nope"

[output]
directory = "build"
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
    fn error_has_span() {
        let input = r#"
[project]
name = "Test"
version = "bad"

[output]
directory = "build"
"#;
        let err = parse_config(input).unwrap_err();
        // toml should provide a byte span for the invalid value
        assert!(err.span().is_some());
    }
}
