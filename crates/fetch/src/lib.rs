// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! Fetching a library: what a URL names is read, unpacked, hashed and kept
//! in the cache under the hash of what it holds
//! ([`db::dependencies::ContentHash`]), and its entry is written in a
//! `config.toml`.
//!
//! A URL names an archive, over `https`, `http` or `file`, or a commit of a
//! git repository, over `git+https` or `git+http`.
//!
//! `db` reads the libraries a workspace depends on and never downloads.
//! This crate is the one that does, and only `rk` links it: the language
//! server has no way to reach the network.
//!
//! 1. What the URL names is unpacked, in memory.
//! 2. Its `config.toml` is read: a library has one, with its name and the
//!    files that make it (`paths`).
//! 3. What the library leaves out is dropped, and with it the errors about
//!    entries that are not part of it.
//! 4. What is left is hashed.
//! 5. It is written beside its place in the cache and moved there whole.
//!    What the cache has under that hash already is left untouched: a
//!    build may be reading it.
//! 6. The hash is checked against the one the dependency declares.

pub mod archive;
pub mod git;
pub mod tree;

use std::path::{Path, PathBuf};

use auto_lsp::lsp_types::Url;
use db::cache::StoreError;
use db::config_file::CONFIG_FILE;
use db::dependencies::{ContentHash, Remote, cached_urls};

pub use db::dependencies::FetchError;

use archive::{FileType, MAX_UNPACKED};
use tree::{Entry, Filter, Tree};

/// A library the cache now holds.
#[derive(Debug)]
pub struct Fetched {
    /// The `name` of its `config.toml`.
    pub name: String,
    pub hash: ContentHash,
    pub dir: PathBuf,
    /// The URL to write down for it: the one given, or for a git ref, the
    /// same with the commit that ref names today.
    pub url: String,
}

/// How a git URL that names no commit is taken.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Refs {
    /// By `rk fetch --save`: it is resolved, and the commit is what is
    /// written down.
    Latest,
    /// By a dependency: it has to say which commit, since the same line
    /// would otherwise name another library tomorrow.
    Explicit,
}

/// Fetches what `url` names into `cache`.
pub fn fetch(cache: &Path, url: &str) -> Result<Fetched, FetchError> {
    fetch_with(cache, url, Refs::Latest, &library_manifest)
}

/// Fetches `remote`, which has to hold what its hash says.
pub fn fetch_remote(cache: &Path, remote: &Remote<'_>) -> Result<(), FetchError> {
    let fetched = fetch_with(cache, remote.url, Refs::Explicit, &library_manifest)?;
    if fetched.hash != *remote.hash {
        return Err(FetchError::new(format!(
            "hash mismatch: config.toml declares {} but the fetched library has {}",
            remote.hash, fetched.hash
        )));
    }
    Ok(())
}

/// The name of the package a tree holds, and the files that make it.
type Manifest<'a> = &'a dyn Fn(&Tree) -> Result<(String, Filter), FetchError>;

fn fetch_with(
    cache: &Path,
    url: &str,
    refs: Refs,
    manifest: Manifest<'_>,
) -> Result<Fetched, FetchError> {
    let (mut tree, url) = unpack(url, refs)?;
    let (name, filter) = manifest(&tree)?;
    tree.validate(&filter)?;
    tree.retain(&filter);
    let hash = tree.digest();
    let dir = db::cache::store(cache, &hash, &tree.entries).map_err(|error| match error {
        StoreError::Link { .. } => FetchError::new(tree.root_error_message).note(error.to_string()),
        StoreError::Write { .. } => FetchError::new(error.to_string()),
    })?;
    record_url(cache, &hash, &url)?;
    Ok(Fetched {
        name,
        hash,
        dir,
        url,
    })
}

