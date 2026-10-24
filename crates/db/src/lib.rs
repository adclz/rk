// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use std::collections::BTreeMap;

use auto_lsp::{core::errors::DataBaseError, default::db::file::File, lsp_types::Url};
use rustc_hash::FxHashMap;
use salsa::{Database, Durability, Event, Setter};

pub mod cache;
pub mod config_file;
pub mod dependencies;
pub mod loader;
pub mod stdlib;
pub mod syntax;
pub mod workspace;

/// The database: salsa's storage, which holds the files with everything
/// else. A clone is a handle on the same storage.
#[salsa::db]
#[derive(Clone)]
pub struct RootDatabase {
    storage: salsa::Storage<Self>,
}

/// The files of the database: the workspace's and the libraries', each in
/// the order of their URLs, and the library each library file is of. The
/// only list of them, and a salsa input: a query that goes through the
/// files, or asks which library one is of, reads them here, and adding or
/// removing a file is a write, which starts a revision. auto-lsp's file
/// manager is not used: it keeps its files in a map salsa does not see.
#[salsa::input(singleton, debug)]
pub struct FileSet {
    #[returns(ref)]
    pub workspace: Vec<File>,
    #[returns(ref)]
    pub libraries: Vec<File>,
    /// The library each library file is of.
    #[returns(ref)]
    pub library_ids: FxHashMap<File, dependencies::LibraryId>,
}

/// Where the file at `url` is in `files`, which are in the order of their
/// URLs, or where it would go.
fn position(db: &dyn Database, files: &[File], url: &Url) -> Result<usize, usize> {
    files.binary_search_by(|file| file.url(db).cmp(url))
}

/// `files` with each of `new` in place of the one at its URL, in the order
/// of the URLs.
fn merged(db: &dyn Database, files: &[File], new: impl IntoIterator<Item = File>) -> Vec<File> {
    let mut by_url: BTreeMap<Url, File> = files
        .iter()
        .map(|file| (file.url(db).clone(), *file))
        .collect();
    for file in new {
        by_url.insert(file.url(db).clone(), file);
    }
    by_url.into_values().collect()
}

/// The library `file` is of, and `None` for a file of the workspace. A query
/// of its own, so that a change to the set of files reaches only the files
/// whose answer changed, not every query that asks.
#[salsa::tracked]
fn library_of_file(db: &dyn WorkspaceDataBase, file: File) -> Option<dependencies::LibraryId> {
    FileSet::get(db).library_ids(db).get(&file).copied()
}

impl Default for RootDatabase {
    fn default() -> Self {
        Self::new(None)
    }
}

impl RootDatabase {
    pub fn new(logs: Option<Box<dyn Fn(Event) + Send + Sync>>) -> Self {
        let db = Self {
            storage: salsa::Storage::new(logs),
        };
        // Made with the database: a query that read the set before it
        // existed would have recorded no dependency on it.
        FileSet::new(&db, Vec::new(), Vec::new(), FxHashMap::default());
        db
    }

    /// Adds `file` to the workspace. Refused when the workspace holds a file
    /// at its URL.
    pub fn add_file(&mut self, file: File) -> Result<(), DataBaseError> {
        let set = FileSet::get(self);
        let url = file.url(self).clone();
        let mut files = set.workspace(self).clone();
        match position(self, &files, &url) {
            Ok(_) => Err(DataBaseError::FileAlreadyExists { uri: url }),
            Err(at) => {
                files.insert(at, file);
                set.set_workspace(self).to(files);
                Ok(())
            }
        }
    }

    /// Removes the workspace's file at `url`.
    pub fn remove_file(&mut self, url: &Url) -> Result<(), DataBaseError> {
        let set = FileSet::get(self);
        let mut files = set.workspace(self).clone();
        match position(self, &files, url) {
            Ok(at) => {
                files.remove(at);
                set.set_workspace(self).to(files);
                Ok(())
            }
            Err(_) => Err(DataBaseError::FileNotFound { uri: url.clone() }),
        }
    }

