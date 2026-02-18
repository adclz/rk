use crate::{WorkspaceDataBase, configuration::Configuration};

#[salsa::tracked]
pub fn get_config<'db>(db: &'db dyn WorkspaceDataBase) -> Config {
    let config = match Configuration::try_get(db) {
        Some(config) => config,
        None => {
            // Return default config if not initialized yet
            return Config::default();
        }
    };

    Config {

    }
}

#[derive(Default, Clone, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub struct Config {
    
}