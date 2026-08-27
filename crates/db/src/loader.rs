use std::path::{Path, PathBuf};
use std::sync::Arc;

use auto_lsp::{
    core::document::Document,
    default::db::{FileManager, file::File},
    lsp_types::Url,
    salsa::Durability,
};
use rayon::prelude::*;

use crate::RootDatabase;
use crate::sysroot::LibraryOrigin;
use crate::workspace::Workspace;

type ParseResult = Result<(Url, Arc<Document>), Box<dyn std::error::Error + Send + Sync>>;

// --- Path resolution ---

/// Environment variable naming the library directory; when nothing names
/// one, the library is found beside the executable (see
/// [`crate::sysroot`]).
pub const STDLIB_PATH_ENV: &str = "RK_STDLIB_PATH";

/// Where the library came from, or why there is none.
pub enum LibraryPathResolution {
    /// Nothing named a library and the probe found none; the probed paths
    /// travel with the answer.
    NotFound { probed: Vec<PathBuf> },
    /// The explicit, silent "no library". Either asked for — `RK_STDLIB_PATH`
    /// set to the empty string — or because the workspace IS the library, and
    /// loading it beside itself would duplicate every declaration.
    Disabled,
    /// A directory that exists, and where it came from.
    Found {
        dir: PathBuf,
        origin: LibraryOrigin,
    },
    /// Named as something that is not a readable directory; never silently
    /// ignored.
    Invalid(String),
}

/// Resolves the library directory.
///
/// In order: `RK_STDLIB_PATH` in the environment, the same variable in a
/// `.env` at the workspace root, then the probe beside the executable. A
/// named path wins outright — including when it names nothing, which is the
/// veto that keeps the probe from overriding a deliberate choice.
pub fn resolve_library_path(workspace: Option<&Path>) -> LibraryPathResolution {
    let named = std::env::var(STDLIB_PATH_ENV)
        .ok()
        .map(|value| (LibraryOrigin::Env, value))
        .or_else(|| {
            workspace
                .and_then(read_dotenv_var)
                .map(|value| (LibraryOrigin::Dotenv, value))
        });

    if let Some((origin, value)) = named {
        if value.is_empty() {
            return LibraryPathResolution::Disabled;
        }
        return match std::fs::canonicalize(&value) {
            Ok(dir) if dir.is_dir() => LibraryPathResolution::Found { dir, origin },
            _ => LibraryPathResolution::Invalid(value),
        };
    }

    match crate::sysroot::probe() {
        // A workspace that IS the library loads no library, whatever `.env`
        // says.
        Some((_, dir)) if is_the_workspace(&dir, workspace) => LibraryPathResolution::Disabled,
        Some((origin, dir)) => LibraryPathResolution::Found { dir, origin },
        None => LibraryPathResolution::NotFound {
            probed: crate::sysroot::probed(),
        },
    }
}

/// Whether a resolved library directory is the workspace being compiled,
/// compared canonically.
fn is_the_workspace(library: &Path, workspace: Option<&Path>) -> bool {
    let Some(workspace) = workspace else {
        return false;
    };
    match (library.canonicalize(), workspace.canonicalize()) {
        (Ok(library), Ok(workspace)) => library == workspace,
        _ => library == workspace,
    }
}

/// Reads `RK_STDLIB_PATH` from `<workspace>/.env`, if present.
///
/// Deliberately minimal: `KEY=VALUE` lines, `#` comments, optional single or
/// double quotes around the value. Only this one variable is looked up.
fn read_dotenv_var(workspace: &Path) -> Option<String> {
    let text = std::fs::read_to_string(workspace.join(".env")).ok()?;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=')
            && key.trim() == STDLIB_PATH_ENV
        {
            let value = value.trim();
            let value = value
                .strip_prefix('"')
                .and_then(|v| v.strip_suffix('"'))
                .or_else(|| value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
                .unwrap_or(value);
            return Some(value.to_string());
        }
    }
    None
}

/// Resolves the workspace config file (`config.toml` at the workspace root).
pub fn resolve_config_file(workspace: &Path) -> Option<PathBuf> {
    let path = workspace.join(crate::sysroot::CONFIG_FILE);
    if path.exists() {
        std::fs::canonicalize(path).ok()
    } else {
        None
    }
}

// --- File discovery ---

/// Recursively discovers all `.st` files under `path`.
pub fn find_st_files(path: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if !path.exists() {
        return files;
    }
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let entry_path = entry.path();
            if entry_path.is_file() && entry_path.extension().map(|e| e == "st").unwrap_or(false) {
                files.push(entry_path);
            } else if entry_path.is_dir() {
                files.extend(find_st_files(&entry_path));
            }
        }
    }
    files
}

// --- Parsing ---

/// Reads and parses a single `.st` file, returning the URL and parsed Document.
fn read_and_parse(path: &Path, parsers: &'static auto_lsp::core::parsers::Parser) -> ParseResult {
    let content = std::fs::read_to_string(path)?;
    let absolute_path = std::fs::canonicalize(path)?;
    let url = Url::from_file_path(&absolute_path)
        .map_err(|_| "Failed to convert path to URL".to_string())?;
    let tree = parsers
        .parser
        .write()
        .parse(content.as_bytes(), None)
        .ok_or_else(|| "tree-sitter parse failed".to_string())?;
    let document = Document::new(content, tree, None);
    Ok((url, Arc::new(document)))
}

// --- Loading ---

/// Loads a single workspace file into the database (LOW durability).
pub fn load_file(db: &mut RootDatabase, path: &Path) -> Result<File, Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(path)?;
    let absolute_path = std::fs::canonicalize(path)?;
    let url = Url::from_file_path(&absolute_path)
        .map_err(|_| "Failed to convert path to URL".to_string())?;

    let file = File::from_string()
        .db(db)
        .parsers(&ast::RK_PARSER)
        .url(&url)
        .source(content)
        .call()?;

    db.add_file(file)?;
    Ok(file)
}

/// Discovers and loads all `.st` files under `path`: I/O and parsing in
/// parallel, Salsa inputs created sequentially.
pub fn load_workspace(db: &mut RootDatabase, path: &Path) -> Vec<Result<File, String>> {
    let paths = find_st_files(path);
    let parsers = &*ast::RK_PARSER;

    let parsed: Vec<_> = paths
        .into_par_iter()
        .map(|p| read_and_parse(&p, parsers))
        .collect();

    parsed
        .into_iter()
        .map(|result| match result {
            Ok((url, document)) => {
                let file = File::builder(url.clone(), parsers, document, None).new(db);
                db.workspace_files.insert(url, file);
                Ok(file)
            }
            Err(e) => Err(e.to_string()),
        })
        .collect()
}

/// Loads all library `.st` files with HIGH durability: libraries rarely
/// change, so salsa can skip re-validating queries that only touched them.
pub fn load_libraries(db: &mut RootDatabase) {
    let Some(config) = Workspace::try_get(db) else {
        return;
    };
    let Some(library_path) = config.library_path(db) else {
        return;
    };

    let paths = find_st_files(library_path);
    let parsers = &*ast::RK_PARSER;

    let parsed: Vec<_> = paths
        .into_par_iter()
        .map(|p| read_and_parse(&p, parsers))
        .collect();

    for result in parsed {
        match result {
            Ok((url, document)) => {
                let file = File::builder(url.clone(), parsers, document, None)
                    .durability(Durability::HIGH)
                    .new(db);
                db.library_files.insert(url, file);
            }
            Err(e) => eprintln!("failed to load library file: {}", e),
        }
    }
}
