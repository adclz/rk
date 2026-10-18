// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use auto_lsp::{
    default::db::{BaseDatabase, file::File},
    lsp_types::Url,
};
use dashmap::DashMap;
use salsa::{Database, Event};

pub mod cache;
pub mod config_file;
pub mod dependencies;
pub mod loader;
pub mod stdlib;
pub mod syntax;
pub mod workspace;

#[salsa::db]
#[derive(Default, Clone)]
pub struct RootDatabase {
    storage: salsa::Storage<Self>,
    pub(crate) workspace_files: DashMap<Url, File>,
    pub(crate) library_files: DashMap<Url, File>,
    /// Which library each library file belongs to.
    pub(crate) file_libraries: DashMap<File, dependencies::LibraryId>,
}

impl RootDatabase {
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
    }

    pub fn new(logs: Option<Box<dyn Fn(Event) + Send + Sync>>) -> Self {
        Self {
            storage: salsa::Storage::new(logs),
            ..Default::default()
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
        self.file_libraries.get(&file).map(|library| *library)
    }
}
