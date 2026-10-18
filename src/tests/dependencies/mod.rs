// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! Dependencies: the libraries a workspace names in its `config.toml`,
//! how they are resolved, fetched over HTTP, from a file or from git, kept
//! in the global cache, and read with the workspace. And the standard
//! library, built into the compiler and kept in the same cache.

mod archives;
mod cache;
mod git;
mod platforms;
mod resolution;
mod stdlib;
mod workspace;

use std::io::{Read, Write};
use std::path::Path;

use auto_lsp::lsp_types::Url;

/// `path` without the `\\?\` that `canonicalize` puts before it on Windows:
/// git does not take one as an argument.
pub(super) fn plain(path: &Path) -> std::path::PathBuf {
    let text = path.to_string_lossy();
    match text.strip_prefix(r"\\?\UNC\") {
        Some(rest) => std::path::PathBuf::from(format!(r"\\{rest}")),
        None => std::path::PathBuf::from(text.strip_prefix(r"\\?\").unwrap_or(&text)),
    }
}

/// The URL of `bytes`, written as a file under `dir`.
pub(super) fn served(dir: &Path, name: &str, bytes: &[u8]) -> String {
    let path = dir.join(name);
    std::fs::write(&path, bytes).unwrap();
    Url::from_file_path(path.canonicalize().unwrap())
        .unwrap()
        .to_string()
}

/// The files of a fetched package, in the order of their paths.
pub(super) fn package_files(dir: &Path) -> Vec<String> {
    fn walk(root: &Path, dir: &Path, files: &mut Vec<String>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(root, &path, files);
            } else if path.is_file() && !path.is_symlink() {
                let relative = path.strip_prefix(root).unwrap();
                files.push(relative.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    let mut files = Vec::new();
    walk(dir, dir, &mut files);
    files.sort();
    files
}

/// A `.tar.gz` of `entries`, each a path and its content. A path that
/// ends with `/` is a folder, and one that starts with `link:` a link
/// to its content.
pub(super) fn archive(entries: &[(&str, &str)]) -> Vec<u8> {
    let gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    let mut tar = tar::Builder::new(gz);
    for (path, content) in entries {
        let mut header = tar::Header::new_gnu();
        // `x:` before a name gives the file the permissions of a program.
        let (path, mode) = match path.strip_prefix("x:") {
            Some(path) => (path, 0o755),
            None => (*path, 0o644),
        };
        header.set_mode(mode);
        if let Some(link) = path.strip_prefix("link:") {
            header.set_entry_type(tar::EntryType::Symlink);
            header.set_size(0);
            // The name and the target are written as they are, as
            // another tool would: the builder refuses a `..` itself.
            header.as_old_mut().name[..link.len()].copy_from_slice(link.as_bytes());
            header.as_old_mut().linkname[..content.len()].copy_from_slice(content.as_bytes());
            header.set_cksum();
            tar.append(&header, std::io::empty()).unwrap();
        } else if path.ends_with('/') {
            header.set_entry_type(tar::EntryType::Directory);
            header.set_size(0);
            tar.append_data(&mut header, path, std::io::empty())
                .unwrap();
        } else {
            header.set_size(content.len() as u64);
            header.as_old_mut().name[..path.len()].copy_from_slice(path.as_bytes());
            header.set_cksum();
            tar.append(&header, content.as_bytes()).unwrap();
        }
    }
    tar.into_inner().unwrap().finish().unwrap()
}

pub(super) const CONFIG: &str = "[project]\nname = \"drives\"\nversion = \"2.1.0\"\n";

/// One answer to one HTTP request.
pub(super) struct Answer {
    content_type: &'static str,
    body: Vec<u8>,
}

/// What one request asked: its first line and its body.
pub(super) type Asked = (String, Vec<u8>);

/// A server that gives `answers`, one per request, in their order. It
/// answers with what was asked of it: each request's first line and
/// its body.
pub(super) fn serve(answers: Vec<Answer>) -> (String, std::thread::JoinHandle<Vec<Asked>>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    let server = std::thread::spawn(move || {
        let mut asked = Vec::new();
        for answer in answers {
            let (mut stream, _) = listener.accept().unwrap();
            let mut head = Vec::new();
            let mut byte = [0];
            while !head.ends_with(b"\r\n\r\n") && stream.read(&mut byte).unwrap() == 1 {
                head.push(byte[0]);
            }
            let head = String::from_utf8_lossy(&head).into_owned();
            let length = head
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().unwrap())
                })
                .unwrap_or(0);
            let mut body = vec![0; length];
            stream.read_exact(&mut body).unwrap();
            asked.push((head.lines().next().unwrap_or_default().to_string(), body));
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                answer.content_type,
                answer.body.len()
            )
            .unwrap();
            stream.write_all(&answer.body).unwrap();
        }
        asked
    });
    (address, server)
}

pub(super) fn packet(data: &[u8]) -> Vec<u8> {
    let mut out = format!("{:04x}", data.len() + 4).into_bytes();
    out.extend_from_slice(data);
    out
}

/// An entry of a library, for [`expected_hash`].
pub(super) enum Expected<'a> {
    File(&'a [u8]),
    Link(&'a str),
}

/// The hash a library made of `entries` has, computed here apart from rk's
/// own code: each file as its path, two zero bytes and its content, each
/// link as its path and its target, and then those hashes in the order of
/// the paths.
pub(super) fn expected_hash(entries: &[(&str, Expected<'_>)]) -> String {
    use sha2::{Digest, Sha256};
    let mut sorted: Vec<_> = entries.iter().collect();
    sorted.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
    let mut all = Sha256::new();
    for (path, entry) in sorted {
        let mut one = Sha256::new();
        one.update(path.as_bytes());
        match entry {
            Expected::File(content) => {
                one.update([0, 0]);
                one.update(content);
            }
            Expected::Link(target) => one.update(target.as_bytes()),
        }
        all.update(one.finalize());
    }
    let digest: String = all
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    format!("sha256:{digest}")
}
