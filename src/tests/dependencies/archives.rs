// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! A library fetched from an archive: what makes it, what its hash is
//! computed from, and what is refused.

use db::dependencies::{cached_urls, packages_dir};
use fetch::archive::{FileType, stays_inside};
use fetch::tree::{Entry, FileHeader, Filter, Tree, UnpackError};
use fetch::{FetchError, fetch};

use super::{Answer, CONFIG, Expected, archive, expected_hash, package_files, serve, served};

/// The archive a host makes of a tag holds the library in one folder,
/// which is not part of it: the same files with and without it, and in
/// another order, have one hash.
#[test]
fn the_hash_is_of_the_files() {
    let t = tempfile::tempdir().unwrap();
    let cache = t.path().join("cache");
    let rooted = archive(&[
        ("drives-2.1.0/", ""),
        ("drives-2.1.0/config.toml", CONFIG),
        ("drives-2.1.0/src/", ""),
        (
            "drives-2.1.0/src/Axis.st",
            "FUNCTION Axis : INT END_FUNCTION",
        ),
    ]);
    let flat = archive(&[
        ("src/Axis.st", "FUNCTION Axis : INT END_FUNCTION"),
        ("config.toml", CONFIG),
    ]);
    assert_ne!(rooted, flat);
    let first = fetch(&cache, &served(t.path(), "rooted.tar.gz", &rooted)).unwrap();
    let second = fetch(&cache, &served(t.path(), "flat.tar.gz", &flat)).unwrap();
    assert_eq!(first.name, "drives");
    assert_eq!(first.hash, second.hash);
    assert_eq!(first.dir, second.dir);
    assert_eq!(
        std::fs::read_to_string(first.dir.join("src/Axis.st")).unwrap(),
        "FUNCTION Axis : INT END_FUNCTION"
    );
    // Each URL that gave it is one the cache answers for.
    let urls = std::fs::read_to_string(cached_urls(&cache, &first.hash)).unwrap();
    assert_eq!(urls.lines().count(), 2, "{urls}");

    // Another content is another hash.
    let changed = archive(&[
        ("config.toml", CONFIG),
        ("src/Axis.st", "FUNCTION Axis : DINT END_FUNCTION"),
    ]);
    let third = fetch(&cache, &served(t.path(), "changed.tar.gz", &changed)).unwrap();
    assert_ne!(third.hash, first.hash);
}

/// A library lists the files that make it. What it does not list is
/// not kept and not hashed, and `config.toml` is always part of it.
#[test]
fn a_library_is_made_of_the_paths_it_lists() {
    let t = tempfile::tempdir().unwrap();
    let cache = t.path().join("cache");
    let config = format!("{CONFIG}paths = [\"src\", \"LICENSE\"]\n");
    let with_tests = archive(&[
        ("config.toml", &config),
        ("LICENSE", "MIT"),
        ("src/Axis.st", "one"),
        ("tests/Axis.st", "tested"),
        ("rk_build/debug/core.wasm", "built"),
    ]);
    let without = archive(&[
        ("config.toml", &config),
        ("LICENSE", "MIT"),
        ("src/Axis.st", "one"),
    ]);
    let first = fetch(&cache, &served(t.path(), "with.tar.gz", &with_tests)).unwrap();
    let second = fetch(&cache, &served(t.path(), "without.tar.gz", &without)).unwrap();
    assert_eq!(first.hash, second.hash);
    assert_eq!(
        package_files(&first.dir),
        ["LICENSE", "config.toml", "src/Axis.st"]
    );
}

