// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! A library fetched from a git repository: the protocol against a server
//! that answers as a git server does, and against git itself.

use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};

use db::dependencies::{ContentHash, Remote};
use fetch::archive::FileType;
use fetch::git::{Error, Format, Oid, Repository, Session, resolve};

use super::{Answer, package_files, packet, plain, serve};

fn git(dir: &Path, args: &[&str]) -> Vec<u8> {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git is installed");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

fn git_text(dir: &Path, args: &[&str]) -> String {
    String::from_utf8(git(dir, args))
        .unwrap()
        .trim()
        .to_string()
}

/// A library in a repository of the object format `format`: two files
/// alike enough for a delta, a binary file, a program, a link, a few
/// revisions, a tag of each kind, a branch, and a commit with a
/// submodule. Answers the bare copy a server serves.
fn repository(root: &Path, format: &str) -> std::path::PathBuf {
    let root = plain(root);
    let work = root.join(format!("drives-{format}"));
    std::fs::create_dir_all(work.join("src/motion")).unwrap();
    std::fs::create_dir_all(work.join("src/io")).unwrap();
    git(
        &work,
        &[
            "init",
            "-q",
            &format!("--object-format={format}"),
            "-b",
            "main",
        ],
    );
    git(&work, &["config", "user.email", "test@example.org"]);
    git(&work, &["config", "user.name", "test"]);
    git(&work, &["config", "core.autocrlf", "false"]);

    let write = |path: &str, content: &[u8]| std::fs::write(work.join(path), content).unwrap();
    write(
        "config.toml",
        b"[project]\nname = \"drives\"\nversion = \"1.0.0\"\n",
    );
    let axis: String = (0..2000)
        .map(|i| format!("    // line {i} of the axis, {}\n", i * 7919 % 104729))
        .collect();
    write("src/motion/Axis.st", axis.as_bytes());
    write(
        "src/io/AxisCopy.st",
        format!("{axis}// a change\n").as_bytes(),
    );
    write("src/io/blob.bin", b"\x00\x01\x02binary\xff");
    write("build.sh", b"#!/bin/sh\necho build\n");
    // A link, where the system has them: in the work tree, so that each
    // commit after this one keeps it.
    #[cfg(unix)]
    std::os::unix::fs::symlink("motion/Axis.st", work.join("src/link.st")).unwrap();
    git(&work, &["add", "-A"]);
    git(&work, &["update-index", "--chmod=+x", "build.sh"]);
    git(&work, &["commit", "-qm", "one"]);
    git(&work, &["tag", "v1"]);
    for revision in 1..=3 {
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(work.join("src/motion/Axis.st"))
            .unwrap();
        writeln!(file, "// revision {revision}").unwrap();
        git(&work, &["commit", "-qam", &format!("revision {revision}")]);
    }
    git(&work, &["tag", "-a", "v2", "-m", "version 2"]);
    git(&work, &["checkout", "-qb", "dev"]);
    write("dev.txt", b"dev\n");
    git(&work, &["add", "dev.txt"]);
    git(&work, &["commit", "-qm", "dev"]);
    git(&work, &["checkout", "-q", "main"]);
    let earlier = git_text(&work, &["rev-parse", "HEAD~1"]);
    git(
        &work,
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("160000,{earlier},vendor/sub"),
        ],
    );
    git(&work, &["commit", "-qm", "submodule"]);
    git(&work, &["tag", "v3"]);

    let bare = root.join(format!("drives-{format}.git"));
    git(
        &root,
        &[
            "clone",
            "-q",
            "--bare",
            work.to_str().unwrap(),
            bare.to_str().unwrap(),
        ],
    );
    bare
}

/// The pack git makes of everything `revisions` reach, its deltas written
/// against an offset in the pack with `by_offset`, and against an object ID
/// otherwise.
fn pack_of(bare: &Path, revisions: &[&str], by_offset: bool) -> Vec<u8> {
    let mut args = vec!["pack-objects", "--revs", "--stdout", "-q"];
    if by_offset {
        args.push("--delta-base-offset");
    }
    let mut child = Command::new("git")
        .args(&args)
        .current_dir(bare)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    for revision in revisions {
        writeln!(stdin, "{revision}").unwrap();
    }
    drop(stdin);
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    output.stdout
}

/// The files of `commit` as `git archive` gives them, read the way rk reads
/// an archive.
fn archived(bare: &Path, commit: &str) -> std::collections::BTreeMap<String, fetch::tree::Entry> {
    let tar = git(bare, &["archive", "--format=tar", commit]);
    fetch::archive::unpack(FileType::Tar, &tar).unwrap().entries
}