/// A library's own account of itself: the `config.toml` at its root. The
/// files it lists in `paths` make the library, and `config.toml` with them.
fn library_manifest(tree: &Tree) -> Result<(String, Filter), FetchError> {
    let Some(Entry::File(config)) = tree.entries.get(CONFIG_FILE) else {
        return Err(FetchError::new("no config.toml at the root")
            .note("a library is a folder with a config.toml"));
    };
    let config = std::str::from_utf8(config)
        .map_err(|error| error.to_string())
        .and_then(|source| {
            db::config_file::parse_config(source).map_err(|error| error.message().to_string())
        })
        .map_err(|error| FetchError::new(format!("invalid config.toml: {error}")))?;
    let mut paths = config.project.paths;
    if !paths.is_empty() {
        paths.push(CONFIG_FILE.to_string());
    }
    Ok((config.project.name, Filter::new(paths)))
}

/// What `url` names, unpacked, and the URL to write down for it.
fn unpack(url: &str, refs: Refs) -> Result<(Tree, String), FetchError> {
    let uri = Url::parse(url)
        .map_err(|error| FetchError::new(format!("invalid URL '{url}': {error}")))?;
    match uri.scheme() {
        "file" => {
            let path = uri
                .to_file_path()
                .map_err(|()| FetchError::new(format!("unable to open '{url}': not a path")))?;
            let bytes = std::fs::read(&path).map_err(|error| {
                FetchError::new(format!("unable to open '{}': {error}", path.display()))
            })?;
            let file_type = FileType::from_path(uri.path())
                .ok_or_else(|| FetchError::new(format!("unknown file type: '{}'", uri.path())))?;
            Ok((archive::unpack(file_type, &bytes)?, url.to_string()))
        }
        "http" | "https" => {
            let response = agent(10).get(url).call().map_err(|error| match error {
                ureq::Error::StatusCode(code) => {
                    FetchError::new(format!("bad HTTP response code: '{code}'"))
                }
                error => FetchError::new(format!("unable to connect to server: {error}")),
            })?;
            let header = |name: &str| {
                response
                    .headers()
                    .get(name)
                    .and_then(|value| value.to_str().ok())
                    .map(str::to_string)
            };
            let content_type = header("content-type");
            let content_disposition = header("content-disposition");
            let file_type = FileType::from_response(
                content_type.as_deref(),
                content_disposition.as_deref(),
                uri.path(),
            )?;
            let bytes = body(response)
                .map_err(|error| FetchError::new(format!("HTTP response read failure: {error}")))?;
            Ok((archive::unpack(file_type, &bytes)?, url.to_string()))
        }
        "git+http" | "git+https" => unpack_git(&uri, refs),
        scheme => Err(FetchError::new(format!("unsupported URL scheme: {scheme}"))),
    }
}

/// The files of the commit a `git+` URL names after its `#`: a commit, a
/// branch or a tag, and the server's `HEAD` when it names none.
fn unpack_git(uri: &Url, refs: Refs) -> Result<(Tree, String), FetchError> {
    // The server is at the URL without what follows its path.
    let mut location = uri.clone();
    location.set_query(None);
    location.set_fragment(None);
    let location = location.as_str();
    let transport = &location["git+".len()..];

    let session = git::Session::init(transport).map_err(|error| {
        FetchError::new(format!(
            "unable to start a session with the git server: {error}"
        ))
    })?;
    let want_ref = uri.fragment().unwrap_or("HEAD");
    let want_oid = git::resolve(&session, want_ref)?;
    if refs == Refs::Explicit && uri.fragment().is_none() {
        return Err(FetchError::new("the url names no commit after its `#`")
            .note(format!("try url = \"{location}#{want_oid}\"")));
    }

    let want = want_oid.to_string();
    let pack = session
        .fetch(&[&want])
        .map_err(|error| FetchError::new(format!("unable to fetch the pack: {error}")))?;
    let unpacked = |error: git::Error| FetchError::new(format!("unable to read the pack: {error}"));
    let tree = git::Repository::new(want_oid.format(), pack)
        .and_then(|mut repository| repository.checkout(&want_oid))
        .map_err(unpacked)?;
    // A tree is checked as an archive is: git takes a name like `..` or
    // `aux.st` from whoever wrote the pack.
    let refused = |note: String| FetchError::new(tree.root_error_message).note(note);
    for (path, entry) in &tree.entries {
        if archive::portable(path).map_err(refused)? != *path {
            return Err(refused(format!(
                "file '{path}' has a name that not every system accepts"
            )));
        }
        if let Entry::Link(link) = entry {
            archive::stays_inside(path, link).map_err(refused)?;
        }
    }

    // The commit takes the place of the ref, which is kept as a query: it
    // says what to ask for the day the library is to be moved on.
    let mut saved = uri.clone();
    if refs == Refs::Latest && uri.fragment() != Some(want.as_str()) {
        if let Some(named) = uri.fragment() {
            saved.set_query(Some(&format!("ref={named}")));
        }
        saved.set_fragment(Some(&want));
    }
    Ok((tree, saved.to_string()))
}