/// An archive is fetched over HTTP as it is from a file, and its kind
/// is the one the server says.
#[test]
fn a_library_is_fetched_over_http() {
    let bytes = archive(&[("config.toml", CONFIG), ("Axis.st", "one")]);
    let (address, server) = serve(vec![Answer {
        content_type: "application/gzip",
        body: bytes.clone(),
    }]);

    let t = tempfile::tempdir().unwrap();
    let over_http = fetch(&t.path().join("cache"), &format!("{address}/download")).unwrap();
    let asked = server.join().unwrap();
    assert_eq!(asked[0].0, "GET /download HTTP/1.1");
    let from_a_file = fetch(
        &t.path().join("other"),
        &served(t.path(), "drives.tar.gz", &bytes),
    )
    .unwrap();
    assert_eq!(over_http.name, "drives");
    assert_eq!(over_http.hash, from_a_file.hash);
}

/// What is not a library, or would not be the same on every system, is
/// refused, each in its own words, and leaves nothing in the cache.
#[test]
fn an_archive_that_is_not_a_library_is_refused() {
    let t = tempfile::tempdir().unwrap();
    let cache = t.path().join("cache");
    let unpacking = |note: &str| format!("error: unable to unpack tarball\n    note: {note}\n");
    let cases: &[(&[(&str, &str)], String)] = &[
        (
            &[("Axis.st", "x")],
            "error: no config.toml at the root\n    note: a library is a folder with a config.toml\n"
                .to_string(),
        ),
        (
            &[("a/config.toml", CONFIG), ("b/Axis.st", "x")],
            "error: no config.toml at the root\n    note: a library is a folder with a config.toml\n"
                .to_string(),
        ),
        (
            &[("config.toml", CONFIG), ("../escape.st", "x")],
            unpacking("file '../escape.st' is outside the package"),
        ),
        (
            &[("config.toml", CONFIG), ("/etc/escape.st", "x")],
            unpacking("file '/etc/escape.st' is outside the package"),
        ),
        (
            &[("config.toml", CONFIG), ("dir\\Axis.st", "x")],
            unpacking("file 'dir\\Axis.st' has a name that not every system accepts"),
        ),
        (
            &[("config.toml", CONFIG), ("link:alias.st", "../outside.st")],
            unpacking("symlink 'alias.st' points outside the package, to '../outside.st'"),
        ),
        (
            &[("config.toml", CONFIG), ("Axis.st", "x"), ("axis.st", "y")],
            unpacking("unable to create file 'axis.st': PathAlreadyExists"),
        ),
        (
            &[("config.toml", CONFIG), ("Axis.st", "x"), ("Axis.st", "y")],
            unpacking("unable to create file 'Axis.st': PathAlreadyExists"),
        ),
    ];
    for (index, (entries, error)) in cases.iter().enumerate() {
        let url = served(t.path(), &format!("{index}.tar.gz"), &archive(entries));
        assert_eq!(
            fetch(&cache, &url).unwrap_err().to_string(),
            *error,
            "{entries:?}"
        );
    }
    let message = |url: &str| fetch(&cache, url).unwrap_err().message;
    let invalid = served(
        t.path(),
        "invalid.tar.gz",
        &archive(&[("config.toml", "not toml")]),
    );
    assert!(message(&invalid).starts_with("invalid config.toml: "));
    let plain = served(t.path(), "plain.txt", b"not an archive");
    assert!(message(&plain).starts_with("unknown file type: '"));
    assert!(message(&plain).ends_with("/plain.txt'"));
    let zip = served(t.path(), "drives.zip", b"PK");
    assert_eq!(message(&zip), "unable to unpack: rk reads no .zip");
    assert_eq!(
        message("ftp://example.org/x.tar.gz"),
        "unsupported URL scheme: ftp"
    );
    assert!(message("drives.tar.gz").starts_with("invalid URL 'drives.tar.gz': "));
    assert!(!packages_dir(&cache).exists());
}

