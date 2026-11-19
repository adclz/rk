// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! The libraries a workspace reads beside its own files: the standard
//! library, which comes with the compiler, and the `[dependencies]` of its
//! `config.toml`.
//!
//! A dependency is a folder (`path`), or an archive (`url`) and the hash of
//! what it holds (`hash`). There is no registry and nothing to solve: the
//! hash names one content, and the URL is a place that serves it. A library
//! has no dependencies of its own: the workspace names every library it
//! reads, and a library whose `config.toml` names some is refused, nothing
//! it names read.
//!
//! A library's name is its namespace, and names are case-insensitive: two
//! entries whose names differ only in case are one name, and both are
//! refused, as is an entry whose name differs from the standard library's
//! only in case.
//!
//! This module only reads. A fetched library is looked up in the cache, and
//! fetching it is the caller's ([`Fetch`]): `rk` does it, the editor does
//! not.

use std::collections::BTreeMap;
use std::ops::Range;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::cache::Entry;
use crate::config_file::{CONFIG_FILE, Config, Dependency, parse_config};

/// A library's place in [`crate::workspace::Workspace::libraries`].
pub type LibraryId = u32;

/// Whether `name` can name a namespace: letters, digits and `_`, not
/// starting with a digit. A library's name is the namespace it declares
/// everything in, so it has to be one.
pub fn is_namespace_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|first| first.is_alphabetic() || first == '_')
        && chars.all(|c| c.is_alphanumeric() || c == '_')
}

/// `name` with what a namespace name does not take replaced by `_`: what
/// to suggest.
pub fn as_namespace_name(name: &str) -> String {
    let mut fixed: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if fixed
        .chars()
        .next()
        .is_some_and(|first| first.is_ascii_digit())
    {
        fixed.insert(0, '_');
    }
    if fixed.is_empty() {
        fixed.push('_');
    }
    fixed
}

/// A library: a folder that holds a `config.toml` and sources.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Library {
    /// The `name` of its `config.toml`. A library is one library by its
    /// name: asked for twice, it has to be the same folder both times.
    pub name: String,
    pub dir: PathBuf,
    /// Whether it is the standard library, which no `config.toml` names.
    pub standard: bool,
}

impl Library {
    /// The standard library, at `dir`. It takes the name its `config.toml`
    /// gives it, and `std` when it has none.
    pub fn standard(dir: PathBuf) -> Self {
        let name = std::fs::read_to_string(dir.join(CONFIG_FILE))
            .ok()
            .and_then(|source| parse_config(&source).ok())
            .map_or_else(|| "std".to_string(), |config| config.project.name);
        Self {
            name,
            dir,
            standard: true,
        }
    }
}

/// What a library holds: `sha256:` and 64 hexadecimal digits.
///
/// It is the hash of the files, not of the archive, which a host may build
/// again with other bytes ([`ContentHash::of`]).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ContentHash(String);

impl ContentHash {
    pub const PREFIX: &'static str = "sha256:";

    /// The hash of a library's entries, by their paths. A path is relative
    /// to the library's folder, with `/` between its parts.
    ///
    /// Each entry is hashed alone: a file as its path, two zero bytes and
    /// its content, a link as its path and what it points at. The hash is
    /// the one of those hashes, in the order of the paths.
    pub fn of(entries: &BTreeMap<String, Entry>) -> Self {
        let mut all = Sha256::new();
        for (path, entry) in entries {
            let mut one = Sha256::new();
            one.update(path.as_bytes());
            match entry {
                Entry::File(content) => {
                    // Where an executable bit would be, which no file has:
                    // not every system stores one.
                    one.update([0, 0]);
                    one.update(content);
                }
                Entry::Link(target) => one.update(target.as_bytes()),
            }
            all.update(one.finalize());
        }
        Self::from_digest(&all.finalize().into())
    }

    /// The hash `text` writes, when it is one.
    pub fn parse(text: &str) -> Option<Self> {
        let hex = text.strip_prefix(Self::PREFIX)?;
        (hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit()))
            .then(|| Self(hex.to_ascii_lowercase()))
    }

    pub fn from_digest(digest: &[u8; 32]) -> Self {
        Self(digest.iter().map(|byte| format!("{byte:02x}")).collect())
    }

    /// The 64 digits, without the prefix: the name of its folder in the
    /// cache.
    pub fn hex(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ContentHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{}", Self::PREFIX, self.0)
    }
}

