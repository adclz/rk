use std::{fmt::Display, path::PathBuf};

use auto_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, PositionEncodingKind, Url};
use ide_diagnostic::IdeDiagnostic;
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

    pub encoding: PositionEncodingKind,
}

impl Workspace {
    pub fn init_or_update(
        db: &mut RootDatabase,
        workspace_uri: Option<Url>,
        encoding: PositionEncodingKind,
        file_errors: &mut Vec<IdeDiagnostic>,
        notices: &mut Vec<ConfigurationNotice>,
    ) -> Self {
        match Self::try_get(db) {
            Some(config) => {
                config.update(db, workspace_uri, file_errors, notices);
                config
            }
            None => Self::create(db, workspace_uri, encoding, file_errors, notices),
        }
    }

    fn create(
        db: &RootDatabase,
        workspace_uri: Option<Url>,
        encoding: PositionEncodingKind,
        file_errors: &mut Vec<IdeDiagnostic>,
        notices: &mut Vec<ConfigurationNotice>,
    ) -> Self {
        let (workspace_folder, stdlib_path, config_file) =
            resolve_all(workspace_uri, &encoding, file_errors, notices);

        Self::builder(workspace_folder, stdlib_path, config_file, encoding)
            .durability(Durability::HIGH)
            .new(db)
    }

    fn update(
        &self,
        db: &mut RootDatabase,
        workspace_uri: Option<Url>,
        file_errors: &mut Vec<IdeDiagnostic>,
        notices: &mut Vec<ConfigurationNotice>,
    ) {
        let (workspace_folder, stdlib_path, config_file) =
            resolve_all(workspace_uri, &self.encoding(db), file_errors, notices);

        self.set_workspace_folder(db).to(workspace_folder);
        self.set_stdlib_path(db).to(stdlib_path);
        self.set_config_file(db).to(config_file);
    }
}

fn resolve_all(
    workspace_uri: Option<Url>,
    encoding: &PositionEncodingKind,
    file_errors: &mut Vec<IdeDiagnostic>,
    notices: &mut Vec<ConfigurationNotice>,
) -> (Option<PathBuf>, Option<PathBuf>, Option<PathBuf>) {
    // 1. Resolve workspace folder from URI
    let workspace_folder = match workspace_uri {
        Some(uri) => match uri.to_file_path() {
            Ok(path) => Some(path),
            Err(_) => {
                notices.push(ConfigurationNotice::InvalidWorkspaceUri { uri });
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
                notices.push(ConfigurationNotice::ConfigFileNotFound {
                    path: ws.join("config.toml"),
                });
            }
            resolved
        }
        None => None,
    };

    // 3. Parse config file — extract user stdlib_path and disable_stdlib, report parse errors
    let (user_stdlib_path, disable_stdlib) = match &config_file {
        Some(path) => match std::fs::read_to_string(path) {
            Ok(source) => match crate::config_file::parse_config(&source) {
                Ok(config) => (
                    config
                        .stdlib_path()
                        .map(PathBuf::from)
                        .filter(|p| p.exists()),
                    config.disable_stdlib(),
                ),
                Err(e) => {
                    if let Ok(_uri) = Url::from_file_path(path) {
                        // Reuse the already-read `source` for offset→line/col conversion.
                        let range = e
                            .span()
                            .and_then(|span| {
                                let parsers = ast::RK_PARSER.get("st")?;
                                let tree = parsers.parser.write().parse("".as_bytes(), None)?;
                                let doc = auto_lsp::core::document::Document::new(
                                    source.clone(),
                                    tree,
                                    Some(encoding),
                                );
                                doc.range_at(span).ok()
                            })
                            .unwrap_or_default();
                        file_errors.push(IdeDiagnostic::new(Diagnostic {
                            range,
                            severity: Some(DiagnosticSeverity::ERROR),
                            source: Some("rk-lsp".to_string()),
                            message: e.message().to_string(),
                            ..Default::default()
                        }));
                    }
                    (None, false)
                }
            },
            Err(_) => (None, false),
        },
        None => (None, false),
    };

    // 4. Resolve stdlib: skip if disable_stdlib is set, otherwise user override > default
    let stdlib_path = if disable_stdlib {
        None
    } else {
        let path = user_stdlib_path.or_else(crate::loader::resolve_stdlib_path);
        if path.is_none() {
            notices.push(ConfigurationNotice::StdlibNotFound);
        }
        path
    };

    (workspace_folder, stdlib_path, config_file)
}

/// Configuration-level issue with no specific file location.
/// Reported via `window/showMessage`.
#[derive(Clone, Debug)]
pub enum ConfigurationNotice {
    InvalidWorkspaceUri { uri: Url },
    ConfigFileNotFound { path: PathBuf },
    StdlibNotFound,
}

impl Display for ConfigurationNotice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigurationNotice::InvalidWorkspaceUri { uri } => {
                write!(f, "invalid workspace URI: {}", uri)
            }
            ConfigurationNotice::ConfigFileNotFound { path } => {
                write!(
                    f,
                    "configuration file not found at path: {}",
                    path.display()
                )
            }
            ConfigurationNotice::StdlibNotFound => {
                write!(f, "standard library not found")
            }
        }
    }
}
