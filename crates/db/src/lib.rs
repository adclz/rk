use auto_lsp::{default::db::{BaseDatabase, File}, lsp_types::Url, salsa};
use dashmap::DashMap;

#[salsa::db]
#[derive(Default, Clone)]
pub struct RootDatabase {
    storage: salsa::Storage<Self>,
    pub(crate) files: DashMap<Url, File>,
}

#[salsa::db]
impl salsa::Database for RootDatabase {}

impl std::panic::RefUnwindSafe for RootDatabase {}

#[salsa::db]
impl BaseDatabase for RootDatabase {
    fn get_files(&self) -> &DashMap<Url, File> {
        &self.files
    }

    fn get_file(&self,url: &Url) -> Option<File> {
        self.files.get(url).map(|file| *file)
    }
}