/// The folder of the cache that holds the libraries, one per hash.
pub fn packages_dir(cache: &Path) -> PathBuf {
    cache.join("p")
}

/// Where the cache keeps what `hash` names.
pub fn cached_dir(cache: &Path, hash: &ContentHash) -> PathBuf {
    packages_dir(cache).join(hash.hex())
}

/// The URLs that gave `hash`, one per line, beside its folder.
pub fn cached_urls(cache: &Path, hash: &ContentHash) -> PathBuf {
    packages_dir(cache).join(format!("{}.urls", hash.hex()))
}

/// Whether the cache holds `hash` as what `url` gave.
///
/// The hash alone does not say it. A `url` changed for a newer archive, with
/// the hash of the older one left beside it, names a folder the cache has:
/// the build would go on with the old library on this machine, and fail on
/// the next one. So a pair is trusted once that URL has given that hash
/// here.
pub fn is_cached(cache: &Path, hash: &ContentHash, url: &str) -> bool {
    cached_dir(cache, hash).is_dir()
        && std::fs::read_to_string(cached_urls(cache, hash))
            .is_ok_and(|urls| urls.lines().any(|line| line == url))
}

/// A dependency the cache lacks.
pub struct Remote<'a> {
    pub name: &'a str,
    pub url: &'a str,
    pub hash: &'a ContentHash,
}

/// Why a library could not be fetched: what failed, and one note for each
/// thing that led to it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FetchError {
    pub message: String,
    pub notes: Vec<String>,
}

impl FetchError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            notes: Vec::new(),
        }
    }

    pub fn note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }
}

/// One `error:` line and a `note:` line for each note.
impl std::fmt::Display for FetchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "error: {}", self.message)?;
        for note in &self.notes {
            writeln!(f, "    note: {note}")?;
        }
        Ok(())
    }
}

/// Puts a [`Remote`] in the cache, or says what stopped it.
pub type Fetch<'a> = &'a mut dyn FnMut(&Remote<'_>) -> Result<(), FetchError>;

/// What is wrong with a dependency, in the words of a diagnostic.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Problem {
    /// The entry of the workspace's own `[dependencies]` it comes from, in
    /// the text of its `config.toml`.
    pub span: Range<usize>,
    pub message: String,
    pub notes: Vec<String>,
    pub help: Option<String>,
}

impl Problem {
    fn new(span: &Range<usize>, message: String) -> Self {
        Self {
            span: span.clone(),
            message,
            notes: Vec::new(),
            help: None,
        }
    }

    fn note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }

    fn help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }
}

/// The libraries a workspace depends on, in the order they are read, and
/// what could not be resolved.
#[derive(Default)]
pub struct Resolution {
    pub libraries: Vec<Library>,
    pub problems: Vec<Problem>,
}

const SHAPE: &str = "a dependency is a folder (`path`) or an archive (`url` and `hash`)";

/// Whether one system or another reads `path` from a root: `/x` and `\x`,
/// or a drive, `C:x`. A `config.toml` is read the same on every system.
fn is_absolute_anywhere(path: &str) -> bool {
    Path::new(path).is_absolute()
        || path.starts_with(['/', '\\'])
        || matches!(path.as_bytes(), [drive, b':', ..] if drive.is_ascii_alphabetic())
}