    /// Makes `files` the workspace's, in place of the ones it had: what the
    /// loader found.
    pub(crate) fn set_workspace_files(&mut self, files: Vec<File>) {
        let set = FileSet::get(self);
        let files = merged(self, &[], files);
        if *set.workspace(self) != files {
            set.set_workspace(self).to(files);
        }
    }

    /// Makes `files` the libraries', each of the library it is paired with,
    /// in place of the ones they had: what the loader found.
    pub(crate) fn set_library_files(&mut self, files: Vec<(File, dependencies::LibraryId)>) {
        let set = FileSet::get(self);
        let library_ids: FxHashMap<File, dependencies::LibraryId> = files.iter().copied().collect();
        let libraries = merged(self, &[], files.into_iter().map(|(file, _)| file));
        // Libraries rarely change, as their files' durability says.
        if *set.libraries(self) != libraries {
            set.set_libraries(self)
                .with_durability(Durability::HIGH)
                .to(libraries);
        }
        if *set.library_ids(self) != library_ids {
            set.set_library_ids(self)
                .with_durability(Durability::HIGH)
                .to(library_ids);
        }
    }

    /// Register `file` as a LIBRARY file: analyzed like workspace code,
    /// without diagnostics. It is the first library's, which is the
    /// standard one when there is one.
    pub fn insert_library_file(&mut self, file: File) {
        self.insert_file_of_library(file, 0);
    }

    /// Register `file` as a file of the library `library`.
    pub fn insert_file_of_library(&mut self, file: File, library: dependencies::LibraryId) {
        self.insert_library_files(vec![(file, library)]);
    }

    /// Registers each file as a file of its library, in place of the file
    /// at its URL, in one write.
    pub(crate) fn insert_library_files(&mut self, files: Vec<(File, dependencies::LibraryId)>) {
        let set = FileSet::get(self);
        let mut library_ids = set.library_ids(self).clone();
        for (file, library) in &files {
            if let Some(replaced) = self.get_library_file(file.url(self)) {
                library_ids.remove(&replaced);
            }
            library_ids.insert(*file, *library);
        }
        let libraries = merged(
            self,
            set.libraries(self),
            files.into_iter().map(|(file, _)| file),
        );
        // Libraries rarely change, as their files' durability says.
        set.set_libraries(self)
            .with_durability(Durability::HIGH)
            .to(libraries);
        set.set_library_ids(self)
            .with_durability(Durability::HIGH)
            .to(library_ids);
    }
}

#[salsa::db]
impl salsa::Database for RootDatabase {}

impl std::panic::RefUnwindSafe for RootDatabase {}

#[salsa::db]
pub trait WorkspaceDataBase: Database {
    /// The workspace's files, in the order of their URLs.
    fn get_files(&self) -> &[File];

    /// The workspace's file at `url`.
    fn get_file(&self, url: &Url) -> Option<File>;

    /// Files of loaded libraries: analyzed exactly like workspace code, but
    /// not part of the workspace: no diagnostics, no LSP requests. In the
    /// order of their URLs.
    fn get_library_files(&self) -> &[File];

    /// The library file at `url`.
    fn get_library_file(&self, url: &Url) -> Option<File>;

    /// The library `file` belongs to, and `None` for a file of the
    /// workspace. Two files are on the same side of the library line when
    /// this answers the same for both: `INTERNAL` and `PRIVATE` stop there.
    fn library_of(&self, file: File) -> Option<dependencies::LibraryId>;
}

#[salsa::db]
impl WorkspaceDataBase for RootDatabase {
    fn get_files(&self) -> &[File] {
        FileSet::get(self).workspace(self)
    }

    fn get_file(&self, url: &Url) -> Option<File> {
        let files = self.get_files();
        position(self, files, url).ok().map(|at| files[at])
    }

    fn get_library_files(&self) -> &[File] {
        FileSet::get(self).libraries(self)
    }

    fn get_library_file(&self, url: &Url) -> Option<File> {
        let files = self.get_library_files();
        position(self, files, url).ok().map(|at| files[at])
    }

    fn library_of(&self, file: File) -> Option<dependencies::LibraryId> {
        library_of_file(self, file)
    }
}
