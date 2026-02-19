use std::{fmt::Display, path::PathBuf};

use auto_lsp::lsp_types::{PositionEncodingKind, Url};
use salsa::{Durability, Setter};

use crate::RootDatabase;

/// Pure data store for workspace configuration, created and updated by
/// the loader.
#[salsa::input(singleton, debug)]
pub struct Workspace {
    #[returns(as_ref)]
    pub workspace_folder: Option<PathBuf>,

    #[returns(as_ref)]
    pub stdlib_path: Option<PathBuf>,

    #[returns(as_ref)]
    pub config_file: Option<PathBuf>,

    pub encoding: PositionEncodingKind
} 

impl Workspace {
    pub fn init_or_update(
        db: &mut RootDatabase,
        workspace_uri: Option<Url>,
        encoding: PositionEncodingKind,
        errors: &mut Vec<ConfigurationError>,
    ) -> Self {
        match Self::try_get(db) {
            Some(config) => {
                config.update(db, workspace_uri, errors);
                config
            }
            None => Self::create(db, workspace_uri, encoding, errors),
        }
    }

    fn create(
        db: &RootDatabase,
        workspace_uri: Option<Url>,
        encoding: PositionEncodingKind,
        errors: &mut Vec<ConfigurationError>,
    ) -> Self {
        let (workspace_folder, stdlib_path, config_file) = resolve_all(workspace_uri, errors);

        Self::builder(workspace_folder, stdlib_path, config_file, encoding)
            .durability(Durability::HIGH)
            .new(db)
    }

    fn update(
        &self,
        db: &mut RootDatabase,
        workspace_uri: Option<Url>,
        errors: &mut Vec<ConfigurationError>,
    ) {
        let (workspace_folder, stdlib_path, config_file) = resolve_all(workspace_uri, errors);

        self.set_workspace_folder(db).to(workspace_folder);
        self.set_stdlib_path(db).to(stdlib_path);
        self.set_config_file(db).to(config_file);
    }
}

fn resolve_all(
    workspace_uri: Option<Url>,
    errors: &mut Vec<ConfigurationError>,
) -> (Option<PathBuf>, Option<PathBuf>, Option<PathBuf>) {
    // 1. Resolve workspace folder from URI
    let workspace_folder = match workspace_uri {
        Some(uri) => match uri.to_file_path() {
            Ok(path) => Some(path),
            Err(_) => {
                errors.push(ConfigurationError::InvalidWorkspaceUri { uri });
                None
            }
        },
        None => None,
    };

    // 2. Locate config file
    let config_file = match &workspace_folder {
        Some(ws) => {
            let resolved = crate::loader::resolve_config_file(ws);
            if resolved.is_none() {
                errors.push(ConfigurationError::ConfigFileNotFound {
                    path: ws.join("config.toml"),
                });
            }
            resolved
        }
        None => None,
    };

    // 3. Parse config file — extract user stdlib_path, report parse errors
    let user_stdlib_path = match &config_file {
        Some(path) => match std::fs::read_to_string(path) {
            Ok(source) => match crate::config_file::parse_config(&source) {
                Ok(config) => config
                    .stdlib_path
                    .map(PathBuf::from)
                    .filter(|p| p.exists()),
                Err(e) => {
                    errors.push(ConfigurationError::InvalidConfigFile {
                        path: path.clone(),
                        message: e.message().to_string(),
                    });
                    None
                }
            },
            Err(_) => None,
        },
        None => None,
    };

    // 4. Resolve stdlib: user override from config.toml > default
    let stdlib_path = user_stdlib_path.or_else(crate::loader::resolve_stdlib_path);
    if stdlib_path.is_none() {
        errors.push(ConfigurationError::StdlibNotFound);
    }

    (workspace_folder, stdlib_path, config_file)
}

#[derive(Clone, Debug)]
pub enum ConfigurationError {
    InvalidWorkspaceUri { uri: Url },
    ConfigFileNotFound { path: PathBuf },
    InvalidConfigFile {
        path: PathBuf,
        message: String,
    },
    StdlibNotFound,
}

impl Display for ConfigurationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigurationError::InvalidWorkspaceUri { uri } => {
                write!(f, "invalid workspace URI: {}", uri)
            }
            ConfigurationError::ConfigFileNotFound { path } => {
                write!(
                    f,
                    "configuration file not found at path: {}",
                    path.display()
                )
            }
            ConfigurationError::InvalidConfigFile { path, message } => {
                write!(f, "{}: {}", path.display(), message)
            }
            ConfigurationError::StdlibNotFound => {
                write!(f, "standard library not found")
            }
        }
    }
}
