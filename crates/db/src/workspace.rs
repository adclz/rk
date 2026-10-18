// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use std::{fmt::Display, path::PathBuf};

use auto_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, PositionEncodingKind, Url};
use ide_diagnostic::IdeDiagnostic;
use salsa::{Durability, Setter};

use crate::RootDatabase;
use crate::dependencies::{Fetch, Library};

/// Pure data store for workspace configuration, created and updated by
/// the loader.
#[salsa::input(singleton, debug)]
pub struct Workspace {
    #[returns(as_ref)]
    pub workspace_folder: Option<PathBuf>,

    /// The libraries read beside the workspace's own files, in the order
    /// they are read: the standard library, when there is one, and then the
    /// `[dependencies]` of `config.toml`. A library's place here is its
    /// [`crate::dependencies::LibraryId`].
    #[returns(ref)]
    pub libraries: Vec<Library>,

    #[returns(as_ref)]
    pub config_file: Option<PathBuf>,

    pub encoding: PositionEncodingKind,
}

impl Workspace {
    /// The standard library, when one is loaded: the one built into the
    /// compiler, or the folder `RK_STDLIB_PATH` names.
    pub fn standard_library(self, db: &dyn salsa::Database) -> Option<&Library> {
        self.libraries(db).iter().find(|library| library.standard)
    }

    pub fn init_or_update(
        db: &mut RootDatabase,
        workspace_uri: Option<Url>,
        encoding: PositionEncodingKind,
        fetch: Option<Fetch<'_>>,
        file_errors: &mut Vec<IdeDiagnostic>,
        notices: &mut Vec<ConfigurationNotice>,
    ) -> Self {
        match Self::try_get(db) {
            Some(config) => {
                config.update(db, workspace_uri, fetch, file_errors, notices);
                config
            }
            None => Self::create(db, workspace_uri, encoding, fetch, file_errors, notices),
        }
    }

    fn create(
        db: &RootDatabase,
        workspace_uri: Option<Url>,
        encoding: PositionEncodingKind,
        fetch: Option<Fetch<'_>>,
        file_errors: &mut Vec<IdeDiagnostic>,
        notices: &mut Vec<ConfigurationNotice>,
    ) -> Self {
        let (workspace_folder, libraries, config_file) =
            resolve_all(workspace_uri, fetch, file_errors, notices);

        Self::builder(workspace_folder, libraries, config_file, encoding)
            .durability(Durability::HIGH)
            .new(db)
    }

    fn update(
        &self,
        db: &mut RootDatabase,
        workspace_uri: Option<Url>,
        fetch: Option<Fetch<'_>>,
        file_errors: &mut Vec<IdeDiagnostic>,
        notices: &mut Vec<ConfigurationNotice>,
    ) {
        let (workspace_folder, libraries, config_file) =
            resolve_all(workspace_uri, fetch, file_errors, notices);

        self.set_workspace_folder(db).to(workspace_folder);
        self.set_libraries(db).to(libraries);
        self.set_config_file(db).to(config_file);
    }
}

fn resolve_all(
    workspace_uri: Option<Url>,
    fetch: Option<Fetch<'_>>,
    file_errors: &mut Vec<IdeDiagnostic>,
    notices: &mut Vec<ConfigurationNotice>,
) -> (Option<PathBuf>, Vec<Library>, Option<PathBuf>) {
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
    //    where the standard library is (see step 4), and names the other
    //    libraries (step 5).
    let mut config = None;
    if let Some(path) = &config_file
        && let Ok(source) = std::fs::read_to_string(path)
    {
        match crate::config_file::parse_config(&source) {
            Ok(parsed) => config = Some((source, parsed)),
            Err(e) if Url::from_file_path(path).is_ok() => {
                // The config parser reports a byte span; convert it to an LSP
                // range by counting line breaks in the already-read `source`.
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
            Err(_) => {}
        }
    }

    // 4. Resolve the standard library. Not having one is never fatal: uses
    // of library names fail to resolve like any other unknown name.
    use crate::stdlib::Standard;
    let standard = match crate::stdlib::locate(workspace_folder.as_deref()) {
        Standard::Found { library, .. } => Some(library),
        Standard::Disabled => None,
        Standard::NotAFolder(value) => {
            notices.push(ConfigurationNotice::LibraryPathInvalid { value });
            None
        }
        Standard::NotANamespaceName { dir, name } => {
            notices.push(ConfigurationNotice::LibraryNameInvalid { dir, name });
            None
        }
        Standard::NotInstalled(reason) => {
            notices.push(ConfigurationNotice::LibraryNotInstalled { reason });
            None
        }
    };

    // 5. Resolve the libraries the configuration names. One the cache
    // lacks is fetched by the caller, or reported when it fetches nothing.
    let libraries = match (&workspace_folder, config) {
        (Some(workspace), Some((source, config))) => {
            let cache = crate::cache::root();
            let resolution =
                crate::dependencies::resolve(workspace, &config, standard, cache.as_deref(), fetch);
            file_errors.extend(
                resolution
                    .problems
                    .into_iter()
                    .map(|problem| dependency_diagnostic(&source, problem)),
            );
            resolution.libraries
        }
        _ => standard.into_iter().collect(),
    };

    (workspace_folder, libraries, config_file)
}

/// A dependency that could not be resolved, as a diagnostic on the entry of
/// `config.toml` it comes from. `source` is the text of that file.
pub fn dependency_diagnostic(source: &str, problem: crate::dependencies::Problem) -> IdeDiagnostic {
    let mut diagnostic = IdeDiagnostic::new(Diagnostic {
        range: byte_range_to_lsp(source, problem.span),
        severity: Some(DiagnosticSeverity::ERROR),
        source: Some("rk-lsp".to_string()),
        message: problem.message,
        ..Default::default()
    });
    for note in problem.notes {
        diagnostic.with_note(note);
    }
    if let Some(help) = problem.help {
        diagnostic.with_help(help);
    }
    diagnostic
}

/// Configuration-level issue with no specific file location.
/// Reported via `window/showMessage`.
#[derive(Clone, Debug)]
pub enum ConfigurationNotice {
    InvalidWorkspaceUri {
        uri: Url,
    },
    ConfigFileNotFound {
        path: PathBuf,
    },
    /// The standard library could not be written in the cache.
    LibraryNotInstalled {
        reason: String,
    },
    /// `RK_STDLIB_PATH` is set to something that is not a readable directory.
    LibraryPathInvalid {
        value: String,
    },
    /// The library `RK_STDLIB_PATH` names has a name no namespace can have.
    LibraryNameInvalid {
        dir: PathBuf,
        name: String,
    },
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
            ConfigurationNotice::LibraryNotInstalled { reason } => {
                write!(
                    f,
                    "the standard library could not be installed, so Std.* names \
                     will not resolve: {reason}"
                )
            }
            ConfigurationNotice::LibraryNameInvalid { dir, name } => {
                write!(
                    f,
                    "the library at '{}' is named '{name}', which is not a namespace name: \
                     a library's name is the namespace its declarations are in.\nNo standard \
                     library loaded",
                    dir.display()
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