/// The `Content-Type` comes first, then the `Content-Disposition`, then
/// the path of the URL.
#[test]
fn the_kind_of_a_response() {
    let from = FileType::from_response;
    assert_eq!(
        from(Some("application/x-gzip; charset=binary"), None, "/x"),
        Ok(FileType::TarGz)
    );
    assert_eq!(
        from(Some("application/zip"), None, "/x.tar.gz"),
        Ok(FileType::Zip)
    );
    assert_eq!(
        from(
            Some("application/octet-stream"),
            Some("attachment; filename=lib-1.0.tar.gz"),
            "/download"
        ),
        Ok(FileType::TarGz)
    );
    assert_eq!(
        from(Some("application/octet-stream"), None, "/lib-1.0.tgz"),
        Ok(FileType::TarGz)
    );
    let message = |result: Result<FileType, FetchError>| result.unwrap_err().message;
    assert_eq!(
        message(from(None, None, "/lib.tar.gz")),
        "missing 'Content-Type' header"
    );
    assert_eq!(
        message(from(Some("text/html"), None, "/lib.tar.gz")),
        "unrecognized 'Content-Type' header: 'text/html'"
    );
    assert_eq!(
        message(from(Some("application/octet-stream"), None, "/download")),
        "unknown file type: '/download'"
    );
}

/// A link may point anywhere inside the package, and nowhere else.
#[test]
fn a_link_stays_inside_the_package() {
    for (path, link) in [
        ("hello_ln", "hello"),
        ("a/b/link", "../c/file"),
        ("a/b/link", "../../file"),
        ("a/link", "./b/../c"),
    ] {
        assert!(stays_inside(path, link).is_ok(), "{path} -> {link}");
    }
    for (path, link) in [
        ("link", "../file"),
        ("a/b/link", "../../../file"),
        ("link", "/etc/passwd"),
        ("link", "C:\\file"),
    ] {
        assert!(stays_inside(path, link).is_err(), "{path} -> {link}");
    }
}

/// A name that differs from another only by case is the same file where
/// the file system says so: it is the same error everywhere.
#[test]
fn a_name_that_differs_only_by_case_is_taken() {
    let mut tree = Tree::default();
    tree.insert("src/Axis.st".to_string(), Entry::File(b"one".to_vec()));
    tree.insert("src/axis.st".to_string(), Entry::File(b"two".to_vec()));
    assert_eq!(tree.entries.len(), 1);
    assert_eq!(
        tree.errors,
        [UnpackError::CreateFile {
            name: "src/axis.st".to_string(),
            code: "PathAlreadyExists".to_string(),
        }]
    );
}

/// A file is a program when its first bytes say so: the `#!` of a script,
/// or the header of an ELF or a Mach-O executable, in either byte order.
/// The bytes may come in several pieces.
#[test]
fn a_program_is_told_by_its_first_bytes() {
    let mut h = FileHeader::default();
    assert!(!h.is_executable());

    let elf_magic = b"\x7fELF";
    h.update(&elf_magic[0..2]);
    assert!(!h.is_executable());
    h.update(&elf_magic[2..4]);
    assert!(h.is_executable());
    h.update(&elf_magic[2..4]);
    assert!(h.is_executable());

    let mut h = FileHeader::default();
    h.update(&[0xCF, 0xFA, 0xED, 0xFE]);
    assert!(h.is_executable());
    let mut h = FileHeader::default();
    h.update(&[0xFE, 0xED, 0xFA, 0xCF]);
    assert!(h.is_executable());

    assert!(FileHeader::of(b"#!/bin/sh\n").is_executable());
    assert!(!FileHeader::of(b"#").is_executable());
    assert!(!FileHeader::of(b"FUNCTION f").is_executable());
}

/// A library includes the folders it lists, and what they hold at any
/// depth.
#[test]
fn a_library_includes_the_folders_it_lists() {
    let filter = Filter::new(["src"]);
    assert!(filter.includes("src/core/unix/poll.st"));
    assert!(!filter.includes(".gitignore"));
    assert!(!filter.includes("srcs/a.st"));
    assert!(Filter::default().includes("anything"));
    assert!(Filter::new([""]).includes("anything"));
}

