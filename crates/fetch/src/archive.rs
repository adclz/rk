// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! Reading an archive into a [`Tree`]: which kind of archive a file or an
//! HTTP response is, and the entries of a tarball.

use std::io::Read;

use db::dependencies::FetchError;

use crate::tree::{Entry, Tree, UnpackError};

/// How much an archive may unpack to. A library is source text: one past
/// this is not one, and is not read to the end.
pub const MAX_UNPACKED: u64 = 256 * 1024 * 1024;

/// The kind of archive a resource is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileType {
    Tar,
    TarGz,
    TarXz,
    TarZst,
    GitPack,
    Zip,
}

impl FileType {
    /// By the end of a file's name.
    pub fn from_path(file_path: &str) -> Option<Self> {
        let path = file_path.to_ascii_lowercase();
        let ends = |suffix: &str| path.ends_with(suffix);
        if ends(".tar") {
            Some(Self::Tar)
        } else if ends(".tgz") || ends(".tar.gz") {
            Some(Self::TarGz)
        } else if ends(".txz") || ends(".tar.xz") {
            Some(Self::TarXz)
        } else if ends(".tzst") || ends(".tar.zst") {
            Some(Self::TarZst)
        } else if ends(".zip") || ends(".jar") {
            Some(Self::Zip)
        } else {
            None
        }
    }

    /// By the file name of a `Content-Disposition: attachment` header.
    pub fn from_content_disposition(cd_header: &str) -> Option<Self> {
        let lower = cd_header.to_ascii_lowercase();
        let attach = lower.find("attachment;")?;
        let mut value_start = attach + 1 + lower[attach + 1..].find("filename")? + "filename".len();
        let bytes = cd_header.as_bytes();
        if bytes.get(value_start) == Some(&b'*') {
            value_start += 1;
        }
        if bytes.get(value_start) != Some(&b'=') {
            return None;
        }
        value_start += 1;

        let mut value_end = cd_header[value_start..]
            .find(';')
            .map_or(cd_header.len(), |end| value_start + end);
        if value_end > value_start && bytes[value_end - 1] == b'"' {
            value_end -= 1;
        }
        Self::from_path(&cd_header[value_start..value_end])
    }

    /// The kind an HTTP response is of. Its `Content-Type` comes first,
    /// then the file name of its `Content-Disposition`, then the path of
    /// the URL.
    pub fn from_response(
        content_type: Option<&str>,
        content_disposition: Option<&str>,
        uri_path: &str,
    ) -> Result<Self, FetchError> {
        let content_type =
            content_type.ok_or_else(|| FetchError::new("missing 'Content-Type' header"))?;
        // The MIME type, without its charset and boundary directives.
        let mime_type = content_type.split(';').next().unwrap_or_default();
        let is = |name: &str| mime_type.eq_ignore_ascii_case(name);
        if is("application/x-tar") {
            return Ok(Self::Tar);
        }
        if is("application/gzip")
            || is("application/x-gzip")
            || is("application/tar+gzip")
            || is("application/x-tar-gz")
            || is("application/x-gtar-compressed")
        {
            return Ok(Self::TarGz);
        }
        if is("application/x-xz") {
            return Ok(Self::TarXz);
        }
        if is("application/zstd") {
            return Ok(Self::TarZst);
        }
        if is("application/zip")
            || is("application/x-zip-compressed")
            || is("application/java-archive")
        {
            return Ok(Self::Zip);
        }
        if !is("application/octet-stream") && !is("application/x-compressed") {
            return Err(FetchError::new(format!(
                "unrecognized 'Content-Type' header: '{content_type}'"
            )));
        }
        if let Some(cd_header) = content_disposition {
            return Self::from_content_disposition(cd_header).ok_or_else(|| {
                FetchError::new(format!(
                    "unsupported Content-Disposition header value: '{cd_header}' for Content-Type=application/octet-stream"
                ))
            });
        }
        Self::from_path(uri_path)
            .ok_or_else(|| FetchError::new(format!("unknown file type: '{uri_path}'")))
    }
}

/// Unpacks an archive of the kind `file_type`.
pub fn unpack(file_type: FileType, bytes: &[u8]) -> Result<Tree, FetchError> {
    match file_type {
        FileType::Tar => unpack_tarball(bytes),
        FileType::TarGz => unpack_tarball(flate2::read::GzDecoder::new(bytes)),
        FileType::TarXz => Err(FetchError::new("unable to unpack: rk reads no .tar.xz")),
        FileType::TarZst => Err(FetchError::new("unable to unpack: rk reads no .tar.zst")),
        FileType::Zip => Err(FetchError::new("unable to unpack: rk reads no .zip")),
        FileType::GitPack => unreachable!("a pack is unpacked by the git client"),
    }
}

