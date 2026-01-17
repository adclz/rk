use std::path::PathBuf;

use auto_lsp::lsp_types::Url;
use salsa::{Durability, Setter};

use crate::WorkspaceDataBase;

#[salsa::input(singleton, debug)]
pub struct Configuration {
    #[returns(as_ref)]
    pub workspace_folder: Option<PathBuf>,
}

impl Configuration {
    pub fn init_or_update(db: &mut dyn WorkspaceDataBase, workspace_uri: Option<Url>) -> Self {
        match Self::try_get(db) {
            Some(config) => {
                config.update(db, workspace_uri);
                config
            }
            None => Self::from_uri(db, workspace_uri),
        }
    }

    fn from_uri(db: &dyn WorkspaceDataBase, uri: Option<Url>) -> Self {
        let uri = match uri {
            Some(uri) => uri.to_file_path().ok(),
            None => None,
        };

        Self::builder(uri).durability(Durability::HIGH).new(db)
    }

    fn update(&self, db: &mut dyn WorkspaceDataBase, uri: Option<Url>) {
        let uri = match uri {
            Some(uri) => uri.to_file_path().ok(),
            None => None,
        };
        self.set_workspace_folder(db).to(uri);
    }
}
