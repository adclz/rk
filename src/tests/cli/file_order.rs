use db::RootDatabase;
use rk::file_order::ordered_files;

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
    files
        .iter()
        .map(|f| f.url(db).as_str().to_owned())
        .collect()
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
    /// the order, and nothing else does.
    const ALLOWED: &[&str] = &["file_order.rs"];
    // Split so this test does not match its own source.
    let banned = ["get_files", "get_library_files"].map(|n| format!("{n}()"));

    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("crates/cli/src");
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
            let rel = path
                .strip_prefix(&src)
                .unwrap()
                .to_string_lossy()
                .to_string();
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
