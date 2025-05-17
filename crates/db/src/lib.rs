use std::sync::{Arc, Mutex};

use auto_lsp::{core::salsa::db::{BaseDatabase, File}, lsp_types::Url, salsa};
use dashmap::DashMap;

#[salsa::db]
#[derive(Default, Clone)]
pub struct RootDatabase {
    storage: salsa::Storage<Self>,
    pub(crate) files: DashMap<Url, File>,
    #[cfg(debug_assertions)]
    logs: Arc<Mutex<Vec<String>>>,
}

#[salsa::db]
impl salsa::Database for RootDatabase {
    fn salsa_event(&self, _event: &dyn Fn() -> salsa::Event) {
        #[cfg(debug_assertions)]
        {
            let event = _event();
            if let salsa::EventKind::WillExecute { .. } = event.kind {
                self.logs.lock().unwrap().push(format!("{event:?}"));
            }
        }
    }
}

impl std::panic::RefUnwindSafe for RootDatabase {}

#[salsa::db]
impl BaseDatabase for RootDatabase {
    fn get_files(&self) -> &DashMap<Url, File> {
        &self.files
    }

    fn get_file(&self,url: &Url) -> Option<File> {
        self.files.get(url).map(|file| *file)
    }

    #[cfg(debug_assertions)]
    fn take_logs(&self) -> Vec<String> {
        std::mem::take(&mut self.logs.lock().unwrap())
    }
}