// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! The standard library, built into the compiler.
//!
//! Its files are part of the executable. Before a workspace reads them,
//! they are written in the cache under their hash, where the editor opens
//! them like any other file. That copy is hashed again at each start, and
//! written again when it differs from the one built in.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use crate::cache::{self, Entry, StoreError};
use crate::config_file::{CONFIG_FILE, parse_config};
use crate::dependencies::{ContentHash, Library, cached_dir};

/// Environment variable naming a folder to read as the standard library,
/// in place of the one built in, in the environment or in the `.env` at
/// the workspace's root. Empty, no standard library is read.
pub const PATH_ENV: &str = "RK_STDLIB_PATH";

/// The files of `stdlib/`, each by its path in the library: its sources,
/// its `config.toml` and its `LICENSE`. The build script lists them.
const FILES: &[(&str, &[u8])] = include!(concat!(env!("OUT_DIR"), "/stdlib_files.rs"));

/// The standard library the compiler is built with.
pub struct Bundle {
    /// The `name` of its `config.toml`.
    pub name: String,
    pub entries: BTreeMap<String, Entry>,
    pub hash: ContentHash,
}

/// The standard library the compiler is built with.
pub fn bundle() -> &'static Bundle {
    static BUNDLE: LazyLock<Bundle> = LazyLock::new(|| {
        let entries: BTreeMap<String, Entry> = FILES
            .iter()
            .map(|(path, content)| (path.to_string(), Entry::File(content.to_vec())))
            .collect();
        let config = FILES
            .iter()
            .find(|(path, _)| *path == CONFIG_FILE)
            .and_then(|(_, source)| std::str::from_utf8(source).ok())
            .and_then(|source| parse_config(source).ok())
            .expect("stdlib/ has a valid config.toml");
        Bundle {
            name: config.project.name,
            hash: ContentHash::of(&entries),
            entries,
        }
    });
    &BUNDLE
}

/// Writes the standard library in `cache`, unless the copy there is the
/// same already, and returns its folder.
///
/// The copy is hashed each time. A file changed or added since it was
/// written makes it differ from the one built in, and it is written again.
pub fn install(cache: &Path) -> Result<PathBuf, StoreError> {
    let bundle = bundle();
    let dir = cached_dir(cache, &bundle.hash);
    if dir.is_dir() {
        let intact =
            cache::read(&dir).is_ok_and(|entries| ContentHash::of(&entries) == bundle.hash);
        if intact {
            return Ok(dir);
        }
        cache::remove(&dir).map_err(|error| StoreError::Write {
            dir: dir.clone(),
            error,
        })?;
    }
    cache::store(cache, &bundle.hash, &bundle.entries)
}

/// Where the standard library comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    /// Built into the compiler.
    Bundled,
    /// The folder `RK_STDLIB_PATH` names in the environment.
    Env,
    /// The folder `RK_STDLIB_PATH` names in the workspace's `.env`.
    Dotenv,
}

impl Origin {
    /// The spelling `rk env` uses.
    pub fn as_str(self) -> &'static str {
        match self {
            Origin::Bundled => "bundled",
            Origin::Env => "env",
            Origin::Dotenv => "dotenv",
        }
    }
}

/// The standard library a workspace reads, or why it reads none.
#[derive(Debug)]
pub enum Standard {
    Found {
        library: Library,
        origin: Origin,
    },
    /// `RK_STDLIB_PATH` is empty: the workspace reads none. The standard
    /// library's own workspace says so in its `.env`.
    Disabled,
    /// `RK_STDLIB_PATH` names something that is not a folder.
    NotAFolder(String),
    /// The folder `RK_STDLIB_PATH` names has a name no namespace can have.
    NotANamespaceName {
        dir: PathBuf,
        name: String,
    },
    /// The one built in could not be written in the cache.
    NotInstalled(String),
}

/// The standard library of the workspace at `workspace`: the folder
/// `RK_STDLIB_PATH` names, in the environment or else in the `.env` at the
/// workspace's root, or else the one built in, written in the cache.
pub fn locate(workspace: Option<&Path>) -> Standard {
    let named = std::env::var(PATH_ENV)
        .ok()
        .map(|value| (value, Origin::Env))
        .or_else(|| {
            workspace
                .and_then(dotenv_value)
                .map(|value| (value, Origin::Dotenv))
        });
    match named {
        Some((value, _)) if value.is_empty() => Standard::Disabled,
        Some((value, origin)) => match std::fs::canonicalize(&value) {
            Ok(dir) if dir.is_dir() => {
                let library = Library::standard(dir);
                if !crate::dependencies::is_namespace_name(&library.name) {
                    return Standard::NotANamespaceName {
                        dir: library.dir,
                        name: library.name,
                    };
                }
                Standard::Found { library, origin }
            }
            _ => Standard::NotAFolder(value),
        },
        None => {
            let Some(cache) = cache::root() else {
                return Standard::NotInstalled(format!(
                    "there is no cache directory: set {}",
                    cache::CACHE_DIR_ENV
                ));
            };
            let dir = match install(&cache) {
                Ok(dir) => dir,
                Err(error) => return Standard::NotInstalled(error.to_string()),
            };
            Standard::Found {
                library: Library {
                    name: bundle().name.clone(),
                    dir: dir.canonicalize().unwrap_or(dir),
                    standard: true,
                },
                origin: Origin::Bundled,
            }
        }
    }
}

/// The `RK_STDLIB_PATH` of `<workspace>/.env`: `KEY=VALUE` lines, `#`
/// comments, optional quotes. It is the only variable read from that file.
fn dotenv_value(workspace: &Path) -> Option<String> {
    let text = std::fs::read_to_string(workspace.join(".env")).ok()?;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=')
            && key.trim() == PATH_ENV
        {
            let value = value.trim();
            let value = value
                .strip_prefix('"')
                .and_then(|v| v.strip_suffix('"'))
                .or_else(|| value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
                .unwrap_or(value);
            return Some(value.to_string());
        }
    }
    None
}