/// A pack made by git, its deltas by offset or by ID, gives each commit
/// the files `git archive` gives it, in both hash formats.
#[test]
fn a_pack_made_by_git_holds_the_files_of_each_commit() {
    let t = tempfile::tempdir().unwrap();
    let root = t.path().canonicalize().unwrap();
    for (format, name) in [(Format::Sha1, "sha1"), (Format::Sha256, "sha256")] {
        let bare = repository(&root, name);
        for by_offset in [true, false] {
            let pack = pack_of(&bare, &["v3", "dev"], by_offset);
            let mut repository = Repository::new(format, pack).unwrap();
            for revision in ["v1", "v2", "dev", "v3"] {
                let commit = git_text(&bare, &["rev-parse", &format!("{revision}^{{commit}}")]);
                let tree = repository
                    .checkout(&Oid::parse(format, &commit).unwrap())
                    .unwrap();
                assert!(
                    tree.errors.is_empty(),
                    "{name} {revision}: {:?}",
                    tree.errors
                );
                assert_eq!(
                    tree.entries,
                    archived(&bare, &commit),
                    "{name} {revision}, deltas by offset: {by_offset}"
                );
            }
        }
    }
}

/// What a git server answers a fetch of `commit` with: its capabilities,
/// an annotated tag `v1.0` that points at the commit, then the pack.
fn git_answers(commit: &str, pack: &[u8], with_refs: bool) -> Vec<Answer> {
    let mut capabilities = Vec::new();
    // As some servers do, a line about the service comes first.
    capabilities.extend(packet(b"# service=git-upload-pack\n"));
    capabilities.extend(b"0000");
    for line in [
        "version 2\n",
        "agent=git/test\n",
        "ls-refs\n",
        "fetch=shallow\n",
        "object-format=sha1\n",
    ] {
        capabilities.extend(packet(line.as_bytes()));
    }
    capabilities.extend(b"0000");

    let mut refs = Vec::new();
    // An annotated tag: the tag object, and the commit it points at.
    let tag = "1111111111111111111111111111111111111111";
    refs.extend(packet(
        format!("{tag} refs/tags/v1.0 peeled:{commit}\n").as_bytes(),
    ));
    refs.extend(b"0000");

    let mut body = Vec::new();
    body.extend(packet(b"shallow-info\n"));
    body.extend(packet(format!("shallow {commit}\n").as_bytes()));
    body.extend(b"0001");
    body.extend(packet(b"packfile\n"));
    body.extend(packet(b"\x02Counting objects"));
    for chunk in pack.chunks(1000) {
        let mut data = vec![1];
        data.extend_from_slice(chunk);
        body.extend(packet(&data));
    }
    body.extend(b"0000");

    let answer = |content_type, body| Answer { content_type, body };
    let mut answers = vec![answer(
        "application/x-git-upload-pack-advertisement",
        capabilities,
    )];
    if with_refs {
        answers.push(answer("application/x-git-upload-pack-result", refs));
    }
    answers.push(answer("application/x-git-upload-pack-result", body));
    answers
}

/// A tag of a git repository is resolved to its commit, the commit is
/// fetched without its history, and the URL to write down names it,
/// with the tag kept beside it. Each request says what it should.
#[test]
fn a_tag_of_a_git_repository_is_fetched() {
    let t = tempfile::tempdir().unwrap();
    let root = t.path().canonicalize().unwrap();
    let bare = repository(&root, "sha1");
    let commit = git_text(&bare, &["rev-parse", "v2^{commit}"]);
    let pack = pack_of(&bare, &[&commit], true);

    let (address, server) = serve(git_answers(&commit, &pack, true));
    let url = format!("git+{address}/acme/drives#v1.0");
    let fetched = fetch::fetch(&root.join("cache"), &url).unwrap();
    let asked = server.join().unwrap();

    assert_eq!(fetched.name, "drives");
    assert_eq!(
        fetched.url,
        format!("git+{address}/acme/drives?ref=v1.0#{commit}")
    );
    assert!(package_files(&fetched.dir).contains(&"src/motion/Axis.st".to_string()));

    let line = |index: usize| asked[index].0.as_str();
    let body = |index: usize| String::from_utf8_lossy(&asked[index].1).into_owned();
    assert_eq!(
        line(0),
        "GET /acme/drives/info/refs?service=git-upload-pack HTTP/1.1"
    );
    assert_eq!(line(1), "POST /acme/drives/git-upload-pack HTTP/1.1");
    assert!(body(1).contains("command=ls-refs\n"), "{}", body(1));
    assert!(
        body(1).contains("ref-prefix refs/tags/v1.0\n"),
        "{}",
        body(1)
    );
    assert!(body(1).contains("peel\n"), "{}", body(1));
    assert!(body(2).contains("command=fetch\n"), "{}", body(2));
    assert!(body(2).contains("deepen 1\n"), "{}", body(2));
    assert!(body(2).contains(&format!("want {commit}\n")), "{}", body(2));

    // The same files, fetched by their commit as a dependency names them:
    // no ref to ask the server for, and the hash already written.
    let (address, server) = serve(git_answers(&commit, &pack, false));
    let pinned = format!("git+{address}/acme/drives#{commit}");
    let remote = Remote {
        name: "drives",
        url: &pinned,
        hash: &fetched.hash,
    };
    fetch::fetch_remote(&root.join("other"), &remote).unwrap();
    assert_eq!(server.join().unwrap().len(), 2);
}

