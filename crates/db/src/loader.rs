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
use crate::workspace::Workspace;

type ParseResult = Result<(Url, Arc<Document>), Box<dyn std::error::Error + Send + Sync>>;

// --- Path resolution ---

/// Environment variable naming the standard library's directory.
///
/// The compiler has no notion of a standard library — only of a library
/// directory, loaded alongside the workspace and analyzed like any other
/// code. This variable is the only way a library is acquired; it is read
/// from the process environment first, then from a `.env` file at the
/// workspace root (process wins, the usual dotenv convention).
pub const STDLIB_PATH_ENV: &str = "RK_STDLIB_PATH";

/// What the library variable said.
pub enum LibraryPathResolution {
    /// Not set anywhere: no library loads, and it is worth telling the user
    /// why `Std.*` names will not resolve.
    Unset,
    /// Set to the empty string: the explicit, silent "no library" — how the
    /// standard library's own workspace avoids loading a second copy of
    /// itself (see `stdlib/.env`).
    Disabled,
    /// Set to a directory that exists.
    Found(PathBuf),
    /// Set to something that is not a readable directory. Never silently
    /// ignored: compiling against a library the user did not choose would be
    /// worse than compiling against none.
    Invalid(String),
}

/// Resolves the library directory from `RK_STDLIB_PATH`.
pub fn resolve_library_path(workspace: Option<&Path>) -> LibraryPathResolution {
    let value = std::env::var(STDLIB_PATH_ENV)
        .ok()
        .or_else(|| workspace.and_then(read_dotenv_var));
    match value {
        None => LibraryPathResolution::Unset,
        Some(v) if v.is_empty() => LibraryPathResolution::Disabled,
        Some(v) => match std::fs::canonicalize(&v) {
            Ok(dir) if dir.is_dir() => LibraryPathResolution::Found(dir),
            _ => LibraryPathResolution::Invalid(v),
        },
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
    let path = workspace.join("config.toml");
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