/// The entries of a tarball, without the folder that holds them all when
/// there is one. Its permissions are not read, and its empty folders are
/// left out.
fn unpack_tarball(reader: impl Read) -> Result<Tree, FetchError> {
    let unreadable =
        |error: std::io::Error| FetchError::new(format!("unable to read the tarball: {error}"));
    let mut entries = Vec::new();
    let mut errors = Vec::new();
    let mut root = Root::default();
    let mut unpacked = 0u64;
    let mut tar = tar::Archive::new(reader);
    for entry in tar.entries().map_err(unreadable)? {
        let mut entry = entry.map_err(unreadable)?;
        let name = String::from_utf8_lossy(&entry.path_bytes()).into_owned();
        use tar::EntryType;
        let kind = entry.header().entry_type();
        match kind {
            EntryType::Regular | EntryType::Continuous => {
                root.find(Kind::File, &name);
                unpacked += entry.size();
                if unpacked > MAX_UNPACKED {
                    return Err(FetchError::new(format!(
                        "unable to unpack tarball: it holds more than {} MiB",
                        MAX_UNPACKED / (1024 * 1024)
                    )));
                }
                let mut content = Vec::new();
                entry.read_to_end(&mut content).map_err(unreadable)?;
                entries.push((name, Entry::File(content)));
            }
            EntryType::Symlink => {
                root.find(Kind::Link, &name);
                let link = entry
                    .link_name_bytes()
                    .map(|link| String::from_utf8_lossy(&link).into_owned())
                    .unwrap_or_default();
                entries.push((name, Entry::Link(link)));
            }
            EntryType::Directory => root.find(Kind::Directory, &name),
            // The format's own.
            EntryType::XGlobalHeader
            | EntryType::XHeader
            | EntryType::GNULongName
            | EntryType::GNULongLink => {}
            other => errors.push((name, other.as_byte() as char)),
        }
    }

    let mut tree = Tree {
        root_error_message: "unable to unpack tarball",
        ..Tree::default()
    };
    for (name, kind) in errors {
        tree.errors.push(UnpackError::UnsupportedType {
            name: portable(root.strip(&name)).map_err(insecure)?,
            kind,
        });
    }
    for (name, entry) in entries {
        let path = portable(root.strip(&name)).map_err(insecure)?;
        if let Entry::Link(link) = &entry {
            stays_inside(&path, link).map_err(insecure)?;
        }
        tree.insert(path, entry);
    }
    Ok(tree)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    File,
    Link,
    Directory,
}

/// The one folder every entry of an archive is in, when there is one: the
/// archive a host makes of a tag has the package in `name-tag/`.
#[derive(Default)]
struct Root {
    entries: usize,
    dir: String,
}

impl Root {
    fn find(&mut self, kind: Kind, path: &str) {
        if path.is_empty() {
            return;
        }
        self.entries += 1;
        let root_dir = Self::of(path, kind);
        if self.entries == 1 {
            self.dir = root_dir.to_string();
            return;
        }
        if self.dir.is_empty() || root_dir == self.dir {
            return;
        }
        self.dir.clear();
    }

    /// The first folder of `path`. A file at the top is in none.
    fn of(path: &str, kind: Kind) -> &str {
        let buf = path.strip_prefix('/').unwrap_or(path);
        let buf = buf.strip_suffix('/').unwrap_or(buf);
        match buf.split_once('/') {
            Some((root, _)) => root,
            None if kind == Kind::Directory => buf,
            None => "",
        }
    }

    fn strip<'a>(&self, path: &'a str) -> &'a str {
        if self.dir.is_empty() {
            return path;
        }
        path.strip_prefix(self.dir.as_str())
            .and_then(|rest| rest.strip_prefix('/'))
            .unwrap_or(path)
    }
}

fn insecure(note: String) -> FetchError {
    FetchError::new("unable to unpack tarball").note(note)
}

/// `path` with one `/` between its parts, when it names a file inside the
/// package and by the same name on every system, and else why not.
///
/// A `\` is a separator on one system and a letter on another, and is
/// refused on both. So is a name one system keeps for a device (`CON`,
/// `nul.st`), a character one system does not take in a name, and a dot or
/// a space at the end of a name, which one system drops.
pub fn portable(path: &str) -> Result<String, String> {
    let outside = || format!("file '{path}' is outside the package");
    if path.starts_with('/') {
        return Err(outside());
    }
    let mut parts = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => return Err(outside()),
            part if !is_portable_name(part) => {
                return Err(format!(
                    "file '{path}' has a name that not every system accepts"
                ));
            }
            part => parts.push(part),
        }
    }
    Ok(parts.join("/"))
}

/// Whether every system takes `name` as the name of a file.
fn is_portable_name(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or(name).as_bytes();
    let device = [b"CON", b"PRN", b"AUX", b"NUL"]
        .iter()
        .any(|device| stem.eq_ignore_ascii_case(*device))
        || (stem.len() == 4
            && (stem[..3].eq_ignore_ascii_case(b"COM") || stem[..3].eq_ignore_ascii_case(b"LPT"))
            && stem[3].is_ascii_digit());
    !device
        && !name.ends_with(['.', ' '])
        && !name.contains(['\\', ':', '<', '>', '"', '|', '?', '*'])
        && !name.contains(char::is_control)
}

/// A link is refused when it points out of the package: what is there is
/// not in the hash, and would be read as part of the library.
pub fn stays_inside(path: &str, link: &str) -> Result<(), String> {
    let outside = || format!("symlink '{path}' points outside the package, to '{link}'");
    if link.starts_with('/') || link.contains(['\\', ':']) {
        return Err(outside());
    }
    // The folders from the root of the package to the link.
    let mut depth = path.split('/').count() - 1;
    for part in link.split('/') {
        match part {
            "" | "." => {}
            ".." => depth = depth.checked_sub(1).ok_or_else(outside)?,
            _ => depth += 1,
        }
    }
    Ok(())
}
