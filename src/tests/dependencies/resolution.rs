// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! How the `[dependencies]` of a workspace are read: read-only, from
//! folders and from the cache. A library has none of its own.

use std::fs;
use std::path::{Path, PathBuf};

use db::config_file::{CONFIG_FILE, Config, parse_config};
use db::dependencies::{
    ContentHash, Library, Remote, Resolution, cached_urls, packages_dir, resolve,
};

/// A folder under `root` with a `config.toml` that names it `name`, and
/// the `[dependencies]` given.
fn library(root: &Path, dir: &str, name: &str, dependencies: &str) -> PathBuf {
    let dir = root.join(dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join(CONFIG_FILE),
        format!("[project]\nname = \"{name}\"\nversion = \"1\"\n\n[dependencies]\n{dependencies}"),
    )
    .unwrap();
    dir.canonicalize().unwrap()
}

/// The workspace `root/ws`, whose `[dependencies]` are `dependencies`, and
/// its configuration.
fn workspace_of(root: &Path, dependencies: &str) -> (PathBuf, Config) {
    let workspace = library(root, "ws", "ws", dependencies);
    let source = fs::read_to_string(workspace.join(CONFIG_FILE)).unwrap();
    (workspace, parse_config(&source).unwrap())
}

/// The resolution of the workspace `root/ws`, whose `[dependencies]`
/// are `dependencies`, with no standard library and no cache.
fn resolved(root: &Path, dependencies: &str) -> Resolution {
    let (workspace, config) = workspace_of(root, dependencies);
    resolve(&workspace, &config, None, None, None)
}

/// The hash of `byte` written 32 times, as a dependency writes it.
fn a_hash(byte: &str) -> ContentHash {
    ContentHash::parse(&format!("sha256:{}", byte.repeat(32))).unwrap()
}

fn names(resolution: &Resolution) -> Vec<&str> {
    resolution
        .libraries
        .iter()
        .map(|library| library.name.as_str())
        .collect()
}

fn messages(resolution: &Resolution) -> Vec<&str> {
    resolution
        .problems
        .iter()
        .map(|problem| problem.message.as_str())
        .collect()
}

/// A library has no dependencies of its own: one whose `config.toml` names
/// some is refused, and what it names is not read. An empty
/// `[dependencies]` table names none.
#[test]
fn a_library_with_dependencies_is_refused() {
    let t = tempfile::tempdir().unwrap();
    library(t.path(), "a", "a", "shared = { path = \"../shared\" }\n");
    library(t.path(), "b", "b", "");
    library(t.path(), "shared", "shared", "");
    let resolution = resolved(
        t.path(),
        "a = { path = \"../a\" }\nb = { path = \"../b\" }\n",
    );
    assert_eq!(names(&resolution), ["b"]);
    assert_eq!(messages(&resolution), ["'a' has dependencies of its own"]);
    assert_eq!(
        resolution.problems[0].notes,
        ["a library has no `[dependencies]`: the workspace lists every library it reads"]
    );
}

/// The standard library comes first, and a dependency of its name from
/// another folder does not replace it.
#[test]
fn the_standard_library_is_not_replaced() {
    let t = tempfile::tempdir().unwrap();
    let std = library(t.path(), "std", "stdlib", "");
    library(t.path(), "mine", "stdlib", "");
    let (workspace, config) = workspace_of(t.path(), "stdlib = { path = \"../mine\" }\n");
    let resolution = resolve(
        &workspace,
        &config,
        Some(Library::standard(std)),
        None,
        None,
    );
    assert_eq!(names(&resolution), ["stdlib"]);
    assert!(resolution.libraries[0].standard);
    assert_eq!(
        messages(&resolution),
        ["'stdlib' is required from two places"]
    );
    let problem = &resolution.problems[0];
    assert!(problem.notes[0].starts_with("the compiler uses"));
    assert!(problem.notes[0].contains("and the workspace uses"));
    assert_eq!(
        problem.help.as_deref(),
        Some("remove this entry, or name its folder in RK_STDLIB_PATH")
    );
}

/// The workspace may name the standard library's own folder: it is read
/// once, as the standard library.
#[test]
fn the_standard_library_s_own_folder_is_read_once() {
    let t = tempfile::tempdir().unwrap();
    let std = library(t.path(), "std", "stdlib", "");
    let (workspace, config) = workspace_of(t.path(), "stdlib = { path = \"../std\" }\n");
    let resolution = resolve(
        &workspace,
        &config,
        Some(Library::standard(std)),
        None,
        None,
    );
    assert_eq!(messages(&resolution), [] as [&str; 0]);
    assert_eq!(names(&resolution), ["stdlib"]);
    assert!(resolution.libraries[0].standard);
}

