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

/// Resolves the stdlib path.
///
/// In debug builds, looks for `stdlib/` relative to CWD (assumes repo root).
/// In release builds, extracts embedded stdlib to `$HOME/.rk_std/` and returns that path.
pub fn resolve_stdlib_path() -> Option<PathBuf> {
    if cfg!(debug_assertions) {
        let path = PathBuf::from("stdlib");
        if path.exists() {
            return std::fs::canonicalize(path).ok();
        }
    }

    crate::embedded_stdlib::ensure_stdlib_extracted()
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
fn read_and_parse(path: &Path, parsers: &'static auto_lsp::core::parsers::Parsers) -> ParseResult {
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
        .parsers(
            ast::RK_PARSER
                .get("structured_text")
                .ok_or("Parser not found")?,
        )
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
    let parsers = match ast::RK_PARSER.get("structured_text") {
        Some(p) => p,
        None => return vec![],
    };

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

/// Loads all stdlib `.st` files into the database with HIGH durability.
///
/// Reads the stdlib path from `Configuration`, discovers files recursively,
/// and inserts them into `std_lib_files` (not workspace files).
pub fn load_stdlib(db: &mut RootDatabase) {
    let Some(config) = Workspace::try_get(db) else {
        return;
    };
    let Some(stdlib_path) = config.stdlib_path(db) else {
        return;
    };

    let paths = find_st_files(stdlib_path);
    let parsers = match ast::RK_PARSER.get("structured_text") {
        Some(p) => p,
        None => return,
    };

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
                db.std_lib_files.insert(url, file);
            }
            Err(e) => eprintln!("failed to load stdlib file: {}", e),
        }
    }
}
