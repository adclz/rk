//! One deterministic order over the workspace's files. The database stores
//! files in `DashMap`s, whose iteration order changes per process, and
//! that order reaches the wasm function index space. Anything that feeds
//! compilation or per-file output takes its files from here.

use auto_lsp::default::db::{BaseDatabase, file::File};
use db::{RootDatabase, WorkspaceDataBase};

/// The LIBRARY files in URL order, the ones the editor surface stays
/// silent about.
pub fn ordered_library_files(db: &RootDatabase) -> Vec<File> {
    let mut files: Vec<File> = db
        .get_library_files()
        .iter()
        .map(|entry| *entry.value())
        .collect();
    sort_by_url(db, &mut files);
    files
}

pub fn ordered_files(db: &RootDatabase) -> Vec<File> {
    let mut files: Vec<File> = db.get_files().iter().map(|entry| *entry.value()).collect();
    sort_by_url(db, &mut files);
    files
}

/// The workspace's files followed by the libraries', each group in URL
/// order, so adding a workspace file cannot renumber the libraries'
/// functions.
pub fn ordered_files_with_libraries(db: &RootDatabase) -> Vec<File> {
    let mut files = ordered_files(db);
    let mut std_files: Vec<File> = db
        .get_library_files()
        .iter()
        .map(|entry| *entry.value())
        .collect();
    sort_by_url(db, &mut std_files);
    files.extend(std_files);
    files
}

fn sort_by_url(db: &RootDatabase, files: &mut [File]) {
    files.sort_by(|a, b| a.url(db).as_str().cmp(b.url(db).as_str()));
}