/// Each way an entry can be written wrong, in its own words.
#[test]
fn an_entry_written_wrong_is_reported() {
    let t = tempfile::tempdir().unwrap();
    library(t.path(), "named", "other", "");
    fs::create_dir_all(t.path().join("bare")).unwrap();
    let absolute = t.path().canonicalize().unwrap().join("named");
    let resolution = resolved(
        t.path(),
        &format!(
            "a_nothing = {{}}\n\
             b_both = {{ path = \"../named\", url = \"https://x/y.tar.gz\" }}\n\
             c_hashed = {{ path = \"../named\", hash = \"sha256:00\" }}\n\
             d_absolute = {{ path = {absolute:?} }}\n\
             e_missing = {{ path = \"../nowhere\" }}\n\
             f_bare = {{ path = \"../bare\" }}\n\
             g_named = {{ path = \"../named\" }}\n\
             h_unhashed = {{ url = \"https://x/y.tar.gz\" }}\n\
             i_malformed = {{ url = \"https://x/y.tar.gz\", hash = \"sha256:00\" }}\n\
             j_itself = {{ path = \".\" }}\n"
        ),
    );
    assert_eq!(names(&resolution), [] as [&str; 0]);
    let messages = messages(&resolution);
    assert_eq!(messages.len(), 10, "{messages:#?}");
    assert_eq!(messages[0], "'a_nothing' has no `path` and no `url`");
    assert_eq!(messages[1], "'b_both' has a `path` and a `url`");
    assert_eq!(messages[2], "'c_hashed' has a `path` and a `hash`");
    assert!(
        messages[3].ends_with("is an absolute path"),
        "{}",
        messages[3]
    );
    assert!(messages[4].ends_with("is not a folder"), "{}", messages[4]);
    assert!(
        messages[5].ends_with("has no config.toml"),
        "{}",
        messages[5]
    );
    assert!(
        messages[6].ends_with("is named 'other', not 'g_named'"),
        "{}",
        messages[6]
    );
    assert_eq!(messages[7], "'h_unhashed' has no `hash`");
    assert_eq!(messages[8], "'sha256:00' is not a hash");
    assert_eq!(messages[9], "'j_itself' is the workspace itself");
}

/// A fetched library is looked up by its hash and its URL. With no one
/// to fetch it, one the cache lacks is a problem.
#[test]
fn a_fetched_library_is_read_from_the_cache() {
    let t = tempfile::tempdir().unwrap();
    let cache = t.path().join("cache");
    let hash = a_hash("ab");
    let url = "https://example.org/drives-1.tar.gz";
    let (workspace, config) = workspace_of(
        t.path(),
        &format!("drives = {{ url = \"{url}\", hash = \"{hash}\" }}\n"),
    );

    let absent = resolve(&workspace, &config, None, Some(&cache), None);
    assert_eq!(messages(&absent), ["'drives' is not in the cache"]);

    // In the cache, as another URL gave it: not this pair.
    library(&packages_dir(&cache), hash.hex(), "drives", "");
    fs::write(
        cached_urls(&cache, &hash),
        "https://example.org/other.tar.gz\n",
    )
    .unwrap();
    let other_url = resolve(&workspace, &config, None, Some(&cache), None);
    assert_eq!(messages(&other_url), ["'drives' is not in the cache"]);

    // Fetched by the caller, which is asked once.
    let mut asked = Vec::new();
    let mut fetch = |remote: &Remote<'_>| {
        asked.push(format!("{} {} {}", remote.name, remote.url, remote.hash));
        fs::write(
            cached_urls(&cache, remote.hash),
            format!("{}\n", remote.url),
        )
        .unwrap();
        Ok(())
    };
    let fetched = resolve(&workspace, &config, None, Some(&cache), Some(&mut fetch));
    assert_eq!(asked, [format!("drives {url} {hash}")]);
    assert_eq!(messages(&fetched), [] as [&str; 0]);
    assert_eq!(names(&fetched), ["drives"]);

    // And read with no one to ask, now that the cache has the pair.
    let cached = resolve(&workspace, &config, None, Some(&cache), None);
    assert_eq!(names(&cached), ["drives"]);
}

/// What a fetched library names is never fetched: the library is fetched,
/// since its `config.toml` is in what it holds, and refused.
#[test]
fn what_a_library_names_is_not_fetched() {
    let t = tempfile::tempdir().unwrap();
    let cache = t.path().join("cache");
    let hash = a_hash("ab");
    let url = "https://example.org/drives-1.tar.gz";
    let (workspace, config) = workspace_of(
        t.path(),
        &format!("drives = {{ url = \"{url}\", hash = \"{hash}\" }}\n"),
    );
    library(
        &packages_dir(&cache),
        hash.hex(),
        "drives",
        &format!(
            "units = {{ url = \"https://example.org/units-1.tar.gz\", hash = \"{}\" }}\n",
            a_hash("cd")
        ),
    );

    let mut asked = Vec::new();
    let mut fetch = |remote: &Remote<'_>| {
        asked.push(remote.name.to_string());
        fs::write(
            cached_urls(&cache, remote.hash),
            format!("{}\n", remote.url),
        )
        .unwrap();
        Ok(())
    };
    let resolution = resolve(&workspace, &config, None, Some(&cache), Some(&mut fetch));
    assert_eq!(asked, ["drives"]);
    assert_eq!(names(&resolution), [] as [&str; 0]);
    assert_eq!(
        messages(&resolution),
        ["'drives' has dependencies of its own"]
    );
}

#[test]
fn a_hash_is_read_and_written_the_same() {
    let text = format!("sha256:{}", "0F".repeat(32));
    let hash = ContentHash::parse(&text).unwrap();
    assert_eq!(hash.to_string(), text.to_ascii_lowercase());
    assert_eq!(ContentHash::from_digest(&[0x0f; 32]), hash);
    for wrong in ["", "sha256:", "sha256:zz", &"0f".repeat(32), "md5:00"] {
        assert!(ContentHash::parse(wrong).is_none(), "{wrong}");
    }
}
