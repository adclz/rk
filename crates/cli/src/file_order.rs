//! One deterministic order over the workspace's files.
//!
//! The database stores files in `DashMap`s, which is right for concurrent
//! access but gives no iteration order — and `DashMap` seeds a fresh
//! `RandomState` per process, so the order differs on every run. That is
//! invisible until the order reaches something ordered downstream, and it does:
//! the sequence handed to `lower_modules` becomes `all_pous` push order, which
//! becomes the wasm function index space. Six builds of the stdlib produced six
//! distinct artifacts, differing in about 80% of their bytes while behaving
//! identically.
//!
//! The fix is not to change how files are STORED — the maps stay exactly as
//! they are, concurrency and all — but to impose an order where one is needed,
//! at the point of iteration. Sorting a few hundred URLs once per build costs
//! nothing next to codegen.
//!
//! Anything that feeds compilation, or that emits per-file output a human or a
//! snapshot test will read, should take its files from here rather than
//! iterating the maps directly.

use auto_lsp::default::db::{BaseDatabase, file::File};
use db::{RootDatabase, WorkspaceDataBase};

/// The workspace's files in URL order.
///
/// Stable for a given set of files. Adding, removing or renaming a file
/// reorders what follows it — making the index space stable across renames is a
/// separate and much larger question, and is not what this solves.
pub fn ordered_files(db: &RootDatabase) -> Vec<File> {
    let mut files: Vec<File> = db.get_files().iter().map(|entry| *entry.value()).collect();
    sort_by_url(db, &mut files);
    files
}

/// The workspace's files followed by the stdlib's, each group in URL order.
///
/// The two groups are kept apart rather than merged and sorted as one, so that
/// adding a workspace file cannot renumber the stdlib's functions.
pub fn ordered_files_with_stdlib(db: &RootDatabase) -> Vec<File> {
    let mut files = ordered_files(db);
    let mut std_files: Vec<File> = db
        .get_std_lib_files()
        .iter()
        .map(|entry| *entry.value())
        .collect();
    sort_by_url(db, &mut std_files);
    files.extend(std_files);
    files
}

fn sort_by_url(db: &RootDatabase, files: &mut [File]) {
    files.sort_by(|a, b| a.url(db).as_str().cmp(b.url(db).as_str()));
}

#[cfg(test)]
mod tests {
    use super::*;
    use auto_lsp::{default::db::FileManager, lsp_types::Url};

    /// Inserting files in one order and getting them back in another is the
    /// whole defect, so insert them deliberately out of order.
    fn db_with(names: &[&str]) -> RootDatabase {
        let mut db = RootDatabase::default();
        for name in names {
            let url = Url::parse(&format!("file:///{name}")).unwrap();
            let file = auto_lsp::default::db::file::File::from_string()
                .db(&db)
                .parsers(&ast::RK_PARSER)
                .url(&url)
                .source(String::new())
                .call()
                .unwrap();
            db.add_file(file).unwrap();
        }
        db
    }

    fn urls(db: &RootDatabase, files: &[auto_lsp::default::db::file::File]) -> Vec<String> {
        files.iter().map(|f| f.url(db).as_str().to_owned()).collect()
    }

    /// The order must come from the URLs, not from the map.
    #[test]
    fn files_come_back_in_url_order() {
        let db = db_with(&["zeta.st", "alpha.st", "mid.st", "beta.st"]);
        assert_eq!(
            urls(&db, &ordered_files(&db)),
            vec![
                "file:///alpha.st",
                "file:///beta.st",
                "file:///mid.st",
                "file:///zeta.st",
            ]
        );
    }

    /// Nothing in the crate may reach for the file maps directly: a source
    /// scan catches the site being added, since hash order is stable within
    /// one process. A genuinely unordered walk goes in `ALLOWED` with a note.
    #[test]
    fn nothing_else_iterates_the_file_maps() {
        /// Sites permitted to touch the maps directly, relative to `src/`.
        /// `file_order.rs` is the one that imposes the order.
        const ALLOWED: &[&str] = &["file_order.rs"];
        // Split so this test does not match its own source.
        let banned = ["get_files", "get_std_lib_files"].map(|n| format!("{n}()"));

        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut offenders = Vec::new();
        let mut stack = vec![src.clone()];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).expect("read src dir") {
                let path = entry.expect("dir entry").path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                if path.extension().is_none_or(|e| e != "rs") {
                    continue;
                }
                let rel = path.strip_prefix(&src).unwrap().to_string_lossy().to_string();
                if ALLOWED.contains(&rel.as_str()) {
                    continue;
                }
                let text = std::fs::read_to_string(&path).expect("read source");
                for (n, line) in text.lines().enumerate() {
                    if banned.iter().any(|b| line.contains(b.as_str())) {
                        offenders.push(format!("{rel}:{}: {}", n + 1, line.trim()));
                    }
                }
            }
        }

        assert!(
            offenders.is_empty(),
            "these sites walk the file maps directly instead of going through \
             `file_order`, so their order varies per process:\n  {}",
            offenders.join("\n  ")
        );
    }

    /// Enough files that a hash-order accident cannot pass by luck: a single
    /// unsorted run has a 1-in-10! chance of coming out sorted.
    #[test]
    fn ordering_holds_for_many_files() {
        let names: Vec<String> = (0..10).rev().map(|i| format!("f{i:02}.st")).collect();
        let refs: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
        let db = db_with(&refs);
        let got = urls(&db, &ordered_files(&db));
        let mut expected = got.clone();
        expected.sort();
        assert_eq!(got, expected, "files must be in URL order");
    }
}
