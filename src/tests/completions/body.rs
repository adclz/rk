use auto_lsp::default::db::BaseDatabase;
use db::RootDatabase;
use hir::{
    HirNodeInfo
};
use ide_proto::{
    handlers::completions_utils::scope::{QueryMode, ScopeCompletionCtx},
};
use rstest::rstest;

use crate::tests::utils::{add_sources, find_pou_with_name, with_db};

/// Test completion BEFORE any path expressions (generic scope)
/// This tests the fallback case where we have no PathExpr at offset
#[rstest]
pub fn body_completion_before_path_expr(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test_fn
    VAR
        my_var: INT;
        another_var: REAL;
    END_VAR

    my_var := 10;
END_FUNCTION
"#;

    add_sources(&mut with_db, &[source]);
    let pou = find_pou_with_name(&with_db, *with_db.get_files().iter().last().unwrap(), "test_fn")
        .unwrap();

    // Test completion at line with assignment (before the path expression starts)
    let offset = source.find("END_VAR").unwrap() + 7; // Position after END_VAR, before any statement
    let mut ctx = ScopeCompletionCtx::new(QueryMode::Body, pou.get_scope_id(&with_db), offset, "");
    ctx.query_scope_items(&with_db);
    let completions = ctx.take_items();

    assert!(!completions.is_empty());
    assert!(format!("{completions:?}").contains("my_var"));
    assert!(format!("{completions:?}").contains("another_var"));
}

/// Test completion WITHIN a simple path expression (VariableAccess)
/// This tests the case where offset is inside a VariableAccess node
#[rstest]
pub fn body_completion_within_variable_access(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test_fn
    VAR
        my_var: INT;
        another_var: REAL;
    END_VAR

    my_var := 10;
END_FUNCTION
"#;

    add_sources(&mut with_db, &[source]);
    let pou = find_pou_with_name(&with_db, *with_db.get_files().iter().last().unwrap(), "test_fn")
        .unwrap();

    // Test completion INSIDE the variable name itself (within PathExpr/VariableAccess)
    let offset = source.find("my_var := 10").unwrap() + 1; // Position inside "my_var"
    let mut ctx = ScopeCompletionCtx::new(QueryMode::Body, pou.get_scope_id(&with_db), offset, "");
    ctx.query_scope_items(&with_db);
    let completions = ctx.take_items();

    assert!(!completions.is_empty());
    assert!(format!("{completions:?}").contains("my_var"));
    assert!(format!("{completions:?}").contains("another_var"));
}

/// Test completion AFTER a complete path expression (fallback case)
/// This tests where offset is past the end of a PathExpr, should fall back to parent/target
#[rstest]
pub fn body_completion_after_variable_access(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test_fn
    VAR
        my_var: INT;
        another_var: REAL;
    END_VAR

    my_var := 10;
END_FUNCTION
"#;

    add_sources(&mut with_db, &[source]);
    let pou = find_pou_with_name(&with_db, *with_db.get_files().iter().last().unwrap(), "test_fn")
        .unwrap();

    // Test completion after the variable (at the space after "my_var")
    let offset = source.find("my_var :=").unwrap() + 6; // Position just after "my_var"
    let mut ctx = ScopeCompletionCtx::new(QueryMode::Body, pou.get_scope_id(&with_db), offset, "");
    ctx.query_scope_items(&with_db);
    let completions = ctx.take_items();

    // Should have completions from scope (fallback worked)
    assert!(!completions.is_empty());
}

/// Test completion in function block with variables
#[rstest]
pub fn body_completion_in_function_block(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK my_fb
    VAR
        internal_var: INT;
        another_fb_var: REAL;
    END_VAR

END_FUNCTION_BLOCK
"#;

    add_sources(&mut with_db, &[source]);
    let pou = find_pou_with_name(&with_db, *with_db.get_files().iter().last().unwrap(), "my_fb")
        .unwrap();

    // Get completions in function block (which is the scope)
    let offset = source.find("END_FUNCTION_BLOCK").unwrap();
    let mut ctx = ScopeCompletionCtx::new(QueryMode::Body, pou.get_scope_id(&with_db), offset, "");
    ctx.query_scope_items(&with_db);
    let completions = ctx.take_items();

    assert!(!completions.is_empty());
    assert!(format!("{completions:?}").contains("internal_var"));
    assert!(format!("{completions:?}").contains("another_fb_var"));
}

/// Test completion with multiple POUs in scope (some requiring USING)
#[rstest]
pub fn body_completion_with_namespace_pous(mut with_db: RootDatabase) {
    let source = r#"
NAMESPACE ns1
    FUNCTION fn_in_ns
    END_FUNCTION
END_NAMESPACE

FUNCTION main
    VAR
        local_var: INT;
    END_VAR

    fn_in_ns();
END_FUNCTION
"#;

    add_sources(&mut with_db, &[source]);
    let pou = find_pou_with_name(&with_db, *with_db.get_files().iter().last().unwrap(), "main")
        .unwrap();

    let offset = source.find("fn_in_ns()").unwrap();
    let mut ctx = ScopeCompletionCtx::new(QueryMode::Body, pou.get_scope_id(&with_db), offset, "");
    ctx.query_scope_items(&with_db);
    let completions = ctx.take_items();

    assert!(!completions.is_empty());
    // Should contain the function, but check if it has USING directive
    assert!(format!("{completions:?}").contains("fn_in_ns"));
    // Should have USING ns1 in the additional_text_edits
    assert!(format!("{completions:?}").contains("USING ns1"));
}

/// Test completion with multiple variable sections
#[rstest]
pub fn body_completion_with_multiple_var_sections(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION complex_fn
    VAR_INPUT
        input_x: INT;
    END_VAR_INPUT

    VAR_OUTPUT
        output_y: INT;
    END_VAR_OUTPUT

    VAR
        temp: INT;
    END_VAR

    temp := input_x + 1;
    output_y := temp;
END_FUNCTION
"#;

    add_sources(&mut with_db, &[source]);
    let pou = find_pou_with_name(&with_db, *with_db.get_files().iter().last().unwrap(), "complex_fn")
        .unwrap();

    let offset = source.find("temp := input_x").unwrap();
    let mut ctx = ScopeCompletionCtx::new(QueryMode::Body, pou.get_scope_id(&with_db), offset, "");
    ctx.query_scope_items(&with_db);
    let completions = ctx.take_items();

    assert!(!completions.is_empty());
    // Should have access to all variable sections
    assert!(format!("{completions:?}").contains("input_x"));
    assert!(format!("{completions:?}").contains("output_y"));
    assert!(format!("{completions:?}").contains("temp"));
}
