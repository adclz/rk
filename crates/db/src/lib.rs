use auto_lsp::{
    default::db::{BaseDatabase, file::File},
    lsp_types::Url,
};
use dashmap::DashMap;
use salsa::{Database, Event};

pub mod workspace;
pub mod loader;
pub mod config_file;

#[salsa::db]
#[derive(Default, Clone)]
pub struct RootDatabase {
    storage: salsa::Storage<Self>,
    pub(crate) workspace_files: DashMap<Url, File>,
    pub(crate) std_lib_files: DashMap<Url, File>
}

impl RootDatabase {
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
    fn get_std_lib_files(&self) -> &DashMap<Url, File>;
}

#[salsa::db]
impl WorkspaceDataBase for RootDatabase {
    fn get_std_lib_files(&self) -> &DashMap<Url, File> {
        &self.std_lib_files
    }
}

