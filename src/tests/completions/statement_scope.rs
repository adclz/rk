use db::RootDatabase;
use hir::hir_def::semantic_index::semantic_index;
use ide_proto::AsProtocol;
use rstest::rstest;
use auto_lsp::default::db::BaseDatabase;

use crate::tests::utils::{add_sources, with_db};

// local variables completions
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
pub fn variables_in_scope(mut with_db: RootDatabase, #[case] source: (&str, usize)) {
    add_sources(&mut with_db, &[source.0]);
    let sema = semantic_index(&with_db, *with_db.get_files().iter().last().unwrap());

    let in_variables = sema.descendant_at(&with_db, source.1).unwrap();
    let completions = in_variables.as_proto().completion(&with_db, source.1).unwrap();

    assert!(format!("{completions:?}").contains("var1"));
    assert!(format!("{completions:?}").contains("var2"));
}


// global POUs completions
#[rstest]
#[case((r#"
FUNCTION fn1 END_FUNCTION
FUNCTION fn2 END_FUNCTION

FUNCTION  f1


END_FUNCTION"#, 66))] 
#[case((r#"
FUNCTION fn1 END_FUNCTION
FUNCTION fn2 END_FUNCTION

FUNCTION_BLOCK  fb1


END_FUNCTION_BLOCK"#, 73))]
pub fn global_functions_in_scope(mut with_db: RootDatabase, #[case] source: (&str, usize)) {
    add_sources(&mut with_db, &[source.0]);
    let sema = semantic_index(&with_db, *with_db.get_files().iter().last().unwrap());

    let in_variables = sema.descendant_at(&with_db, source.1).unwrap();
    let completions = in_variables.as_proto().completion(&with_db, source.1).unwrap();

    assert!(format!("{completions:?}").contains("fn1"));
    assert!(format!("{completions:?}").contains("fn2"));
}


// global POUs completions
#[rstest]
#[case((r#"
NAMESPACE ns
    FUNCTION fn1 END_FUNCTION
    FUNCTION fn2 END_FUNCTION
END_NAMESPACE

FUNCTION  f1


END_FUNCTION"#, 101))] 
#[case((r#"
NAMESPACE ns
    FUNCTION fn1 END_FUNCTION
    FUNCTION fn2 END_FUNCTION
END_NAMESPACE

FUNCTION_BLOCK  fb1


END_FUNCTION_BLOCK"#, 108))]
pub fn import_functions_in_scope(mut with_db: RootDatabase, #[case] source: (&str, usize)) {
    add_sources(&mut with_db, &[source.0]);
    let sema = semantic_index(&with_db, *with_db.get_files().iter().last().unwrap());

    let in_variables = sema.descendant_at(&with_db, source.1).unwrap();
    let completions = in_variables.as_proto().completion(&with_db, source.1).unwrap();

    assert!(format!("{completions:?}").contains("fn1")); // via using
    assert!(format!("{completions:?}").contains("fn2")); // via using
}