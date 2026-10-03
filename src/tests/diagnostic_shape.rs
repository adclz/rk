// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! A diagnostic is made one way, and this is where that is held to account.
//!
//! - **Header** — what a report says after its code is the diagnostic's
//!   title, the one `crates/doc/examples/<code>.md` gives it. The table in
//!   `ide_diagnostic::headers` is the only place a header is written.
//! - **Text** — a message is one clause, a note the rule it rests on, a help
//!   what to write instead. A text that needs a `;` is two of those.

use std::path::{Path, PathBuf};

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// The header of a title: lower case, unless it starts with a keyword
/// (`THIS`, `VAR_IN_OUT`) or a pragma (`{once}`).
fn header_of(title: &str) -> String {
    let mut chars = title.chars();
    match (chars.next(), chars.next()) {
        (Some(first), Some(second)) if first.is_uppercase() && second.is_lowercase() => {
            first.to_lowercase().chain(title.chars().skip(1)).collect()
        }
        _ => title.to_string(),
    }
}

/// `(code, title)` of every example, sorted by code.
fn titles() -> Vec<(String, String)> {
    let dir = root().join("crates/doc/examples");
    let mut out: Vec<(String, String)> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "md"))
        .map(|path| {
            let text = std::fs::read_to_string(&path).unwrap();
            let title = text
                .lines()
                .find_map(|line| line.strip_prefix("title:"))
                .unwrap_or_else(|| panic!("{}: no title", path.display()))
                .trim()
                .to_string();
            (
                path.file_stem().unwrap().to_string_lossy().into_owned(),
                title,
            )
        })
        .collect();
    out.sort();
    out
}

/// The header table is the titles, code for code: a diagnostic renamed on
/// the site and not in its reports, or the other way, fails here.
#[test]
fn a_header_is_its_title() {
    let expected: Vec<(String, String)> = titles()
        .into_iter()
        .map(|(code, title)| (code, header_of(&title)))
        .collect();
    let actual: Vec<(String, String)> = ide_diagnostic::headers::HEADERS
        .iter()
        .map(|(code, header)| (code.to_string(), header.to_string()))
        .collect();
    let drift: Vec<String> = expected
        .iter()
        .filter(|row| !actual.contains(row))
        .map(|(code, header)| format!("{code}: the title gives \"{header}\""))
        .chain(
            actual
                .iter()
                .filter(|(code, _)| !expected.iter().any(|(c, _)| c == code))
                .map(|(code, _)| format!("{code}: no crates/doc/examples/{code}.md")),
        )
        .collect();
    assert!(
        drift.is_empty(),
        "`ide_diagnostic::headers::HEADERS` and the titles disagree:\n{}",
        drift.join("\n")
    );
    assert!(
        actual.windows(2).all(|pair| pair[0].0 < pair[1].0),
        "HEADERS is searched by code: keep it sorted"
    );
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

/// Every string literal of `text` with its line, comments left out.
fn string_literals(text: &str) -> Vec<(usize, String)> {
    let bytes = text.as_bytes();
    let (mut i, mut line) = (0, 1);
    let mut out = Vec::new();
    while i < bytes.len() {
        match bytes[i] {
            b'\n' => line += 1,
            b'/' if bytes.get(i + 1) == Some(&b'/') => {
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
                continue;
            }
            // A char literal: `'"'` opens no string.
            b'\'' if bytes.get(i + 2) == Some(&b'\'') => i += 2,
            b'"' => {
                let (start, at) = (i + 1, line);
                i += 1;
                while i < bytes.len() && bytes[i] != b'"' {
                    if bytes[i] == b'\\' {
                        i += 1;
                    }
                    if bytes.get(i) == Some(&b'\n') {
                        line += 1;
                    }
                    i += 1;
                }
                out.push((at, text[start..i.min(text.len())].to_string()));
            }
            _ => {}
        }
        i += 1;
    }
    out
}

/// Texts that hold a `;` because they quote the language.
const QUOTES_THE_LANGUAGE: &[&str] = &[];

/// A message, a note and a help are each one clause. `fact; rule` is a
/// message and a note, `rule; advice` a note and a help.
#[test]
fn a_diagnostic_text_is_one_clause() {
    let mut files = Vec::new();
    for dir in ["crates/hir/src/check/errors", "crates/linter/src/rules"] {
        rust_files(&root().join(dir), &mut files);
    }
    assert!(!files.is_empty(), "found no sources to scan");
    let mut found = Vec::new();
    for file in &files {
        let text = std::fs::read_to_string(file).unwrap();
        let rel = file
            .strip_prefix(root())
            .unwrap_or(file)
            .display()
            .to_string();
        for (line, literal) in string_literals(&text) {
            let chained = literal.contains("; ") || literal.contains("can not");
            if chained && !QUOTES_THE_LANGUAGE.contains(&literal.as_str()) {
                found.push(format!("{rel}:{line}\n    {literal}"));
            }
        }
    }
    assert!(
        found.is_empty(),
        "a diagnostic text chains clauses with `;`, or writes `can not`, at {} site(s):\n\n{}\n\n\
         The message says what is wrong here, `with_note` the rule, `with_help` what to \
         write instead. A text that quotes the language goes in `QUOTES_THE_LANGUAGE`.",
        found.len(),
        found.join("\n")
    );
}