/// A dependency names its commit, or at least a ref: a URL with
/// neither would name another library tomorrow.
#[test]
fn a_git_dependency_without_a_ref_is_refused() {
    let t = tempfile::tempdir().unwrap();
    let root = t.path().canonicalize().unwrap();
    let bare = repository(&root, "sha1");
    let head = git_text(&bare, &["rev-parse", "HEAD"]);
    let mut answers = git_answers(&head, &[], true);
    answers.truncate(2);
    answers[1].body = packet(format!("{head} HEAD\n").as_bytes());
    answers[1].body.extend(b"0000");
    let (address, server) = serve(answers);
    let url = format!("git+{address}/acme/drives");
    let hash = ContentHash::from_digest(&[0; 32]);
    let remote = Remote {
        name: "drives",
        url: &url,
        hash: &hash,
    };
    let error = fetch::fetch_remote(&root.join("cache"), &remote).unwrap_err();
    server.join().unwrap();
    assert_eq!(
        error.to_string(),
        format!(
            "error: the url names no commit after its `#`\n\
             \x20   note: try url = \"{url}#{head}\"\n"
        )
    );
}

/// A pack cut short, or one that is not a pack, is an error and not a
/// panic.
#[test]
fn a_pack_that_is_not_one_is_refused() {
    let t = tempfile::tempdir().unwrap();
    let root = t.path().canonicalize().unwrap();
    let pack = pack_of(&repository(&root, "sha1"), &["v3"], true);
    let new = |pack: &[u8]| Repository::new(Format::Sha1, pack.to_vec()).err();
    assert_eq!(new(b"PAC"), Some(Error::InvalidHeader));
    assert_eq!(new(b"JUNKJUNKJUNK"), Some(Error::InvalidHeader));
    assert_eq!(
        new(b"PACK\0\0\0\x03\0\0\0\0"),
        Some(Error::UnsupportedVersion)
    );
    for cut in [13, 100, pack.len() / 2] {
        assert!(new(&pack[..cut]).is_some(), "{cut}");
    }
    let mut repository = Repository::new(Format::Sha1, pack).unwrap();
    let unknown = Oid::parse(Format::Sha1, &"ab".repeat(20)).unwrap();
    assert_eq!(
        repository.checkout(&unknown).err(),
        Some(Error::ObjectNotFound)
    );
}

/// Against a real server, which a test run does not reach: run it by
/// hand with `cargo nextest run -p rk-tests --run-ignored only -E 'test(a_real_server)'`.
#[test]
#[ignore = "needs the network"]
fn a_real_server_is_fetched_from() {
    let session = Session::init("https://github.com/octocat/Hello-World").unwrap();
    assert_eq!(session.object_format, Format::Sha1);
    let commit = resolve(&session, "master").unwrap();
    let pack = session.fetch(&[&commit.to_string()]).unwrap();
    let mut repository = Repository::new(commit.format(), pack).unwrap();
    let worktree = repository.checkout(&commit).unwrap();
    assert!(
        worktree.entries.contains_key("README"),
        "{:?}",
        worktree.entries.keys()
    );
    assert!(resolve(&session, "no-such-branch").is_err());
}

