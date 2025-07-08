#![recursion_limit = "256"]

pub mod hir;
pub mod parser;
pub mod diagnostics;
pub mod ident;
pub mod to_proto;
pub mod solver; 
 
use auto_lsp::{default::db::{BaseDatabase, file::File}, lsp_types::Url, salsa};
use dashmap::DashMap;
use salsa::Event;
pub use ast::RK_PARSER;

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

    fn get_file(&self,url: &Url) -> Option<File> {
        self.files.get(url).map(|file| *file)
    }
}