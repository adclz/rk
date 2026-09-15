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

    /// Directory of the loaded library, if any: named through
    /// `RK_STDLIB_PATH`, else found beside the executable.
    #[returns(as_ref)]
    pub library_path: Option<PathBuf>,

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
        let (workspace_folder, library_path, config_file) =
            resolve_all(workspace_uri, &encoding, file_errors, notices);

        Self::builder(workspace_folder, library_path, config_file, encoding)
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
        let (workspace_folder, library_path, config_file) =
            resolve_all(workspace_uri, &self.encoding(db), file_errors, notices);

        self.set_workspace_folder(db).to(workspace_folder);
        self.set_library_path(db).to(library_path);
        self.set_config_file(db).to(config_file);
    }
}

fn resolve_all(
    workspace_uri: Option<Url>,
    _encoding: &PositionEncodingKind,
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

    // 3. Parse config file — report parse errors. The config has no say in
    //    library resolution (see step 4).
    if let Some(path) = &config_file
        && let Ok(source) = std::fs::read_to_string(path)
        && let Err(e) = crate::config_file::parse_config(&source)
        && Url::from_file_path(path).is_ok()
    {
        // The config parser reports a byte span; convert it to an LSP range by
        // counting line breaks in the already-read `source`.
        let range = e
            .span()
            .map(|span| byte_range_to_lsp(&source, span))
            .unwrap_or_default();
        file_errors.push(IdeDiagnostic::new(Diagnostic {
            range,
            severity: Some(DiagnosticSeverity::ERROR),
            source: Some("rk-lsp".to_string()),
            message: e.message().to_string(),
            ..Default::default()
        }));
    }

    // 4. Resolve the library. Not finding one is never fatal: uses of
    // library names fail to resolve like any other unknown name.
    use crate::loader::LibraryPathResolution;
    let library_path = match crate::loader::resolve_library_path(workspace_folder.as_deref()) {
        LibraryPathResolution::Found { dir, .. } => Some(dir),
        LibraryPathResolution::Disabled => None,
        LibraryPathResolution::NotFound { probed } => {
            notices.push(ConfigurationNotice::LibraryNotFound { probed });
            None
        }
        LibraryPathResolution::Invalid(value) => {
            notices.push(ConfigurationNotice::LibraryPathInvalid { value });
            None
        }
    };

    (workspace_folder, library_path, config_file)
}

/// Configuration-level issue with no specific file location.
/// Reported via `window/showMessage`.
#[derive(Clone, Debug)]
pub enum ConfigurationNotice {
    InvalidWorkspaceUri { uri: Url },
    ConfigFileNotFound { path: PathBuf },
    /// Nothing named a library and none was found beside the executable.
    LibraryNotFound { probed: Vec<PathBuf> },
    /// `RK_STDLIB_PATH` is set to something that is not a readable directory.
    LibraryPathInvalid { value: String },
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
            ConfigurationNotice::LibraryNotFound { probed } => {
                writeln!(
                    f,
                    "No standard library found, so Std.* names will not resolve."
                )?;
                if !probed.is_empty() {
                    writeln!(f, "Looked beside the executable in:")?;
                    for path in probed {
                        writeln!(f, "  {}", path.display())?;
                    }
                }
                write!(
                    f,
                    "Set RK_STDLIB_PATH in the environment or in a .env file at \
                     the workspace root; an empty string silences this message."
                )
            }
            ConfigurationNotice::LibraryPathInvalid { value } => {
                write!(
                    f,
                    "RK_STDLIB_PATH points to '{value}', which is not a readable \
                     directory.\nNo standard library loaded"
                )
            }
        }
    }
}

/// Converts a byte `span` in `source` into an LSP range, for TOML parse
/// errors; columns are byte offsets, correct for ASCII config files.
fn byte_range_to_lsp(source: &str, span: std::ops::Range<usize>) -> auto_lsp::lsp_types::Range {
    let position = |offset: usize| -> auto_lsp::lsp_types::Position {
        let mut line = 0u32;
        let mut line_start = 0usize;
        for (i, b) in source.bytes().enumerate() {
            if i >= offset {
                break;
            }
            if b == b'\n' {
                line += 1;
                line_start = i + 1;
            }
        }
        auto_lsp::lsp_types::Position {
            line,
            character: offset.saturating_sub(line_start) as u32,
        }
    };
    auto_lsp::lsp_types::Range {
        start: position(span.start),
        end: position(span.end),
    }
}
