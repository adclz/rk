use db::RootDatabase;
use hir::hir_def::semantic_index::semantic_index;
use ide_proto::AsProtocol;
use rstest::rstest;
use auto_lsp::default::db::BaseDatabase;

use crate::tests::utils::{add_sources, with_db};


// only FUNCTION_BLOCKs sand CLASSEs should be suggested to EXTENDS

#[rstest]
pub fn extends_only_fb(mut with_db: RootDatabase) {
    // in this context, only cb1 should be suggested to EXTENDS
    let source = r#"
FUNCTION_BLOCK cb1 END_FUNCTION_BLOCK

FUNCTION cfn1 END_FUNCTION
INTERFACE cn1 END_INTERFACE
    
FUNCTION_BLOCK fb1 EXTENDS c

END_FUNCTION_BLOCK"#;

    add_sources(&mut with_db, &[source]);
    let sema = semantic_index(&with_db, *with_db.get_files().iter().last().unwrap());

    // 128 = EXTENDS c
    let in_variables = sema.descendant_at(&with_db, 128).unwrap();
    let completions = in_variables.as_proto().completion(&with_db, 128).unwrap();

    assert!(format!("{completions:?}").contains("cb1"));
    assert!(!format!("{completions:?}").contains("cfn1"));
    assert!(!format!("{completions:?}").contains("cn1"));
}

#[rstest]
pub fn extends_only_class(mut with_db: RootDatabase) {
    // in this context, only cl1 should be suggested to EXTENDS
    let source = r#"
CLASS cl1 END_CLASS

FUNCTION cfn1 END_FUNCTION
INTERFACE cn1 END_INTERFACE
    
FUNCTION_BLOCK fb1 EXTENDS c

END_FUNCTION_BLOCK"#;

    add_sources(&mut with_db, &[source]);
    let sema = semantic_index(&with_db, *with_db.get_files().iter().last().unwrap());

    // 109 = EXTENDS c
    let in_variables = sema.descendant_at(&with_db, 109).unwrap();
    let completions = in_variables.as_proto().completion(&with_db, 109).unwrap();

    assert!(format!("{completions:?}").contains("cl1"));
    assert!(!format!("{completions:?}").contains("cfn1"));
    assert!(!format!("{completions:?}").contains("cn1"));
}

#[rstest]
pub fn implements_only_interface(mut with_db: RootDatabase) {
    // in this context, only in1 should be suggested to IMPLEMENTS
    let source = r#"
CLASS il1 END_CLASS

FUNCTION ifn1 END_FUNCTION
INTERFACE in1 END_INTERFACE
    
FUNCTION_BLOCK fb1 IMPLEMENTS i

END_FUNCTION_BLOCK"#;

    add_sources(&mut with_db, &[source]);
    let sema = semantic_index(&with_db, *with_db.get_files().iter().last().unwrap());

    // 112 = EXTENDS c
    let in_variables = sema.descendant_at(&with_db, 112).unwrap();
    let completions = in_variables.as_proto().completion(&with_db, 112).unwrap();

    assert!(format!("{completions:?}").contains("in1"));
    assert!(!format!("{completions:?}").contains("il1"));
    assert!(!format!("{completions:?}").contains("ifn1"));
}