/// Resolves the `[dependencies]` of `config`, the configuration of the
/// workspace at `workspace`.
///
/// `standard` is the standard library, when there is one: it comes first
/// and no dependency replaces it. With no `fetch`, a library the cache
/// lacks is a problem.
pub fn resolve(
    workspace: &Path,
    config: &Config,
    standard: Option<Library>,
    cache: Option<&Path>,
    fetch: Option<Fetch<'_>>,
) -> Resolution {
    let workspace = workspace
        .canonicalize()
        .unwrap_or_else(|_| workspace.to_path_buf());
    let mut resolver = Resolver {
        workspace,
        cache,
        fetch,
        resolution: Resolution::default(),
    };
    let mut clashes = case_clashes(&config.dependencies, standard.as_ref());
    resolver.resolution.libraries.extend(standard);
    for (name, dependency) in &config.dependencies {
        let span = name.span();
        let name = name.get_ref();
        if let Some(problem) = clashes.remove(name.as_str()) {
            resolver.resolution.problems.push(problem);
            continue;
        }
        let read = resolver
            .locate(name, dependency, &span)
            .and_then(|dir| resolver.read(name, dir, &span));
        if let Err(problem) = read {
            resolver.resolution.problems.push(problem);
        }
    }
    // In the order of the file: the entries are read in the order of their
    // names.
    resolver
        .resolution
        .problems
        .sort_by_key(|problem| problem.span.start);
    resolver.resolution
}

/// The rule the problems about two names that are one rest on.
const CASE_INSENSITIVE: &str = "a library's name is its namespace, and names are case-insensitive";

/// The entries of `dependencies` whose name is another's in another case:
/// another entry's, or the standard library's. The two would be one
/// namespace, so each such entry is refused before anything is read or
/// fetched for it, and the standard library is kept.
///
/// Folded with `to_lowercase`, as names are matched: what the compiler
/// takes for one name is one name here.
fn case_clashes<'a>(
    dependencies: &'a BTreeMap<toml::Spanned<String>, Dependency>,
    standard: Option<&Library>,
) -> BTreeMap<&'a str, Problem> {
    let mut clashes = BTreeMap::new();
    for entry in dependencies.keys() {
        let name = entry.get_ref().as_str();
        let folded = name.to_lowercase();
        let one_name = |other: &str| other != name && other.to_lowercase() == folded;
        let span = entry.span();
        let problem = if let Some(standard) = standard.filter(|standard| one_name(&standard.name)) {
            Problem::new(
                &span,
                format!(
                    "'{name}' and '{}', the standard library, are one name",
                    standard.name
                ),
            )
            .help("remove this entry")
        } else if let Some(other) = dependencies
            .keys()
            .map(|other| other.get_ref().as_str())
            .find(|other| one_name(other))
        {
            Problem::new(&span, format!("'{name}' and '{other}' are one name"))
                .help("keep one of them")
        } else {
            continue;
        };
        clashes.insert(name, problem.note(CASE_INSENSITIVE));
    }
    clashes
}

struct Resolver<'c, 'f> {
    workspace: PathBuf,
    cache: Option<&'c Path>,
    fetch: Option<Fetch<'f>>,
    resolution: Resolution,
}

