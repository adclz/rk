use db::RootDatabase;
use hir::hir_def::semantic_index::semantic_index;
use ide_proto::AsProtocol;
use insta::assert_debug_snapshot;
use rstest::rstest;
use auto_lsp::default::db::BaseDatabase;

use crate::tests::utils::{add_sources, with_db};


// after the FUNCTION_BLOCK name but before VAR
// there should be completions for EXTENDS, IMPLEMENTS and VARS.
#[rstest]
pub fn cursor_before_fb_variables(mut with_db: RootDatabase) {
    let source = r#"FUNCTION_BLOCK fb1 
    VAR
        var1: INT;
        var2: REAL;
    END_VAR 
END_FUNCTION_BLOCK"#;

    add_sources(&mut with_db, &[source]);
    let sema = semantic_index(&with_db, *with_db.get_files().iter().last().unwrap());

    // 19 = fb1 | VAR
    let in_variables = sema.descendant_at(&with_db, 19).unwrap();
    let completions = in_variables.as_proto().completion(&with_db, 19).unwrap();

    assert!(format!("{:?}", completions).contains("EXTENDS"));
    assert!(format!("{:?}", completions).contains("IMPLEMENTS"));
    assert!(format!("{:?}", completions).contains("VAR"));
    assert!(format!("{:?}", completions).contains("VAR_INPUT"));
    assert!(format!("{:?}", completions).contains("VAR_OUTPUT"));
    assert!(format!("{:?}", completions).contains("VAR_TEMP"));
}

// implements is already defined, so there should be no completion for it.
// however, extends should still be suggested.
#[rstest]
pub fn implements_already_defined(mut with_db: RootDatabase) {
    let source = r#"FUNCTION_BLOCK fb1 IMPLEMENTS impl1
    VAR
        var1: INT;
        var2: REAL;
    END_VAR 
END_FUNCTION_BLOCK"#;

    add_sources(&mut with_db, &[source]);
    let sema = semantic_index(&with_db, *with_db.get_files().iter().last().unwrap());

    // 19 = fb1 | VAR
    let in_variables = sema.descendant_at(&with_db, 19).unwrap();
    let completions = in_variables.as_proto().completion(&with_db, 19).unwrap();

    assert!(format!("{:?}", completions).contains("EXTENDS"));
    assert!(!format!("{:?}", completions).contains("IMPLEMENTS"));
}

// same but reversed
#[rstest]
pub fn extends_already_defined(mut with_db: RootDatabase) {
    let source = r#"FUNCTION_BLOCK fb1 EXTENDS ext
    VAR
        var1: INT;
        var2: REAL;
    END_VAR 
END_FUNCTION_BLOCK"#;

    add_sources(&mut with_db, &[source]);
    let sema = semantic_index(&with_db, *with_db.get_files().iter().last().unwrap());

    // 19 = fb1 | VAR
    let in_variables = sema.descendant_at(&with_db, 19).unwrap();
    let completions = in_variables.as_proto().completion(&with_db, 19).unwrap();

    assert!(!format!("{:?}", completions).contains("EXTENDS"));
    assert!(format!("{:?}", completions).contains("IMPLEMENTS"));
}

// both implements and extends are already defined, so there should be no completions for either.
#[rstest]
pub fn implements_extends_already_defined(mut with_db: RootDatabase) {
    let source = r#"FUNCTION_BLOCK fb1 EXTENDS ext IMPLEMENTS impl1
    VAR
        var1: INT;
        var2: REAL;
    END_VAR 
END_FUNCTION_BLOCK"#;

    add_sources(&mut with_db, &[source]);
    let sema = semantic_index(&with_db, *with_db.get_files().iter().last().unwrap());

    // 19 = fb1 | VAR
    let in_variables = sema.descendant_at(&with_db, 19).unwrap();
    let completions = in_variables.as_proto().completion(&with_db, 19).unwrap();

    assert!(!format!("{:?}", completions).contains("EXTENDS"));
    assert!(!format!("{:?}", completions).contains("IMPLEMENTS"));
}
