use db::RootDatabase;
use hir::hir_def::semantic_index::semantic_index;
use ide_proto::AsProtocol;
use insta::assert_debug_snapshot;
use rstest::rstest;
use auto_lsp::default::db::BaseDatabase;

use crate::tests::utils::{add_sources, with_db};


//  functions can only have variables and statements
#[rstest]
pub fn function_variables(mut with_db: RootDatabase) {
    let source = r#"FUNCTION fn1

END_FUNCTION"#;

    add_sources(&mut with_db, &[source]);
    let sema = semantic_index(&with_db, *with_db.get_files().iter().last().unwrap());

    // 13 = middle
    let in_variables = sema.descendant_at(&with_db, 13).unwrap();
    let completions = in_variables.as_proto().completion(&with_db, 13).unwrap();

    assert!(format!("{:?}", completions).contains("VAR"));
    assert!(format!("{:?}", completions).contains("VAR_INPUT"));
    assert!(format!("{:?}", completions).contains("VAR_OUTPUT"));
    assert!(format!("{:?}", completions).contains("VAR_TEMP"));
    assert!(format!("{:?}", completions).contains("IF"));
    assert!(format!("{:?}", completions).contains("FOR"));
    assert!(format!("{:?}", completions).contains("WHILE"));
    assert!(format!("{:?}", completions).contains("REPEAT"));
}