/// `git http-backend` behind an HTTP server of a few lines: each request is
/// handed to it as a CGI program is, with the protocol version the client
/// asked for. Answers the server's address.
fn serve_git(project_root: &Path) -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    let root = project_root.to_path_buf();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let mut stream = stream.unwrap();
            let mut head = Vec::new();
            let mut byte = [0];
            while !head.ends_with(b"\r\n\r\n") && stream.read(&mut byte).unwrap() == 1 {
                head.push(byte[0]);
            }
            let head = String::from_utf8_lossy(&head).into_owned();
            let mut lines = head.lines();
            let mut request = lines.next().unwrap_or_default().split(' ');
            let method = request.next().unwrap_or_default().to_string();
            let target = request.next().unwrap_or_default().to_string();
            let header = |wanted: &str| {
                head.lines().find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case(wanted)
                        .then(|| value.trim().to_string())
                })
            };
            let length: usize = header("content-length").map_or(0, |v| v.parse().unwrap());
            let mut body = vec![0; length];
            stream.read_exact(&mut body).unwrap();

            let (path, query) = target.split_once('?').unwrap_or((&target, ""));
            let mut backend = Command::new("git");
            backend
                .arg("http-backend")
                .env("GIT_PROJECT_ROOT", plain(&root))
                .env("GIT_HTTP_EXPORT_ALL", "1")
                .env("PATH_INFO", path)
                .env("QUERY_STRING", query)
                .env("REQUEST_METHOD", &method)
                .env("REMOTE_ADDR", "127.0.0.1")
                .env("CONTENT_TYPE", header("content-type").unwrap_or_default())
                .env("CONTENT_LENGTH", length.to_string())
                .stdin(Stdio::piped())
                .stdout(Stdio::piped());
            if let Some(protocol) = header("git-protocol") {
                backend.env("GIT_PROTOCOL", protocol);
            }
            let mut child = backend.spawn().unwrap();
            child.stdin.take().unwrap().write_all(&body).unwrap();
            let output = child.wait_with_output().unwrap().stdout;

            let split = output
                .windows(4)
                .position(|w| w == b"\r\n\r\n")
                .expect("the backend's headers");
            let cgi_head = String::from_utf8_lossy(&output[..split]).into_owned();
            let payload = &output[split + 4..];
            let mut status = "200 OK".to_string();
            let mut headers = String::new();
            for line in cgi_head.lines() {
                match line.split_once(": ") {
                    Some((name, value)) if name.eq_ignore_ascii_case("status") => {
                        status = value.to_string()
                    }
                    Some((name, value)) => headers.push_str(&format!("{name}: {value}\r\n")),
                    None => {}
                }
            }
            let _ = write!(
                stream,
                "HTTP/1.1 {status}\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n",
                payload.len()
            );
            let _ = stream.write_all(payload);
        }
    });
    address
}

/// The same commit, as the archive git makes of it: what the protocol has
/// to give, file for file and hash for hash.
fn archive_of(bare: &Path, commit: &str, cache: &Path, dir: &Path) -> fetch::Fetched {
    let path = dir.join(format!("{commit}.tar.gz"));
    std::fs::write(&path, git(bare, &["archive", "--format=tar.gz", commit])).unwrap();
    let url = auto_lsp::lsp_types::Url::from_file_path(&path).unwrap();
    fetch::fetch(cache, url.as_str()).unwrap()
}

#[test]
fn a_commit_fetched_from_git_is_its_git_archive() {
    let t = tempfile::tempdir().unwrap();
    let root = t.path().canonicalize().unwrap();
    let address = serve_git(&root);
    let mut hashes = Vec::new();
    for format in ["sha1", "sha256"] {
        let bare = repository(&root, format);
        let repo = format!("git+{address}/drives-{format}.git");
        let commit = |name: &str| git_text(&bare, &["rev-parse", &format!("{name}^{{commit}}")]);
        // What follows the `#`, the commit it names, and the URL saved.
        let v1 = commit("v1");
        let cases = [
            ("#v1", commit("v1"), "?ref=v1#"),
            ("#v2", commit("v2"), "?ref=v2#"),
            ("#dev", commit("dev"), "?ref=dev#"),
            ("#v3", commit("v3"), "?ref=v3#"),
            (&format!("#{v1}") as &str, v1.clone(), "#"),
            ("", commit("HEAD"), "#"),
        ];
        for (fragment, commit, saved) in &cases {
            let fetched = fetch::fetch(&root.join("cache"), &format!("{repo}{fragment}"))
                .unwrap_or_else(|error| panic!("{format} {fragment}: {error}"));
            assert_eq!(fetched.name, "drives");
            assert_eq!(
                fetched.url,
                format!("{repo}{saved}{commit}"),
                "{format} {fragment}"
            );
            let archive = archive_of(&bare, commit, &root.join("archives"), &root);
            assert_eq!(fetched.hash, archive.hash, "{format} {fragment}");
            hashes.push((fragment.trim_start_matches('#').len() == 40, fetched.hash));

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mode = |path: &str| {
                    std::fs::metadata(fetched.dir.join(path))
                        .unwrap()
                        .permissions()
                        .mode()
                };
                assert_ne!(mode("build.sh") & 0o100, 0, "the program runs");
                assert_eq!(mode("config.toml") & 0o300, 0, "the cache is not written");
                assert_eq!(
                    std::fs::read_link(fetched.dir.join("src/link.st")).unwrap(),
                    Path::new("motion/Axis.st")
                );
            }
        }
    }
    // The files are the same in both formats, and so is their hash: it
    // is computed from the files, not from git's IDs.
    let sha1 = &hashes[..6];
    let sha256 = &hashes[6..];
    for (one, other) in sha1.iter().zip(sha256) {
        assert_eq!(one.1, other.1);
    }
}
