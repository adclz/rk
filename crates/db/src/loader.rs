// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use std::path::{Path, PathBuf};
use std::sync::Arc;

use auto_lsp::{
    core::{document::Document, parsers::Parser},
    default::db::file::File,
    lsp_types::Url,
    salsa::Durability,
};
use rayon::prelude::*;

use crate::RootDatabase;
use crate::WorkspaceDataBase;
use crate::config_file::CONFIG_FILE;
use crate::workspace::Workspace;

type ParseResult = Result<(Url, Arc<Document>), Box<dyn std::error::Error + Send + Sync>>;

// --- Path resolution ---

/// Resolves the workspace config file (`config.toml` at the workspace root).
pub fn resolve_config_file(workspace: &Path) -> Option<PathBuf> {
    let path = workspace.join(CONFIG_FILE);
    if path.exists() {
        std::fs::canonicalize(path).ok()
    } else {
        None
    }
}

// --- File discovery ---

/// Whether `extension` is the one of rk's own files: `st`, in any case
/// (`MAIN.ST` is the usual Windows spelling).
pub fn is_rk_extension(extension: &std::ffi::OsStr) -> bool {
    extension.eq_ignore_ascii_case("st")
}

/// Whether `path` names a Structured Text source, by its extension. What
/// takes rk's own files only asks this: the formatter, the editor.
pub fn is_st_file(path: &Path) -> bool {
    path.extension().is_some_and(is_rk_extension)
}

/// Whether `parser` is rk's own: the file it read is one of rk's.
pub fn is_rk_parser(parser: &Parser) -> bool {
    std::ptr::eq(parser, &*ast::RK_PARSER)
}

/// rk's parser for a `.st` file, and none for another: the [`ParserOf`]
/// of rk alone.
pub fn rk_parser(path: &Path) -> Option<&'static Parser> {
    is_st_file(path).then(|| &*ast::RK_PARSER)
}

/// The parser that reads the file at a path, and none for a file that is
/// no source. The loader does not decide it: whoever opens a workspace
/// says which files it reads, [`rk_parser`] for rk's own alone.
pub type ParserOf = fn(&Path) -> Option<&'static Parser>;

/// Recursively discovers the source files under `path`, each with the
/// parser that reads it. A folder with a `config.toml` of its own is
/// another workspace or library, and its files are left out.
pub fn find_files(path: &Path, parser_of: ParserOf) -> Vec<(PathBuf, &'static Parser)> {
    find_files_except(path, parser_of, &[])
}

/// [`find_files`], leaving out the folders of `except` too, which are
/// canonical: another library's, when one is inside this one.
fn find_files_except(
    path: &Path,
    parser_of: ParserOf,
    except: &[PathBuf],
) -> Vec<(PathBuf, &'static Parser)> {
    let mut files = Vec::new();
    if !path.exists() {
        return files;
    }
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let entry_path = entry.path();
            if entry_path.is_dir() {
                let other = entry_path.join(CONFIG_FILE).is_file()
                    || (!except.is_empty()
                        && entry_path
                            .canonicalize()
                            .is_ok_and(|dir| except.contains(&dir)));
                if !other {
                    files.extend(find_files_except(&entry_path, parser_of, except));
                }
            } else if entry_path.is_file()
                && let Some(parser) = parser_of(&entry_path)
            {
                files.push((entry_path, parser));
            }
        }
    }
    files
}

// --- Parsing ---

