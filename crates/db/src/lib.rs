// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use auto_lsp::{
    core::errors::DataBaseError,
    default::db::{BaseDatabase, FileManager, file::File},
    lsp_types::Url,
};
use dashmap::DashMap;
use rustc_hash::FxHashMap;
use salsa::{Database, Durability, Event, Setter};

pub mod cache;
pub mod config_file;
pub mod dependencies;
pub mod loader;
pub mod stdlib;
pub mod syntax;
pub mod workspace;

#[salsa::db]
#[derive(Clone)]
pub struct RootDatabase {
    storage: salsa::Storage<Self>,
    // Each handle has its own copy, taken when it is cloned: a request goes
    // through its files without holding up the handle that adds one. Queries
    // read the files from the [`FileSet`], which the storage shares.
    pub(crate) workspace_files: DashMap<Url, File>,
    pub(crate) library_files: DashMap<Url, File>,
    /// Which library each library file belongs to.
    pub(crate) file_libraries: DashMap<File, dependencies::LibraryId>,
}

/// The files of the database, as salsa sees them: a query that goes through
/// the files, or asks which library one is of, reads them here, and adding
/// or removing a file is a write, which starts a revision. The maps of
/// [`RootDatabase`] stay for finding a file by its URL, outside queries.
/// Each list is in the order of the URLs, so a query that goes through the
/// files meets them in the same order on every run.
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
            workspace_files: DashMap::default(),
            library_files: DashMap::default(),
            file_libraries: DashMap::default(),
        };
        // Made with the database: a query that read the set before it
        // existed would have recorded no dependency on it.
        FileSet::new(&db, Vec::new(), Vec::new(), FxHashMap::default());
        db
    }

    /// Adds `file` to the workspace, as auto-lsp's [`FileManager::add_file`]
    /// does, and to the [`FileSet`]. Inherent, so `db.add_file` is this one.
    pub fn add_file(&mut self, file: File) -> Result<(), DataBaseError> {
        FileManager::add_file(self, file)?;
        self.sync_file_set();
        Ok(())
    }

    /// Removes the file at `url` from the workspace and from the
    /// [`FileSet`].
    pub fn remove_file(&mut self, url: &Url) -> Result<(), DataBaseError> {
        FileManager::remove_file(self, url)?;
        self.sync_file_set();
        Ok(())
    }

    /// Register `file` as a LIBRARY file: analyzed like workspace code,
    /// without diagnostics. It is the first library's, which is the
    /// standard one when there is one.
    pub fn insert_library_file(&mut self, url: Url, file: File) {
        self.insert_file_of_library(url, file, 0);
    }

    /// Register `file` as a file of the library `library`.
    pub fn insert_file_of_library(
        &mut self,
        url: Url,
        file: File,
        library: dependencies::LibraryId,
    ) {
        self.library_files.insert(url, file);
        self.file_libraries.insert(file, library);
        self.sync_file_set();
    }

    /// Writes the files of the maps into the [`FileSet`], where they differ
    /// from what it holds. Every change to the maps ends here: auto-lsp adds
    /// and removes files in the maps alone, and only a write tells salsa. A
    /// write starts a revision even when the value is the same, hence the
    /// comparisons.
    pub fn sync_file_set(&mut self) {
        fn by_url(files: &DashMap<Url, File>) -> Vec<File> {
            let mut files: Vec<(Url, File)> = files
                .iter()
                .map(|entry| (entry.key().clone(), *entry.value()))
                .collect();
            files.sort_by(|a, b| a.0.cmp(&b.0));
            files.into_iter().map(|(_, file)| file).collect()
        }
        let workspace = by_url(&self.workspace_files);
        let libraries = by_url(&self.library_files);
        let library_ids: FxHashMap<File, dependencies::LibraryId> = self
            .file_libraries
            .iter()
            .map(|entry| (*entry.key(), *entry.value()))
            .collect();
        let set = FileSet::get(self);
        if *set.workspace(self) != workspace {
            set.set_workspace(self).to(workspace);
        }
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
}

#[salsa::db]
impl salsa::Database for RootDatabase {}

impl std::panic::RefUnwindSafe for RootDatabase {}

#[salsa::db]
impl BaseDatabase for RootDatabase {
    fn get_files(&self) -> &DashMap<Url, File> {
        &self.workspace_files
    }

    fn get_file(&self, url: &Url) -> Option<File> {
        self.workspace_files.get(url).map(|file| *file)
    }
}

#[salsa::db]
pub trait WorkspaceDataBase: Database + BaseDatabase {
    /// Files of loaded libraries: analyzed exactly like workspace code, but
    /// not part of the workspace: no diagnostics, no LSP requests.
    fn get_library_files(&self) -> &DashMap<Url, File>;

    /// The library `file` belongs to, and `None` for a file of the
    /// workspace. Two files are on the same side of the library line when
    /// this answers the same for both: `INTERNAL` and `PRIVATE` stop there.
    fn library_of(&self, file: File) -> Option<dependencies::LibraryId>;
}

#[salsa::db]
impl WorkspaceDataBase for RootDatabase {
    fn get_library_files(&self) -> &DashMap<Url, File> {
        &self.library_files
    }

    fn library_of(&self, file: File) -> Option<dependencies::LibraryId> {
        library_of_file(self, file)
    }
}
