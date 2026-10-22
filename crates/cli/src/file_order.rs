// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! One deterministic order over the workspace's files, which reaches the
//! wasm function index space: the database keeps each list in the order of
//! the URLs ([`db::FileSet`]). Anything that feeds compilation or per-file
//! output takes its files from here.

use auto_lsp::default::db::file::File;
use db::{RootDatabase, WorkspaceDataBase};

/// The LIBRARY files in URL order, the ones the editor surface stays
/// silent about.
pub fn ordered_library_files(db: &RootDatabase) -> Vec<File> {
    db.get_library_files().to_vec()
}

pub fn ordered_files(db: &RootDatabase) -> Vec<File> {
    db.get_files().to_vec()
}

/// The workspace's files followed by the libraries', each group in URL
/// order, so adding a workspace file cannot renumber the libraries'
/// functions.
pub fn ordered_files_with_libraries(db: &RootDatabase) -> Vec<File> {
    let mut files = ordered_files(db);
    files.extend(ordered_library_files(db));
    files
}
