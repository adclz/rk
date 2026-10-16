// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! rk's cache directory: what it keeps between builds. Each library is
//! there under the hash of its files, the standard library with them, and
//! the optimizer beside them.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::dependencies::{ContentHash, cached_dir, packages_dir};

/// Environment variable naming the cache directory, for a machine whose
/// user cache is not the place for it.
pub const CACHE_DIR_ENV: &str = "RK_CACHE_DIR";

/// Environment variable that refuses the downloads rk starts by itself.
pub const NO_DOWNLOAD_ENV: &str = "RK_NO_DOWNLOAD";

/// The cache directory: `RK_CACHE_DIR`, or else `rk` in the user's cache
/// directory. `None` when the platform names no cache directory.
pub fn root() -> Option<PathBuf> {
    match std::env::var_os(CACHE_DIR_ENV) {
        Some(dir) if !dir.is_empty() => Some(PathBuf::from(dir)),
        _ => Some(dirs::cache_dir()?.join("rk")),
    }
}

/// Whether rk may download by itself what it lacks.
pub fn may_download() -> bool {
    std::env::var_os(NO_DOWNLOAD_ENV).is_none()
}

/// A file or a link of a library.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Entry {
    File(Vec<u8>),
    /// A symbolic link, by what it points at.
    Link(String),
}

/// Why the cache could not take a library.
#[derive(Debug)]
pub enum StoreError {
    /// A folder of the cache could not be written.
    Write { dir: PathBuf, error: std::io::Error },
    /// A link could not be made.
    Link {
        path: String,
        link: String,
        error: std::io::Error,
    },
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Write { dir, error } => {
                write!(
                    f,
                    "unable to write in the cache at '{}': {error}",
                    dir.display()
                )
            }
            Self::Link { path, link, error } => {
                write!(
                    f,
                    "unable to create symlink from '{path}' to '{link}': {error}"
                )
            }
        }
    }
}

/// Writes `entries` in the cache under `hash`, and returns their folder.
///
/// They are written beside their place and moved there whole, so a build
/// never reads half a library. A folder the cache has under that hash
/// already is left untouched: a build may be reading it. Each file is made
/// read-only.
pub fn store(
    cache: &Path,
    hash: &ContentHash,
    entries: &BTreeMap<String, Entry>,
) -> Result<PathBuf, StoreError> {
    let dest = cached_dir(cache, hash);
    if dest.is_dir() {
        return Ok(dest);
    }
    let packages = packages_dir(cache);
    let written = |error| StoreError::Write {
        dir: packages.clone(),
        error,
    };
    std::fs::create_dir_all(&packages).map_err(written)?;

    let staging = tempfile::Builder::new()
        .prefix(".writing-")
        .tempdir_in(&packages)
        .map_err(written)?;
    let unpacked = staging.path().join("library");
    let moved = write_entries(&unpacked, entries)
        .map_err(|error| match error {
            Unwritten::Io(error) => written(error),
            Unwritten::Link(error) => error,
        })
        .and_then(|()| std::fs::rename(&unpacked, &dest).map_err(written));
    match moved {
        Ok(()) => Ok(dest),
        // Another build wrote it meanwhile, and theirs is the same.
        Err(_) if dest.is_dir() => {
            make_writable(staging.path());
            Ok(dest)
        }
        // What was written is left to be deleted, which a file system that
        // keeps read-only files refuses until they are writable again.
        Err(error) => {
            make_writable(staging.path());
            Err(error)
        }
    }
}

/// Why [`write_entries`] stopped.
enum Unwritten {
    Io(std::io::Error),
    Link(StoreError),
}

/// Writes each entry under `dir`, read-only.
fn write_entries(dir: &Path, entries: &BTreeMap<String, Entry>) -> Result<(), Unwritten> {
    std::fs::create_dir_all(dir).map_err(Unwritten::Io)?;
    for (path, entry) in entries {
        let file = dir.join(path);
        if let Some(parent) = file.parent() {
            std::fs::create_dir_all(parent).map_err(Unwritten::Io)?;
        }
        match entry {
            Entry::File(content) => {
                std::fs::write(&file, content).map_err(Unwritten::Io)?;
                protect(&file, FileHeader::of(content).is_executable()).map_err(Unwritten::Io)?;
            }
            Entry::Link(link) => symlink(link, &file).map_err(|error| {
                Unwritten::Link(StoreError::Link {
                    path: path.clone(),
                    link: link.clone(),
                    error,
                })
            })?,
        }
    }
    Ok(())
}

