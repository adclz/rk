use auto_lsp::{
    default::db::{BaseDatabase, file::File},
    lsp_types::Url,
};
use dashmap::DashMap;
use salsa::{Database, Event};

pub mod configuration;

#[salsa::db]
#[derive(Default, Clone)]
pub struct RootDatabase {
    storage: salsa::Storage<Self>,
    pub(crate) files: DashMap<Url, File>,
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
        &self.files
    }

    fn get_file(&self, url: &Url) -> Option<File> {
        self.files.get(url).map(|file| *file)
    }
}

#[salsa::db]
pub trait WorkspaceDataBase: Database + BaseDatabase {}

#[salsa::db]
impl WorkspaceDataBase for RootDatabase {}