/// Reads a source file and parses it with its parser, returning the URL
/// and parsed Document.
fn read_and_parse(path: &Path, parsers: &'static Parser) -> ParseResult {
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

/// Opens `workspace` in `db`: the configuration, the library that
/// configuration resolves to, and then the workspace's own source files,
/// the ones `parser_of` names. Returns one result per file found, as
/// [`load_workspace`] does.
///
/// The ORDER is the contract, and the reason this is one function instead of
/// three calls each caller writes out. The configuration decides which
/// libraries to read, and they are read before the workspace so that a
/// workspace CONTAINING one loads nothing twice. The CLI and the language
/// server each wrote the sequence by hand once, and nothing held them
/// together: opening the rk checkout in the editor reported all 801 of its
/// POUs as duplicates of themselves while `rk check` on the same folder was
/// clean.
///
/// `fetch` puts in the cache a library the configuration names and the
/// cache lacks. With none, such a library is reported: the editor reads,
/// and `rk` fetches.
pub fn open_workspace(
    db: &mut RootDatabase,
    workspace: Option<&Path>,
    encoding: auto_lsp::lsp_types::PositionEncodingKind,
    parser_of: ParserOf,
    fetch: Option<crate::dependencies::Fetch<'_>>,
    file_errors: &mut Vec<ide_diagnostic::IdeDiagnostic>,
    notices: &mut Vec<crate::workspace::ConfigurationNotice>,
) -> Vec<Result<File, String>> {
    let workspace_uri = workspace
        .and_then(|path| std::fs::canonicalize(path).ok())
        .and_then(|path| Url::from_file_path(path).ok());
    Workspace::init_or_update(db, workspace_uri, encoding, fetch, file_errors, notices);
    load_libraries(db, parser_of);
    match workspace {
        Some(path) => load_workspace(db, path, parser_of),
        None => Vec::new(),
    }
}

/// Whether the library already holds this path, compared the way both
/// loaders build their keys: canonical, then as a URL.
fn is_library_file(db: &RootDatabase, path: &Path) -> bool {
    std::fs::canonicalize(path)
        .ok()
        .and_then(|path| Url::from_file_path(path).ok())
        .is_some_and(|url| db.get_library_file(&url).is_some())
}

/// Whether the file at `path` is the workspace's, as [`load_workspace`]
/// finds them: under the workspace folder, in no folder there with a
/// `config.toml` of its own, and not a library's. With no folder open,
/// every file is but the libraries' and the cache's, which holds libraries
/// only.
///
/// The language server asks it of each document the editor opens, which can
/// be anywhere. A copy of the standard library an earlier build left in the
/// cache, opened again from the editor's history, was read as the
/// workspace's and declared every POU of the library a second time.
pub fn is_workspace_file(db: &RootDatabase, path: &Path) -> bool {
    let Ok(path) = std::fs::canonicalize(path) else {
        return false;
    };
    if is_library_file(db, &path) {
        return false;
    }
    match Workspace::try_get(db).and_then(|workspace| workspace.workspace_folder(db)) {
        Some(folder) => {
            // Canonical, as `path` is: the folder went through a file URL,
            // which gives a Windows path back without its `\\?\`.
            let folder = folder.canonicalize().unwrap_or_else(|_| folder.clone());
            path.strip_prefix(&folder).is_ok_and(|relative| {
                !relative
                    .parent()
                    .into_iter()
                    .flat_map(Path::ancestors)
                    .take_while(|dir| !dir.as_os_str().is_empty())
                    .any(|dir| folder.join(dir).join(CONFIG_FILE).is_file())
            })
        }
        None => !crate::cache::root()
            .and_then(|cache| cache.canonicalize().ok())
            .is_some_and(|cache| path.starts_with(cache)),
    }
}

/// Discovers and loads the source files under `path`: I/O and parsing in
/// parallel, Salsa inputs created sequentially.
pub fn load_workspace(
    db: &mut RootDatabase,
    path: &Path,
    parser_of: ParserOf,
) -> Vec<Result<File, String>> {
    // A workspace folder that CONTAINS the library holds its files twice:
    // once as its own, once as the library's. Every POU then collides with
    // itself and the whole library reads as duplicated (E0102), which is what
    // opening the rk checkout in an editor used to report. The library is
    // loaded first, so the ones it claimed are not this workspace's.
    let paths: Vec<_> = find_files(path, parser_of)
        .into_iter()
        .filter(|(path, _)| !is_library_file(db, path))
        .collect();

    let parsed: Vec<_> = paths
        .into_par_iter()
        .map(|(p, parsers)| (read_and_parse(&p, parsers), p, parsers))
        .collect();

    let files: Vec<Result<File, String>> = parsed
        .into_iter()
        .map(|(result, path, parsers)| match result {
            Ok((url, document)) => Ok(File::builder(url, parsers, document, None).new(db)),
            // The path travels with the error: a workspace of many files
            // cannot otherwise tell which one it could not read.
            Err(e) => Err(format!("{}: {e}", path.display())),
        })
        .collect();
    db.insert_workspace_files(
        files
            .iter()
            .filter_map(|file| file.as_ref().ok().copied())
            .collect(),
    );
    files
}

/// Loads the source files of each library with HIGH durability: libraries
/// rarely change, so salsa can skip re-validating queries that only touched
/// them. A file is its library's alone: a library whose folder holds
/// another one's, or the workspace's, does not read theirs.
pub fn load_libraries(db: &mut RootDatabase, parser_of: ParserOf) {
    let Some(config) = Workspace::try_get(db) else {
        return;
    };
    let libraries = config.libraries(db).clone();
    let canonical = |dir: &Path| dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
    let workspace = config.workspace_folder(db).map(|dir| canonical(dir));

    for (id, library) in libraries.iter().enumerate() {
        let except: Vec<PathBuf> = libraries
            .iter()
            .enumerate()
            .filter(|(other, _)| *other != id)
            .map(|(_, other)| canonical(&other.dir))
            .chain(workspace.clone())
            .collect();
        let parsed: Vec<_> = find_files_except(&library.dir, parser_of, &except)
            .into_par_iter()
            .map(|(p, parsers)| (read_and_parse(&p, parsers), parsers))
            .collect();

        let mut files = Vec::new();
        for (result, parsers) in parsed {
            match result {
                Ok((url, _)) if db.get_library_file(&url).is_some() => {}
                Ok((url, document)) => {
                    let file = File::builder(url, parsers, document, None)
                        .durability(Durability::HIGH)
                        .new(db);
                    files.push((file, id as crate::dependencies::LibraryId));
                }
                Err(e) => eprintln!("failed to load library file: {}", e),
            }
        }
        // One write per library: the next library's files are told from
        // this one's.
        db.insert_library_files(files);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_extension_matches_in_any_case() {
        for name in ["main.st", "MAIN.ST", "Main.St", "dir.d/x.sT"] {
            assert!(is_st_file(Path::new(name)), "{name}");
        }
        for name in ["main.stx", "main.txt", "main", "st", "main.st.bak"] {
            assert!(!is_st_file(Path::new(name)), "{name}");
        }
    }

    /// Discovery matches the extension case-insensitively.
    #[test]
    fn discovery_finds_an_uppercase_extension() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("MAIN.ST"), "FUNCTION f : INT END_FUNCTION").unwrap();
        std::fs::write(dir.path().join("notes.txt"), "not a source").unwrap();
        let found = find_files(dir.path(), rk_parser);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].0.ends_with("MAIN.ST"));
        assert!(is_rk_parser(found[0].1));
    }
}
