use auto_lsp::default::db::BaseDatabase;
use db::RootDatabase;
use ide_proto::{
    walk::{descendant_at},
};
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
    let completions = path_expr.completion(&with_db, 162).unwrap();

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
    let completions = path_expr.completion(&with_db, 191).unwrap();

    assert_eq!(completions.len(), 3);
    assert!(format!("{completions:?}").contains("oil"));
    assert!(format!("{completions:?}").contains("fuel"));
    assert!(format!("{completions:?}").contains("Start()"));
}
