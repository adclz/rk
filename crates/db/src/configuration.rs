use std::{fmt::Display, path::PathBuf};

use auto_lsp::lsp_types::Url;
use salsa::{Durability, Setter};

use crate::RootDatabase;

/// Pure data store for workspace configuration, created and updated by
/// the loader.
#[salsa::input(singleton, debug)]
pub struct Configuration {
    #[returns(as_ref)]
    pub workspace_folder: Option<PathBuf>,

    #[returns(as_ref)]
    pub stdlib_path: Option<PathBuf>,

    #[returns(as_ref)]
    pub config_file: Option<PathBuf>,
}

impl Configuration {
    pub fn init_or_update(
        db: &mut RootDatabase,
        workspace_uri: Option<Url>,
        errors: &mut Vec<ConfigurationError>,
    ) -> Self {
        match Self::try_get(db) {
            Some(config) => {
                config.update(db, workspace_uri, errors);
                config
            }
            None => Self::create(db, workspace_uri, errors),
        }
    }

    fn create(
        db: &RootDatabase,
        workspace_uri: Option<Url>,
        errors: &mut Vec<ConfigurationError>,
    ) -> Self {
        let (workspace_folder, stdlib_path, config_file) = resolve_all(workspace_uri, errors);

        Self::builder(workspace_folder, stdlib_path, config_file)
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

    let stdlib_path = crate::loader::resolve_stdlib_path();
    if stdlib_path.is_none() {
        errors.push(ConfigurationError::StdlibNotFound);
    }

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

    (workspace_folder, stdlib_path, config_file)
}

#[derive(Clone, Debug)]
pub enum ConfigurationError {
    InvalidWorkspaceUri { uri: Url },
    ConfigFileNotFound { path: PathBuf },
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
            ConfigurationError::StdlibNotFound => {
                write!(f, "standard library not found")
            }
        }
    }
}