/// Of four errors, the two about a folder the library leaves out do not
/// count.
#[test]
fn only_the_errors_about_what_the_library_includes_count() {
    let mut res = Tree {
        root_error_message: "unable to unpack",
        ..Tree::default()
    };
    res.errors.push(UnpackError::CreateFile {
        name: "dir1/file1".to_string(),
        code: "File1".to_string(),
    });
    res.errors.push(UnpackError::CreateSymlink {
        name: "dir2/file2".to_string(),
        link: "filename".to_string(),
        code: "SymlinkError".to_string(),
    });
    res.errors.push(UnpackError::CreateFile {
        name: "dir1/file3".to_string(),
        code: "File3".to_string(),
    });
    res.errors.push(UnpackError::UnsupportedType {
        name: "dir2/file4".to_string(),
        kind: 'x',
    });
    let error = res.validate(&Filter::new(["dir2"])).unwrap_err();
    assert_eq!(
        error.to_string(),
        "error: unable to unpack\n\
         \x20   note: unable to create symlink from 'dir2/file2' to 'filename': SymlinkError\n\
         \x20   note: file 'dir2/file4' has unsupported type 'x'\n"
    );
    assert_eq!(res.validate(&Filter::new(["src"])), Ok(()));
}

/// The kind of an attachment is read from its file name, whatever the case
/// of the header, quoted or not, encoded or not.
#[test]
fn the_kind_of_an_attachment_is_read_from_its_file_name() {
    let from = FileType::from_content_disposition;
    assert_eq!(
        from("attaChment; FILENAME=\"stuff.tar.gz\"; size=42"),
        Some(FileType::TarGz)
    );
    assert_eq!(
        from("attachment; filename*=\"stuff.tar.gz\""),
        Some(FileType::TarGz)
    );
    assert_eq!(
        from("ATTACHMENT; filename=\"stuff.tar.xz\""),
        Some(FileType::TarXz)
    );
    assert_eq!(
        from("attachment; FileName=\"stuff.tar.xz\""),
        Some(FileType::TarXz)
    );
    assert_eq!(
        from("attachment; FileName*=UTF-8''xyz%2Fstuff.tar.gz"),
        Some(FileType::TarGz)
    );
    assert_eq!(
        from("attachment; FileName=\"stuff.tar\""),
        Some(FileType::Tar)
    );

    assert_eq!(from("attachment FileName=\"stuff.tar.gz\""), None);
    assert_eq!(from("attachment; FileName\"stuff.gz\""), None);
    assert_eq!(from("attachment; size=42"), None);
    assert_eq!(from("inline; size=42"), None);
    assert_eq!(from("FileName=\"stuff.tar.gz\"; attachment;"), None);
    assert_eq!(from("FileName=\"stuff.tar.gz\";"), None);
}

/// The same path twice in an archive is a file that cannot be made a
/// second time.
#[test]
fn an_archive_with_a_file_twice_is_refused() {
    let t = tempfile::tempdir().unwrap();
    let url = served(
        t.path(),
        "drives.tar.gz",
        &archive(&[
            ("drives-1.0/", ""),
            ("drives-1.0/config.toml", CONFIG),
            ("drives-1.0/dir1/file1", "one"),
            ("drives-1.0/dir1/file1", "two"),
            ("drives-1.0/src/main.st", "FUNCTION main : INT END_FUNCTION"),
        ]),
    );
    let error = fetch(&t.path().join("cache"), &url).unwrap_err();
    assert_eq!(
        error.to_string(),
        "error: unable to unpack tarball\n\
         \x20   note: unable to create file 'dir1/file1': PathAlreadyExists\n"
    );
}

