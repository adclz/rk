use auto_lsp::{
    default::db::{BaseDatabase, file::File},
    lsp_types::Url,
};
use dashmap::DashMap;
use salsa::{Database, Event};


#[salsa::db]
#[derive(Default, Clone)]
pub struct RootDatabase {
    storage: salsa::Storage<Self>,
    pub(crate) files: DashMap<Url, File>,
    pub(crate) workspace_folder: Option<Url>,
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
pub trait WorkspaceDataBase: Database + BaseDatabase {
    fn set_workspace_uri(&mut self, uri: Url);
    fn get_workspace_uri(&self) -> Option<&Url>;
}

#[salsa::db]
impl WorkspaceDataBase for RootDatabase {
    fn set_workspace_uri(&mut self, uri: Url) {
        self.workspace_folder = Some(uri);
    }

    fn get_workspace_uri(&self) -> Option<&Url> {
        self.workspace_folder.as_ref()
    }
}