impl Resolver<'_, '_> {
    /// The folder the dependency `name` names. `span` is its entry.
    fn locate(
        &mut self,
        name: &str,
        dependency: &Dependency,
        span: &Range<usize>,
    ) -> Result<PathBuf, Problem> {
        match (&dependency.path, &dependency.url) {
            (None, None) => {
                Err(Problem::new(span, format!("'{name}' has no `path` and no `url`")).note(SHAPE))
            }
            (Some(_), Some(_)) => Err(Problem::new(
                span,
                format!("'{name}' has a `path` and a `url`"),
            )
            .note(SHAPE)
            .help("keep one of the two")),
            (Some(path), None) => {
                if dependency.hash.is_some() {
                    return Err(
                        Problem::new(span, format!("'{name}' has a `path` and a `hash`"))
                            .note("a `hash` goes with a `url`")
                            .help("remove the `hash`"),
                    );
                }
                if is_absolute_anywhere(path) {
                    return Err(Problem::new(span, format!("'{path}' is an absolute path"))
                        .note("a `path` is relative to the folder of its config.toml"));
                }
                if path.contains('\\') {
                    return Err(Problem::new(
                        span,
                        format!("'{path}' has `\\` between its folders"),
                    )
                    .note("a `path` is written with `/`, the same on every system")
                    .help(format!("write '{}'", path.replace('\\', "/"))));
                }
                let dir = self.workspace.join(path);
                match dir.canonicalize() {
                    Ok(dir) if dir.is_dir() => Ok(dir),
                    _ => Err(Problem::new(
                        span,
                        format!("'{}' is not a folder", dir.display()),
                    )),
                }
            }
            (None, Some(url)) => {
                let fetch_and_save = format!("run `rk fetch --save {url}`");
                let Some(hash) = &dependency.hash else {
                    return Err(Problem::new(span, format!("'{name}' has no `hash`"))
                        .note("a `url` goes with the `hash` of what it serves")
                        .help(fetch_and_save));
                };
                let Some(hash) = ContentHash::parse(hash) else {
                    return Err(Problem::new(span, format!("'{hash}' is not a hash"))
                        .note("a hash is `sha256:` followed by 64 hexadecimal digits")
                        .help(fetch_and_save));
                };
                let Some(cache) = self.cache else {
                    return Err(Problem::new(
                        span,
                        format!("there is no cache directory for '{name}'"),
                    )
                    .help(format!("set {}", crate::cache::CACHE_DIR_ENV)));
                };
                if !is_cached(cache, &hash, url) {
                    let Some(fetch) = self.fetch.as_mut() else {
                        return Err(Problem::new(span, format!("'{name}' is not in the cache"))
                            .help("run `rk fetch`"));
                    };
                    let remote = Remote {
                        name,
                        url,
                        hash: &hash,
                    };
                    if let Err(error) = fetch(&remote) {
                        let mut problem =
                            Problem::new(span, format!("'{name}' could not be fetched"))
                                .note(error.message);
                        problem.notes.extend(error.notes);
                        return Err(problem);
                    }
                }
                let dir = cached_dir(cache, &hash);
                Ok(dir.canonicalize().unwrap_or(dir))
            }
        }
    }

    /// Reads the library `name` at `dir`, which `span`, its entry, names.
    fn read(&mut self, name: &str, dir: PathBuf, span: &Range<usize>) -> Result<(), Problem> {
        if dir == self.workspace {
            return Err(Problem::new(
                span,
                format!("'{name}' is the workspace itself"),
            ));
        }
        let config_file = dir.join(CONFIG_FILE);
        let Ok(source) = std::fs::read_to_string(&config_file) else {
            return Err(
                Problem::new(span, format!("'{}' has no config.toml", dir.display()))
                    .note("a library is a folder with a config.toml"),
            );
        };
        let config = parse_config(&source).map_err(|error| {
            Problem::new(
                span,
                format!(
                    "the config.toml of '{name}' is not valid: {}",
                    error.message()
                ),
            )
        })?;
        if config.project.name != name {
            return Err(Problem::new(
                span,
                format!(
                    "the library at '{}' is named '{}', not '{name}'",
                    dir.display(),
                    config.project.name
                ),
            )
            .note("a dependency has the name written in its library's config.toml")
            .help(format!("rename it to '{}'", config.project.name)));
        }
        if !is_namespace_name(name) {
            return Err(
                Problem::new(span, format!("'{name}' is not a namespace name"))
                    .note("a library's name is the namespace its declarations are in")
                    .help(format!("name it '{}'", as_namespace_name(name))),
            );
        }
        // Only the standard library can have the name already: it is read
        // first, and each entry of the workspace is a name of its own, one
        // that is another's in another case refused before it is read
        // ([`case_clashes`]).
        if let Some(standard) = self
            .resolution
            .libraries
            .iter()
            .find(|library| library.name == name)
        {
            if standard.dir == dir {
                // Its own folder: read already.
                return Ok(());
            }
            return Err(
                Problem::new(span, format!("'{name}' is required from two places"))
                    .note(format!(
                        "the compiler uses '{}' and the workspace uses '{}'",
                        standard.dir.display(),
                        dir.display()
                    ))
                    .help(format!(
                        "remove this entry, or name its folder in {}",
                        crate::stdlib::PATH_ENV
                    )),
            );
        }
        if !config.dependencies.is_empty() {
            return Err(
                Problem::new(span, format!("'{name}' has dependencies of its own")).note(
                    "a library has no `[dependencies]`: the workspace lists every library it reads",
                ),
            );
        }
        self.resolution.libraries.push(Library {
            name: name.to_string(),
            dir,
            standard: false,
        });
        Ok(())
    }
}
