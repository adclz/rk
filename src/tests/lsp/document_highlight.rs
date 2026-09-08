use auto_lsp::default::db::BaseDatabase;
use db::RootDatabase;
use ide_proto::{handlers::document_highlight::document_highlight, walk::descendant_at};
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{add_sources, with_db};

/// The highlights for the symbol at `needle`, as `line: text`.
fn highlights(db: &mut RootDatabase, sources: &[&str], needle: &str) -> String {
    add_sources(db, sources);
    // The file holding `sources[0]`, found by its content: the registry is a
    // set, so its order is not the argument order. The second file is here to
    // prove the answer does not leave the first.
    let source = sources[0];
    let file = *db
        .get_files()
        .iter()
        .find(|f| f.document(db).as_str() == source)
        .expect("the first source is registered");
    let offset = source
        .find(needle)
        .unwrap_or_else(|| panic!("{needle:?} not in the source"));
    let Some(node) = descendant_at(db, file, offset) else {
        return "<no node>".to_string();
    };
    let Some(found) = document_highlight(db, &node, file) else {
        return "<none>".to_string();
    };
    let lines: Vec<&str> = source.lines().collect();
    found
        .iter()
        .map(|h| {
            let line = h.range.start.line as usize;
            let (a, b) = (
                h.range.start.character as usize,
                h.range.end.character as usize,
            );
            let text = lines.get(line).map(|l| &l[a.min(l.len())..b.min(l.len())]);
            format!("line {line}: {}", text.unwrap_or("?"))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

const MAIN: &str = r#"
FUNCTION helper : INT
VAR_INPUT a : INT; END_VAR
    helper := a;
END_FUNCTION

FUNCTION caller : INT
VAR n : INT; END_VAR
    n := helper(a := 1);
    n := n + helper(a := 2);
    caller := n;
END_FUNCTION"#;

const OTHER: &str = r#"
FUNCTION elsewhere : INT
    elsewhere := helper(a := 3);
END_FUNCTION"#;

/// Declaration and every use, in this document only: `helper` is called from
/// the second file too, and that call is not in the answer.
#[rstest]
fn one_document_only(mut with_db: RootDatabase) {
    assert_snapshot!(highlights(&mut with_db, &[MAIN, OTHER], "FUNCTION helper"), @r"
    line 1: helper
    line 3: helper
    line 8: helper
    line 9: helper
    ");
}

/// A local, from one of its uses: the declaration and every occurrence,
/// including two on one line.
#[rstest]
fn a_local_from_a_use(mut with_db: RootDatabase) {
    assert_snapshot!(highlights(&mut with_db, &[MAIN, OTHER], "n + helper"), @r"
    line 7: n
    line 8: n
    line 9: n
    line 9: n
    line 10: n
    ");
}