/// What a folder of the cache holds: each file and link by its path in
/// it, with `/` between the parts, as [`store`] was given them.
pub fn read(dir: &Path) -> std::io::Result<BTreeMap<String, Entry>> {
    let mut entries = BTreeMap::new();
    read_into(dir, "", &mut entries)?;
    Ok(entries)
}

fn read_into(
    dir: &Path,
    prefix: &str,
    entries: &mut BTreeMap<String, Entry>,
) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let path = format!("{prefix}{}", name.to_string_lossy());
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            let link = std::fs::read_link(entry.path())?;
            entries.insert(path, Entry::Link(link.to_string_lossy().into_owned()));
        } else if kind.is_dir() {
            read_into(&entry.path(), &format!("{path}/"), entries)?;
        } else {
            entries.insert(path, Entry::File(std::fs::read(entry.path())?));
        }
    }
    Ok(())
}

/// Deletes a folder of the cache, read-only files and all. One that is
/// gone already is no error: another build deleted it.
pub fn remove(dir: &Path) -> std::io::Result<()> {
    make_writable(dir);
    match std::fs::remove_dir_all(dir) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        result => result,
    }
}

/// Makes the files under `dir` writable again, as far as it can, so that
/// they can be deleted.
fn make_writable(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() && !path.is_symlink() {
            make_writable(&path);
        } else if let Ok(metadata) = std::fs::symlink_metadata(&path) {
            let mut permissions = metadata.permissions();
            #[allow(clippy::permissions_set_readonly_false)]
            permissions.set_readonly(false);
            let _ = std::fs::set_permissions(&path, permissions);
        }
    }
}

/// A file of the cache is what its hash says: it is not to be written, so
/// that an editor that opens one does not write it back. It is to be run
/// when its first bytes say it is a program.
fn protect(file: &Path, executable: bool) -> std::io::Result<()> {
    #[cfg(unix)]
    let permissions = {
        use std::os::unix::fs::PermissionsExt;
        std::fs::Permissions::from_mode(if executable { 0o555 } else { 0o444 })
    };
    #[cfg(not(unix))]
    let permissions = {
        let _ = executable;
        let mut permissions = std::fs::metadata(file)?.permissions();
        permissions.set_readonly(true);
        permissions
    };
    std::fs::set_permissions(file, permissions)
}

fn symlink(target: &str, link: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    return std::os::unix::fs::symlink(target, link);
    #[cfg(windows)]
    return std::os::windows::fs::symlink_file(target, link);
    #[cfg(not(any(unix, windows)))]
    return Err(std::io::Error::other("no symbolic links on this system"));
}

/// The first bytes of a file, which say whether it is a program: an ELF or
/// a Mach-O header, or the `#!` of a script. An archive's own permissions
/// are not read, so this is what marks a file executable once unpacked.
#[derive(Clone, Copy, Debug, Default)]
pub struct FileHeader {
    header: [u8; 4],
    bytes_read: usize,
}

impl FileHeader {
    pub fn of(content: &[u8]) -> Self {
        let mut header = Self::default();
        header.update(content);
        header
    }

    pub fn update(&mut self, buf: &[u8]) {
        if self.bytes_read >= self.header.len() {
            return;
        }
        let n = (self.header.len() - self.bytes_read).min(buf.len());
        self.header[self.bytes_read..][..n].copy_from_slice(&buf[..n]);
        self.bytes_read += n;
    }

    fn starts_with(&self, magic: &[u8]) -> bool {
        self.header[..self.bytes_read.min(magic.len())] == *magic
    }

    fn is_mach_o(&self) -> bool {
        // The magic numbers of both byte orders.
        const MAGIC: [u32; 8] = [
            0xfeed_face,
            0xfeed_facf,
            0xcafe_babe,
            0xcafe_babf,
            0xcefa_edfe,
            0xcffa_edfe,
            0xbeba_feca,
            0xbfba_feca,
        ];
        self.bytes_read >= 4 && MAGIC.contains(&u32::from_be_bytes(self.header))
    }

    pub fn is_executable(&self) -> bool {
        self.starts_with(b"#!") || self.starts_with(b"\x7fELF") || self.is_mach_o()
    }
}
