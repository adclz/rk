//! One deterministic order over the workspace's files. The database stores
//! files in `DashMap`s, whose iteration order changes per process, and
//! that order reaches the wasm function index space. Anything that feeds
//! compilation or per-file output takes its files from here.

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

/// The workspace's files followed by the libraries', each group in URL
/// order, so adding a workspace file cannot renumber the libraries'
/// functions.
pub fn ordered_files_with_libraries(db: &RootDatabase) -> Vec<File> {
    let mut files = ordered_files(db);
    let mut std_files: Vec<File> = db
        .get_library_files()
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
        /// Sites permitted to touch the maps directly: `file_order.rs` imposes
        /// the order; `workspace.rs` only asserts membership in tests.
        const ALLOWED: &[&str] = &["file_order.rs", "workspace.rs"];
        // Split so this test does not match its own source.
        let banned = ["get_files", "get_library_files"].map(|n| format!("{n}()"));

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