/// The same file twice, in a folder the library leaves out, is no error,
/// and that folder is not kept.
#[test]
fn a_file_twice_where_the_library_does_not_look_is_ignored() {
    let t = tempfile::tempdir().unwrap();
    let config = format!("{CONFIG}paths = [\"src\"]\n");
    let main = "FUNCTION main : INT END_FUNCTION";
    let root = "FUNCTION root : INT END_FUNCTION";
    let url = served(
        t.path(),
        "drives.tar.gz",
        &archive(&[
            ("drives-1.0/", ""),
            ("drives-1.0/config.toml", &config),
            ("drives-1.0/dir1/file1", "one"),
            ("drives-1.0/dir1/file1", "two"),
            ("drives-1.0/src/main.st", main),
            ("drives-1.0/src/root.st", root),
        ]),
    );
    let fetched = fetch(&t.path().join("cache"), &url).unwrap();
    assert_eq!(
        package_files(&fetched.dir),
        ["config.toml", "src/main.st", "src/root.st"]
    );
    assert_eq!(
        fetched.hash.to_string(),
        expected_hash(&[
            ("config.toml", Expected::File(config.as_bytes())),
            ("src/main.st", Expected::File(main.as_bytes())),
            ("src/root.st", Expected::File(root.as_bytes())),
        ])
    );
}

/// An archive with its files at the top has no folder to leave out.
#[test]
fn an_archive_without_a_root_folder() {
    let t = tempfile::tempdir().unwrap();
    let config = format!("{CONFIG}paths = [\"src\"]\n");
    let main = "FUNCTION main : INT END_FUNCTION";
    let url = served(
        t.path(),
        "drives.tar.gz",
        &archive(&[
            ("config.toml", &config),
            ("dir1/", ""),
            ("dir1/file2", ""),
            ("dir1/file1", ""),
            ("dir2/", ""),
            ("dir2/file2", ""),
            ("src/", ""),
            ("src/main.st", main),
        ]),
    );
    let fetched = fetch(&t.path().join("cache"), &url).unwrap();
    assert_eq!(package_files(&fetched.dir), ["config.toml", "src/main.st"]);
    assert_eq!(
        fetched.hash.to_string(),
        expected_hash(&[
            ("config.toml", Expected::File(config.as_bytes())),
            ("src/main.st", Expected::File(main.as_bytes())),
        ])
    );
}

/// The permissions an archive gives a file are not read: a file is a
/// program when its first bytes say so. A link is kept, and hashed by what
/// it points at.
#[cfg(unix)]
#[test]
fn a_program_is_marked_executable_by_its_first_bytes() {
    use std::os::unix::fs::PermissionsExt;
    let t = tempfile::tempdir().unwrap();
    let hello = "\x7fELF\x02\x01\x01 a program";
    let script = "#!/bin/sh\necho hi\n";
    let url = served(
        t.path(),
        "tools.tar.gz",
        &archive(&[
            ("tools/", ""),
            ("tools/config.toml", CONFIG),
            ("x:tools/hello", hello),
            ("link:tools/hello_ln", "hello"),
            ("tools/file1", ""),
            ("tools/script_with_shebang_without_exec_bit", script),
            ("x:tools/script_without_shebang", "echo hi"),
            ("x:tools/script", script),
        ]),
    );
    let fetched = fetch(&t.path().join("cache"), &url).unwrap();
    assert_eq!(
        fetched.hash.to_string(),
        expected_hash(&[
            ("config.toml", Expected::File(CONFIG.as_bytes())),
            ("hello", Expected::File(hello.as_bytes())),
            ("hello_ln", Expected::Link("hello")),
            ("file1", Expected::File(b"")),
            (
                "script_with_shebang_without_exec_bit",
                Expected::File(script.as_bytes()),
            ),
            ("script_without_shebang", Expected::File(b"echo hi")),
            ("script", Expected::File(script.as_bytes())),
        ])
    );

    let executable = |name: &str| {
        let mode = std::fs::metadata(fetched.dir.join(name))
            .unwrap()
            .permissions()
            .mode();
        mode & 0o100 != 0
    };
    assert!(!executable("file1"));
    assert!(!executable("script_without_shebang"));
    assert!(executable("hello"));
    assert!(executable("script"));
    assert!(executable("script_with_shebang_without_exec_bit"));
    assert!(executable("hello_ln"));
    assert_eq!(
        std::fs::read_link(fetched.dir.join("hello_ln")).unwrap(),
        std::path::Path::new("hello")
    );
}
