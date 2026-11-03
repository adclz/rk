// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! A diagnostic is made one way, and this is where that is held to account.
//!
//! - **Header** — what a report says after its code is the diagnostic's
//!   title, the one `crates/doc/examples/<code>.md` gives it. The table in
//!   `ide_diagnostic::headers` is the only place a header is written.
//! - **Text** — a message is one clause, a note the rule it rests on, a help
//!   what to write instead. A text that needs a `;` is two of those.
//! - **Place** — a report points at the same text whichever files a program
//!   is written in. A span placed against another file's document does not,
//!   and no test of one file can see it.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use auto_lsp::lsp_types::{NumberOrString, Position, Range};
use db::{RootDatabase, WorkspaceDataBase};
use hir::check::check_file;
use hir::hir_ty::calls::CallGraph;

use crate::tests::utils::add_source;

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

/// What `range` covers in `source`, in the terms of the language server.
fn text_under(source: &str, range: Range) -> String {
    let offset = |position: Position| {
        let line_start: usize = source
            .split_inclusive('\n')
            .take(position.line as usize)
            .map(str::len)
            .sum();
        line_start + position.character as usize
    };
    source
        .get(offset(range.start)..offset(range.end))
        .unwrap_or("<outside the file>")
        .to_string()
}

/// A report as what it says and the text it says it of: its code, its
/// message, the text under its caret, and the same of each related label.
type Told = (String, String, String, Vec<(String, String)>);

fn told(sources: &[String]) -> BTreeSet<Told> {
    let mut db = RootDatabase::default();
    for source in sources {
        add_source(&mut db, source);
    }
    let files = db.get_files().to_vec();
    let graph = CallGraph::of_files(&db, files.iter().copied());
    let mut out = BTreeSet::new();
    for file in &files {
        let source = file.document(&db).as_str().to_string();
        for report in check_file(&db, &graph, *file) {
            let Some(NumberOrString::String(code)) = report.diagnostic.code.clone() else {
                panic!("a report with no code: {}", report.diagnostic.message);
            };
            let related = report.related().iter().map(|label| {
                let source = label.file.document(&db).as_str().to_string();
                let text = source.get(label.range.start_byte..label.range.end_byte);
                (
                    label.message.clone(),
                    text.unwrap_or("<outside the file>").to_string(),
                )
            });
            out.insert((
                code,
                report.diagnostic.message.clone(),
                text_under(&source, report.diagnostic.range),
                related.collect(),
            ));
        }
    }
    out
}

/// `source` cut after each top-level declaration. What stands before one, a
/// comment or a pragma, goes with it, and what follows the last with the last.
fn declarations(source: &str) -> Vec<String> {
    const CLOSED_BY: [(&str, &str); 9] = [
        ("FUNCTION_BLOCK", "END_FUNCTION_BLOCK"),
        ("FUNCTION", "END_FUNCTION"),
        ("PROGRAM", "END_PROGRAM"),
        ("TYPE", "END_TYPE"),
        ("CLASS", "END_CLASS"),
        ("INTERFACE", "END_INTERFACE"),
        ("CONFIGURATION", "END_CONFIGURATION"),
        ("VAR_GLOBAL", "END_VAR"),
        ("NAMESPACE", "END_NAMESPACE"),
    ];
    let mut parts: Vec<String> = vec![];
    let mut part = String::new();
    let mut open: Option<(&str, &str, i32)> = None;
    for line in source.lines() {
        part.push_str(line);
        part.push('\n');
        let mut words = line
            .split(|c: char| !c.is_alphanumeric() && c != '_')
            .filter(|word| !word.is_empty());
        if open.is_none() {
            let first = words.clone().next().unwrap_or_default();
            open = CLOSED_BY
                .iter()
                .find(|(opener, _)| *opener == first)
                .map(|(opener, closer)| (*opener, *closer, 0));
        }
        let Some((opener, closer, depth)) = &mut open else {
            continue;
        };
        for word in words.by_ref() {
            if word == *opener {
                *depth += 1;
            } else if word == *closer {
                *depth -= 1;
            }
        }
        if *depth <= 0 {
            parts.push(std::mem::take(&mut part));
            open = None;
        }
    }
    match parts.last_mut() {
        Some(last) => last.push_str(&part),
        None => parts.push(part),
    }
    parts
}

/// Every example, in one file and in one file per declaration, each with more
/// lines before it than the last: the reports must not move.
#[test]
fn a_report_points_at_the_same_text_across_files() {
    // Two declarations in two files each report, where one file reports once.
    const ONCE_A_FILE: [&str; 2] = ["E1302", "E1402"];

    let mut split = 0;
    let mut moved = vec![];
    for (code, _) in titles() {
        let path = root().join(format!("crates/doc/examples/{code}.md"));
        let text = std::fs::read_to_string(&path).unwrap();
        for fence in text.split("```iecst\n").skip(1) {
            let source = fence.split("\n```").next().unwrap();
            // A USING holds for its file.
            if source.contains("USING") {
                continue;
            }
            let parts = declarations(source);
            if parts.len() < 2 {
                continue;
            }
            split += 1;
            let padded: Vec<String> = parts
                .iter()
                .enumerate()
                .map(|(k, part)| "// padding\n".repeat(3 + 5 * k) + part)
                .collect();
            let whole = told(&[source.to_string()]);
            let apart = told(&padded);
            moved.extend(
                whole
                    .symmetric_difference(&apart)
                    .filter(|report| !ONCE_A_FILE.contains(&report.0.as_str()))
                    .map(|report| format!("{code}: {report:?}")),
            );
        }
    }
    // The cut still finds declarations: an example left whole tests nothing.
    assert!(split >= 100, "only {split} examples were split");
    assert!(moved.is_empty(), "{}", moved.join("\n"));
}