/// Notes that `url` gave `hash`, which is what makes the pair one the
/// cache answers for ([`db::dependencies::is_cached`]).
fn record_url(cache: &Path, hash: &ContentHash, url: &str) -> Result<(), FetchError> {
    use std::io::Write;
    let list = cached_urls(cache, hash);
    let known = std::fs::read_to_string(&list).unwrap_or_default();
    if known.lines().any(|line| line == url) {
        return Ok(());
    }
    // The line in one write: two builds that fetched at once append to the
    // same list, and a line written in two parts would mix with the other's.
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&list)
        .and_then(|mut file| file.write_all(format!("{url}\n").as_bytes()))
        .map_err(|error| {
            FetchError::new(format!(
                "unable to write in the cache at '{}': {error}",
                list.display()
            ))
        })
}

/// An HTTP client under the platform's trust store, that follows up to
/// `max_redirects` redirections.
pub(crate) fn agent(max_redirects: u32) -> ureq::Agent {
    use ureq::tls::{RootCerts, TlsConfig};
    ureq::Agent::config_builder()
        .tls_config(
            TlsConfig::builder()
                .root_certs(RootCerts::PlatformVerifier)
                .build(),
        )
        .max_redirects(max_redirects)
        .build()
        .new_agent()
}

/// The body of a response, which a library does not make longer than
/// [`MAX_UNPACKED`].
pub(crate) fn body(response: ureq::http::Response<ureq::Body>) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let mut bytes = Vec::new();
    response
        .into_body()
        .as_reader()
        .take(MAX_UNPACKED + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() as u64 > MAX_UNPACKED {
        return Err(format!("more than {} MiB", MAX_UNPACKED / (1024 * 1024)));
    }
    Ok(bytes)
}

/// The bytes an `https` or `http` URL serves, whatever their number.
pub fn get(url: &str) -> Result<Vec<u8>, String> {
    let mut body = agent(10)
        .get(url)
        .call()
        .map_err(|e| format!("fetching {url}: {e}"))?
        .into_body();
    let mut bytes = Vec::new();
    std::io::copy(&mut body.as_reader(), &mut bytes).map_err(|e| format!("reading {url}: {e}"))?;
    Ok(bytes)
}

/// Writes `fetched` in the `[dependencies]` of `config_file`, under the
/// name the library gives itself and with the URL to write down for it. An
/// entry of that name is replaced, which is how a library is moved to
/// another version. The rest of the file is left as it is written.
pub fn save(config_file: &Path, fetched: &Fetched) -> Result<(), String> {
    let source = std::fs::read_to_string(config_file)
        .map_err(|e| format!("reading {}: {e}", config_file.display()))?;
    let mut document = source
        .parse::<toml_edit::DocumentMut>()
        .map_err(|e| format!("{}: {e}", config_file.display()))?;
    let dependencies = document
        .entry("dependencies")
        .or_insert(toml_edit::table())
        .as_table_like_mut()
        .ok_or("`dependencies` is not a table in config.toml")?;
    let mut entry = toml_edit::InlineTable::new();
    entry.insert("url", fetched.url.as_str().into());
    entry.insert("hash", fetched.hash.to_string().into());
    dependencies.insert(&fetched.name, toml_edit::value(entry));
    std::fs::write(config_file, document.to_string())
        .map_err(|e| format!("writing {}: {e}", config_file.display()))
}
