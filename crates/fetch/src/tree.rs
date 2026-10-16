// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! What an archive or a commit unpacks to, which of it makes the package,
//! and the hash of that.

use std::collections::BTreeMap;

use db::dependencies::{ContentHash, FetchError};

pub use db::cache::{Entry, FileHeader};

/// One entry that could not be unpacked. It only counts when the package
/// includes that entry ([`Tree::validate`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UnpackError {
    CreateFile {
        name: String,
        code: String,
    },
    CreateSymlink {
        name: String,
        link: String,
        code: String,
    },
    UnsupportedType {
        name: String,
        kind: char,
    },
}

impl UnpackError {
    fn name(&self) -> &str {
        match self {
            Self::CreateFile { name, .. }
            | Self::CreateSymlink { name, .. }
            | Self::UnsupportedType { name, .. } => name,
        }
    }

    fn note(&self) -> String {
        match self {
            Self::CreateFile { name, code } => format!("unable to create file '{name}': {code}"),
            Self::CreateSymlink { name, link, code } => {
                format!("unable to create symlink from '{name}' to '{link}': {code}")
            }
            Self::UnsupportedType { name, kind } => {
                format!("file '{name}' has unsupported type '{kind}'")
            }
        }
    }
}

/// The files and folders a package says it is made of. With none, it is
/// made of everything.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Filter {
    include_paths: Vec<String>,
}

impl Filter {
    pub fn new(include_paths: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            include_paths: include_paths.into_iter().map(Into::into).collect(),
        }
    }

    /// Whether the package includes `sub_path`, which is relative to its
    /// root: it is listed, or a folder that holds it is.
    pub fn includes(&self, sub_path: &str) -> bool {
        let listed = |path: &str| self.include_paths.iter().any(|included| included == path);
        if self.include_paths.is_empty() || listed("") || listed(".") || listed(sub_path) {
            return true;
        }
        let mut dirname = sub_path;
        while let Some((parent, _)) = dirname.rsplit_once('/') {
            if listed(parent) {
                return true;
            }
            dirname = parent;
        }
        false
    }
}

/// What was unpacked: every entry by its path, with `/` between the parts
/// and without the folder that held them all, and what could not be.
#[derive(Clone, Debug, Default)]
pub struct Tree {
    pub entries: BTreeMap<String, Entry>,
    pub errors: Vec<UnpackError>,
    /// What the errors are reported under.
    pub root_error_message: &'static str,
}

impl Tree {
    /// Adds an entry. A path that is there already cannot be created a
    /// second time, and neither can one that differs from it only by case:
    /// it is the same file on a file system that does not tell them apart.
    pub fn insert(&mut self, path: String, entry: Entry) {
        let taken = self
            .entries
            .keys()
            .any(|existing| existing.eq_ignore_ascii_case(&path));
        if !taken {
            self.entries.insert(path, entry);
            return;
        }
        let code = "PathAlreadyExists".to_string();
        self.errors.push(match entry {
            Entry::File(_) => UnpackError::CreateFile { name: path, code },
            Entry::Link(link) => UnpackError::CreateSymlink {
                name: path,
                link,
                code,
            },
        });
    }

    /// Fails with the errors about entries the package includes. An error
    /// about an entry it leaves out is ignored: a link that could not be
    /// made does not matter when the link is not part of the package.
    pub fn validate(&self, filter: &Filter) -> Result<(), FetchError> {
        let notes: Vec<String> = self
            .errors
            .iter()
            .filter(|error| filter.includes(error.name()))
            .map(UnpackError::note)
            .collect();
        if notes.is_empty() {
            return Ok(());
        }
        Err(FetchError {
            message: self.root_error_message.to_string(),
            notes,
        })
    }

    /// Leaves out what the package does not include.
    pub fn retain(&mut self, filter: &Filter) {
        self.entries.retain(|path, _| filter.includes(path));
    }

    /// The hash of the entries ([`ContentHash::of`]).
    pub fn digest(&self) -> ContentHash {
        ContentHash::of(&self.entries)
    }
}
