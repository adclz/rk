use auto_lsp::default::db::BaseDatabase;
use db::RootDatabase;
use hir::HirNodeInfo;
use ide_proto::{
    handlers::{
        CompletionHandler, CompletionRequest,
        completions_utils::{CompletionCtx, QueryMode},
    },
    walk::descendant_at,
};
use rstest::rstest;

use crate::tests::utils::{add_sources, find_pou_with_name, with_db};

#[rstest]
pub fn body_query_scope_variables(mut with_db: RootDatabase) {
    let source = r#"   
    FUNCTION fn
        VAR_INPUT
            input_var: INT;
        END_VAR

        VAR_OUTPUT
            output_var: INT;
        END_VAR

        VAR
            temp_var: INT;
        END_VAR

        VAR_IN_OUT
            inout_var: INT;
        END_VAR

    END_FUNCTION
"#;

    add_sources(&mut with_db, &[source]);
    let pou =
        find_pou_with_name(&with_db, *with_db.get_files().iter().last().unwrap(), "fn").unwrap();

    let mut ctx = CompletionCtx::new(283, QueryMode::Body);
    ctx.scope_completion(pou.get_scope_id(&with_db), "", &with_db);
    let completions = ctx.take_items();

    assert_eq!(completions.len(), 5);
    assert!(format!("{completions:?}").contains("input_var"));
    assert!(format!("{completions:?}").contains("output_var"));
    assert!(format!("{completions:?}").contains("temp_var"));
    assert!(format!("{completions:?}").contains("inout_var"));
    // fn itself should be in scope
    assert!(format!("{completions:?}").contains("fn"));
    // it should have a signature with all variable sections
}

#[rstest]
pub fn body_query_pous_in_scope(mut with_db: RootDatabase) {
    let source = r#"  
    FUNCTION_BLOCK fb1
    END_FUNCTION_BLOCK

    CLASS cl1
    END_CLASS

    INTERFACE itf1
    END_INTERFACE

    FUNCTION fn1
    END_FUNCTION

    FUNCTION fn2

    END_FUNCTION
"#;

    add_sources(&mut with_db, &[source]);
    let pou =
        find_pou_with_name(&with_db, *with_db.get_files().iter().last().unwrap(), "fn2").unwrap();

    let mut ctx = CompletionCtx::new(169, QueryMode::Body);
    ctx.scope_completion(pou.get_scope_id(&with_db), "", &with_db);
    let completions = ctx.take_items();

    // only functions should be in scope, because all other POUs must be instantiated
    assert_eq!(completions.len(), 2);
    assert!(format!("{completions:?}").contains("fn1"));
    assert!(format!("{completions:?}").contains("fn2"));

    // both should have signatures in insert_text
    assert!(format!("{completions:?}").contains("fn1()"));
    assert!(format!("{completions:?}").contains("fn2()"));
}

/// DataType completions in body mode should not include parentheses.
/// They are used as constants (TYPE_NAME.field), not called.
#[rstest]
pub fn data_type_completion_no_parentheses(mut with_db: RootDatabase) {
    let source = r#"
    TYPE
        MY_CONSTANTS : STRUCT
            MAX_VAL : INT := 100;
        END_STRUCT
    END_TYPE

    FUNCTION fn1
    END_FUNCTION
"#;

    add_sources(&mut with_db, &[source]);
    let pou =
        find_pou_with_name(&with_db, *with_db.get_files().iter().last().unwrap(), "fn1").unwrap();

    let mut ctx = CompletionCtx::new(120, QueryMode::Body);
    ctx.scope_completion(pou.get_scope_id(&with_db), "", &with_db);
    let completions = ctx.take_items();

    let debug = format!("{completions:?}");

    // DataType should be in completions
    assert!(
        debug.contains("MY_CONSTANTS"),
        "MY_CONSTANTS should appear in completions"
    );

    // DataType should NOT have parentheses in insert_text
    assert!(
        !debug.contains("MY_CONSTANTS("),
        "DataType completion should not include parentheses"
    );

    // Function should still have parentheses
    assert!(
        debug.contains("fn1()"),
        "Function completion should include parentheses"
    );
}

#[rstest]
pub fn deduplicate_scope_and_local_vars(mut with_db: RootDatabase) {
    let source = r#"  
    FUNCTION fn1
    END_FUNCTION

    FUNCTION_BLOCK fn2
        VAR
            fn1: INT;
        END_VAR

    END_FUNCTION_BLOCK
"#;

    add_sources(&mut with_db, &[source]);
    let pou =
        find_pou_with_name(&with_db, *with_db.get_files().iter().last().unwrap(), "fn2").unwrap();

    let mut ctx = CompletionCtx::new(102, QueryMode::Body);
    ctx.scope_completion(pou.get_scope_id(&with_db), "", &with_db);
    let completions = ctx.take_items();

    // variable should override the function in scope
    assert_eq!(completions.len(), 1);
    assert!(format!("{completions:?}").contains("fn1"));

    // both should have signatures in insert_text
    assert!(!format!("{completions:?}").contains("fn1()"));
}

#[rstest]
pub fn enum_variants_completion(mut with_db: RootDatabase) {
    let source = r#"
TYPE
    List: UINT (A, B, C);
END_TYPE

FUNCTION_BLOCK fn1
    VAR
        test: List;
    END_VAR

    test := L

END_FUNCTION_BLOCK
"#;

    add_sources(&mut with_db, &[source]);
    let expr = descendant_at(&with_db, *with_db.get_files().iter().last().unwrap(), 128).unwrap();
    let req = CompletionRequest {
        offset: 128,
        trigger_character: None,
        query: "".into(),
        node_index_pos: None,
        is_last_before: false,
    };
    let completions = expr.completion(&with_db, &req).unwrap();

    assert_eq!(completions.len(), 10); // statements ... + 2 pragmas + 3 variants
    assert!(format!("{completions:?}").contains("List#A"));
    assert!(format!("{completions:?}").contains("List#B"));
    assert!(format!("{completions:?}").contains("List#C"));
}
