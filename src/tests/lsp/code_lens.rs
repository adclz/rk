use std::ops::ControlFlow;

use auto_lsp::default::db::BaseDatabase;
use db::RootDatabase;
use hir::hir_def::semantic_index::semantic_index;
use ide_proto::{handlers::CodeLensHandler, walk::WalkHir};
use rstest::rstest;

use crate::tests::utils::{add_sources, with_db};

fn collect_code_lenses(db: &RootDatabase) -> Vec<auto_lsp::lsp_types::CodeLens> {
    let mut results = vec![];
    for file in db.get_files().iter() {
        let sema = semantic_index(db, *file);
        let _ = sema.walk_hir(db, &mut |node| {
            if let Some(lens) = node.code_lens(db) {
                results.push(lens);
            }
            ControlFlow::Continue(())
        });
    }
    results
}

#[rstest]
fn mixed_test_and_non_test(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION helper : INT
END_FUNCTION

{test}
FUNCTION test_one : INT
END_FUNCTION

PROGRAM test_two
END_PROGRAM

PROGRAM normal
END_PROGRAM
"#;

    add_sources(&mut with_db, &[source]);
    let lenses = collect_code_lenses(&with_db);

    assert_eq!(lenses.len(), 1, "a PROGRAM is never a test (E0252), so only the FUNCTION gets a lens");

    let names: Vec<_> = lenses
        .iter()
        .map(|l| {
            l.command.as_ref().unwrap().arguments.as_ref().unwrap()[1]
                .as_str()
                .unwrap()
                .to_string()
        })
        .collect();

    assert!(names.contains(&"test_one".to_string()));
    assert!(!names.contains(&"test_two".to_string()));
}

/// The "Run test" lens sits on the `{test}` pragma, so the editor draws it
/// directly above the mark it runs. A declaration's own span opens at its
/// FIRST pragma, so an `{allow ...}` written above `{test}` used to carry the
/// lens up with it, onto a pragma that has nothing to do with the test.
#[rstest]
fn the_test_lens_sits_on_the_test_pragma(mut with_db: RootDatabase) {
    let source = r#"{allow 'constant-condition'}
{test}
FUNCTION test_marked : BOOL
    test_marked := TRUE;
END_FUNCTION
"#;
    add_sources(&mut with_db, &[source]);
    let lenses = collect_code_lenses(&with_db);
    assert_eq!(lenses.len(), 1);

    let line = lenses[0].range.start.line as usize;
    assert_eq!(
        source.split('\n').nth(line),
        Some("{test}"),
        "the lens anchors to the test pragma, not to the pragma block's first line"
    );
}

/// With `{test}` first, the lens is on it too: the anchor is the marker, not
/// a position in the block.
#[rstest]
fn the_test_lens_ignores_a_pragma_written_after_it(mut with_db: RootDatabase) {
    let source = r#"{test}
{allow 'constant-condition'}
FUNCTION test_marked : BOOL
    test_marked := TRUE;
END_FUNCTION
"#;
    add_sources(&mut with_db, &[source]);
    let lenses = collect_code_lenses(&with_db);
    assert_eq!(lenses.len(), 1);
    let line = lenses[0].range.start.line as usize;
    assert_eq!(source.split('\n').nth(line), Some("{test}"));
}
