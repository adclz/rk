// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! How the `[dependencies]` of a workspace, and those of each library it
//! names, are read: read-only, from folders and from the cache.

use std::fs;
use std::path::{Path, PathBuf};

use db::config_file::{CONFIG_FILE, parse_config};
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

/// The resolution of the workspace `root/ws`, whose `[dependencies]`
/// are `dependencies`.
fn resolved(root: &Path, dependencies: &str) -> Resolution {
    let workspace = library(root, "ws", "ws", dependencies);
    let source = fs::read_to_string(workspace.join(CONFIG_FILE)).unwrap();
    let config = parse_config(&source).unwrap();
    resolve(&workspace, &config, None, None, None)
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

/// A library names its own dependencies, and each is read once, in the
/// order they are met: one that two libraries share, and one that leads
/// back to the library that names it.
#[test]
fn a_library_brings_its_own_dependencies() {
    let t = tempfile::tempdir().unwrap();
    library(
        t.path(),
        "a",
        "a",
        "shared = { path = \"../shared\" }\nb = { path = \"../b\" }\n",
    );
    library(
        t.path(),
        "b",
        "b",
        "shared = { path = \"../shared\" }\na = { path = \"../a\" }\n",
    );
    library(t.path(), "shared", "shared", "");
    let resolution = resolved(t.path(), "a = { path = \"../a\" }\n");
    assert_eq!(messages(&resolution), [] as [&str; 0]);
    assert_eq!(names(&resolution), ["a", "b", "shared"]);
}

/// The standard library comes first, and a dependency of its name from
/// another folder does not replace it.
#[test]
fn the_standard_library_is_not_replaced() {
    let t = tempfile::tempdir().unwrap();
    let std = library(t.path(), "std", "stdlib", "");
    library(t.path(), "mine", "stdlib", "");
    let workspace = library(t.path(), "ws", "ws", "stdlib = { path = \"../mine\" }\n");
    let source = fs::read_to_string(workspace.join(CONFIG_FILE)).unwrap();
    let config = parse_config(&source).unwrap();
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
    assert!(resolution.problems[0].notes[0].starts_with("the compiler uses"));
}

/// The same library from two folders is an error: its names would be
/// declared twice.
#[test]
fn a_library_from_two_places_is_refused() {
    let t = tempfile::tempdir().unwrap();
    library(t.path(), "a", "a", "shared = { path = \"../old\" }\n");
    library(t.path(), "old", "shared", "");
    library(t.path(), "new", "shared", "");
    let resolution = resolved(
        t.path(),
        "a = { path = \"../a\" }\nshared = { path = \"../new\" }\n",
    );
    assert_eq!(names(&resolution), ["a", "shared"]);
    assert_eq!(
        messages(&resolution),
        ["'shared' is required from two places"]
    );
    let note = resolution.problems[0].notes[0].as_str();
    assert!(note.starts_with("'a' uses"), "{note}");
    assert!(note.contains("and the workspace uses"), "{note}");
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
/// to fetch it, one the cache lacks is a problem; one that names a
/// folder outside itself is too.
#[test]
fn a_fetched_library_is_read_from_the_cache() {
    let t = tempfile::tempdir().unwrap();
    let cache = t.path().join("cache");
    let hash = ContentHash::parse(&format!("sha256:{}", "ab".repeat(32))).unwrap();
    let url = "https://example.org/drives-1.tar.gz";
    let dependency = format!("drives = {{ url = \"{url}\", hash = \"{hash}\" }}\n");
    let workspace = library(t.path(), "ws", "ws", &dependency);
    let config = parse_config(&fs::read_to_string(workspace.join(CONFIG_FILE)).unwrap()).unwrap();

    let absent = resolve(&workspace, &config, None, Some(&cache), None);
    assert_eq!(messages(&absent), ["'drives' is not in the cache"]);

    // In the cache, as another URL gave it: not this pair.
    let drives = library(
        &packages_dir(&cache),
        hash.hex(),
        "drives",
        "inner = { path = \"inner\" }\nouter = { path = \"../../../outside\" }\n",
    );
    library(&drives, "inner", "inner", "");
    library(t.path(), "outside", "outer", "");
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
    assert_eq!(names(&fetched), ["drives", "inner"]);
    assert_eq!(
        messages(&fetched),
        ["'outer' (required by 'drives') has a `path` that leaves its library"]
    );

    // And read with no one to ask, now that the cache has the pair.
    let cached = resolve(&workspace, &config, None, Some(&cache), None);
    assert_eq!(names(&cached), ["drives", "inner"]);
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
