use auto_lsp::default::db::BaseDatabase;
use db::RootDatabase;
use ide_proto::{handlers::CompletionHandler, walk::descendant_at};
use rstest::rstest;

use crate::tests::utils::{add_sources, with_db};

#[rstest]
pub fn struct_field_completion(mut with_db: RootDatabase) {
    let source = r#"
TYPE Engine :
    STRUCT
        oil : REAL;
        fuel: INT;
    END_STRUCT
END_TYPE


FUNCTION_BLOCK fb
    VAR
        my_var: Engine;
    END_VAR

    my_var.
END_FUNCTION_BLOCK
"#;

    add_sources(&mut with_db, &[source]);
    let path_expr =
        descendant_at(&with_db, *with_db.get_files().iter().last().unwrap(), 162).unwrap();
    let completions = path_expr
        .completion(&with_db, 162, None, "".into())
        .unwrap();

    assert_eq!(completions.len(), 2);
    assert!(format!("{completions:?}").contains("oil"));
    assert!(format!("{completions:?}").contains("fuel"));
}

#[rstest]
pub fn class_var_and_methods_completion(mut with_db: RootDatabase) {
    let source = r#"
CLASS Engine
    VAR
        oil : REAL;
        fuel: INT;
    END_VAR

    METHOD Start
    END_METHOD

END_CLASS

FUNCTION_BLOCK fb
    VAR
        my_var: Engine;
    END_VAR

    my_var.
END_FUNCTION_BLOCK
"#;

    add_sources(&mut with_db, &[source]);
    let path_expr =
        descendant_at(&with_db, *with_db.get_files().iter().last().unwrap(), 191).unwrap();
    let completions = path_expr
        .completion(&with_db, 191, None, "".into())
        .unwrap();

    assert_eq!(completions.len(), 3);
    assert!(format!("{completions:?}").contains("oil"));
    assert!(format!("{completions:?}").contains("fuel"));
    assert!(format!("{completions:?}").contains("Start()"));
}

#[rstest]
pub fn enum_variants_completion(mut with_db: RootDatabase) {
    let source = r#"
TYPE
    List: UINT (A, B, C);
END_TYPE

FUNCTION fn1
    VAR
        test: List;
    END_VAR

    test := List#

END_FUNCTION
"#;

    add_sources(&mut with_db, &[source]);
    let expr = descendant_at(&with_db, *with_db.get_files().iter().last().unwrap(), 112).unwrap();
    let completions = expr.completion(&with_db, 112, None, "".into()).unwrap();

    assert_eq!(completions.len(), 3);
    assert!(format!("{completions:?}").contains("A"));
    assert!(format!("{completions:?}").contains("B"));
    assert!(format!("{completions:?}").contains("C"));
}

#[rstest]
pub fn invocation_completion(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        test: INT;
    END_VAR

    METHOD method 
    
    END_METHOD

    THIS.

END_FUNCTION_BLOCK
"#;

    add_sources(&mut with_db, &[source]);
    let expr = descendant_at(&with_db, *with_db.get_files().iter().last().unwrap(), 108).unwrap();
    let completions = expr.completion(&with_db, 108, None, "".into()).unwrap();

    assert_eq!(completions.len(), 2);
    assert!(format!("{completions:?}").contains("test"));
    assert!(format!("{completions:?}").contains("method()"));
}
