use auto_lsp::{
    default::db::{BaseDatabase, file::File},
    lsp_types::Url,
};
use dashmap::DashMap;
use salsa::{Database, Event};

pub mod config_file;
pub mod loader;
pub mod sysroot;
pub mod workspace;

#[salsa::db]
#[derive(Default, Clone)]
pub struct RootDatabase {
    storage: salsa::Storage<Self>,
    pub(crate) workspace_files: DashMap<Url, File>,
    pub(crate) library_files: DashMap<Url, File>,
}

impl RootDatabase {
    /// Register `file` as a LIBRARY file: analyzed like workspace code,
    /// without diagnostics.
    pub fn insert_library_file(&mut self, url: Url, file: File) {
        self.library_files.insert(url, file);
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
}

#[salsa::db]
impl WorkspaceDataBase for RootDatabase {
    fn get_library_files(&self) -> &DashMap<Url, File> {
        &self.library_files
    }
}
