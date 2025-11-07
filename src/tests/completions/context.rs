use std::ops::ControlFlow;

use auto_lsp::default::db::BaseDatabase;
use auto_lsp::lsp_types::InlayHint;
use db::RootDatabase;
use hir::hir_def::semantic_index::HirNode;
use hir::hir_def::semantic_index::get_scope;
use hir::hir_def::semantic_index::semantic_index;
use hir::walk::WalkHir;
use ide_proto::AsProtocol;
use ide_proto::ToProtocol;
use insta::assert_debug_snapshot;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::add_sources;
use crate::tests::utils::with_db;

#[rstest]
pub fn no_ctx(mut with_db: RootDatabase) {
    let source = r#"
// Comment should not trigger completions
FUNCTION_BLOCK  fb1
    VAR
        var1: INT;
        var2: REAL;
    END_VAR 
END_FUNCTION_BLOCK"#;

    add_sources(&mut with_db, &[source]);
    let sema = semantic_index(&with_db, *with_db.get_files().iter().last().unwrap());

    assert!(sema.descendant_at(&with_db, 2).is_none());
}

// there should be no completions before the POU name
#[rstest]
#[case((r#"CLASS  fb1
    VAR
        var1: INT;
        var2: REAL;
    END_VAR 
END_CLASS"#, 7))]
#[case((r#"FUNCTION  fn1
    VAR
        var1: INT;
        var2: REAL;
    END_VAR 
END_FUNCTION"#, 10))]
#[case((r#"INTERFACE  i1

END_INTERFACE"#, 11))]
#[case((r#"FUNCTION_BLOCK  fb1
    VAR
        var1: INT;
        var2: REAL;
    END_VAR 
END_FUNCTION_BLOCK"#, 16))]
pub fn broad_ctx_before_pou_name(mut with_db: RootDatabase, #[case] source: (&str, usize)) {
    add_sources(&mut with_db, &[source.0]);
    let sema = semantic_index(&with_db, *with_db.get_files().iter().last().unwrap());

    // source.1 = offset = POU_KEYWORD | pou_name
    let in_variables = sema.descendant_at(&with_db, source.1).unwrap();
    let completions = in_variables.as_proto().completion(&with_db, source.1).unwrap();

    insta::allow_duplicates! { 
        assert_debug_snapshot!(completions, @r"[]");
    }
}

// statements completions should be available inside the POU body, right after any variable block
#[rstest]
#[case((r#"FUNCTION  fn1
    VAR
        var1: INT;
        var2: REAL;
    END_VAR 



END_FUNCTION"#, 75))] 
#[case((r#"FUNCTION_BLOCK  fb1
    VAR
        var1: INT;
        var2: REAL;
    END_VAR 




END_FUNCTION_BLOCK"#, 88))]
pub fn statements_and_variables_after_variables(mut with_db: RootDatabase, #[case] source: (&str, usize)) {
    add_sources(&mut with_db, &[source.0]);
    let sema = semantic_index(&with_db, *with_db.get_files().iter().last().unwrap());

    let in_variables = sema.descendant_at(&with_db, source.1).unwrap();
    let completions = in_variables.as_proto().completion(&with_db, source.1).unwrap();

    assert!(format!("{:?}", completions).contains("VAR_INPUT"));
    assert!(format!("{:?}", completions).contains("VAR_OUTPUT"));
    assert!(format!("{:?}", completions).contains("VAR_IN_OUT"));
    assert!(format!("{:?}", completions).contains("VAR_TEMP"));
    assert!(format!("{:?}", completions).contains("IF"));
    assert!(format!("{:?}", completions).contains("FOR"));
}
