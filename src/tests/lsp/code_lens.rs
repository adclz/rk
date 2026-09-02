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
