use auto_lsp::default::db::BaseDatabase;
use db::RootDatabase;
use hir::HirNodeInfo;
use ide_proto::handlers::completions_utils::{CompletionCtx, QueryMode};
use ide_proto::handlers::{CompletionHandler, CompletionRequest};
use ide_proto::walk::completion_descendant_at;
use rstest::rstest;

use crate::tests::utils::{add_source, add_sources, find_pou_with_name, with_db};

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

    let file = add_source(&mut with_db, source);
    let pou = find_pou_with_name(&with_db, file, "test_fn").unwrap();

    // Test completion at line with assignment (before the path expression starts)
    let offset = source.find("END_VAR").unwrap() + 7; // Position after END_VAR, before any statement
    let mut ctx = CompletionCtx::new(offset, QueryMode::Body);
    ctx.scope_completion(pou.get_scope_id(&with_db), "", &with_db);
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

    let file = add_source(&mut with_db, source);
    let pou = find_pou_with_name(&with_db, file, "test_fn").unwrap();

    // Test completion INSIDE the variable name itself (within PathExpr/VariableAccess)
    let offset = source.find("my_var := 10").unwrap() + 1; // Position inside "my_var"
    let mut ctx = CompletionCtx::new(offset, QueryMode::Body);
    ctx.scope_completion(pou.get_scope_id(&with_db), "", &with_db);
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

    let file = add_source(&mut with_db, source);
    let pou = find_pou_with_name(&with_db, file, "test_fn").unwrap();

    // Test completion after the variable (at the space after "my_var")
    let offset = source.find("my_var :=").unwrap() + 6; // Position just after "my_var"
    let mut ctx = CompletionCtx::new(offset, QueryMode::Body);
    ctx.scope_completion(pou.get_scope_id(&with_db), "", &with_db);
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

    let file = add_source(&mut with_db, source);
    let pou = find_pou_with_name(&with_db, file, "my_fb").unwrap();

    // Get completions in function block (which is the scope)
    let offset = source.find("END_FUNCTION_BLOCK").unwrap();
    let mut ctx = CompletionCtx::new(offset, QueryMode::Body);
    ctx.scope_completion(pou.get_scope_id(&with_db), "", &with_db);
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

    let file = add_source(&mut with_db, source);
    let pou = find_pou_with_name(&with_db, file, "main").unwrap();

    let offset = source.find("fn_in_ns()").unwrap();
    let mut ctx = CompletionCtx::new(offset, QueryMode::Body);
    ctx.scope_completion(pou.get_scope_id(&with_db), "", &with_db);
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

    let file = add_source(&mut with_db, source);
    let pou = find_pou_with_name(&with_db, file, "complex_fn").unwrap();

    let offset = source.find("temp := input_x").unwrap();
    let mut ctx = CompletionCtx::new(offset, QueryMode::Body);
    ctx.scope_completion(pou.get_scope_id(&with_db), "", &with_db);
    let completions = ctx.take_items();

    assert!(!completions.is_empty());
    // Should have access to all variable sections
    assert!(format!("{completions:?}").contains("input_x"));
    assert!(format!("{completions:?}").contains("output_y"));
    assert!(format!("{completions:?}").contains("temp"));
}

/// Function with a return type should suggest its own name for return value assignment.
#[rstest]
pub fn function_self_return_completion(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION my_func : INT
VAR
    x : INT;
END_VAR

END_FUNCTION
"#;

    let file = add_source(&mut with_db, source);

    // Cursor inside the function body (blank line before END_FUNCTION)
    let offset = source.find("END_FUNCTION").unwrap() - 1;

    let (node, idx, is_last_before) = completion_descendant_at(&with_db, file, offset).unwrap();
    let req = CompletionRequest {
        offset,
        trigger_character: None,
        query: String::new(),
        node_index_pos: Some(idx),
        is_last_before,
    };
    let completions = node.completion(&with_db, &req).unwrap();
    let debug = format!("{completions:?}");

    // Should contain the function name with (Self) description
    assert!(
        debug.contains("my_func"),
        "should suggest function name for return value"
    );
    assert!(debug.contains("Self"), "should mark as Self");
}

/// Function WITHOUT a return type should NOT suggest its own name.
#[rstest]
pub fn function_no_return_type_no_self_completion(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION my_void_func
VAR
    x : INT;
END_VAR

END_FUNCTION
"#;

    let file = add_source(&mut with_db, source);

    let offset = source.find("END_FUNCTION").unwrap() - 1;

    let (node, idx, is_last_before) = completion_descendant_at(&with_db, file, offset).unwrap();
    let req = CompletionRequest {
        offset,
        trigger_character: None,
        query: String::new(),
        node_index_pos: Some(idx),
        is_last_before,
    };
    let completions = node.completion(&with_db, &req).unwrap();
    let debug = format!("{completions:?}");

    // Should NOT suggest the function name as Self
    assert!(
        !debug.contains("(Self)"),
        "should not suggest Self for void function"
    );
}

/// Method with a return type should suggest its own name for return value assignment.
#[rstest]
pub fn method_self_return_completion(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK my_fb
METHOD my_method : INT
VAR
    x : INT;
END_VAR

END_METHOD
END_FUNCTION_BLOCK
"#;

    let file = add_source(&mut with_db, source);

    // Cursor inside the method body
    let offset = source.find("END_METHOD").unwrap() - 1;

    let (node, idx, is_last_before) = completion_descendant_at(&with_db, file, offset).unwrap();
    let req = CompletionRequest {
        offset,
        trigger_character: None,
        query: String::new(),
        node_index_pos: Some(idx),
        is_last_before,
    };
    let completions = node.completion(&with_db, &req).unwrap();
    let debug = format!("{completions:?}");

    assert!(
        debug.contains("my_method"),
        "should suggest method name for return value"
    );
    assert!(debug.contains("Self"), "should mark as Self");
}

/// Method WITHOUT a return type should NOT suggest its own name.
#[rstest]
pub fn method_no_return_type_no_self_completion(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK my_fb
METHOD my_void_method
VAR
    x : INT;
END_VAR

END_METHOD
END_FUNCTION_BLOCK
"#;

    let file = add_source(&mut with_db, source);

    let offset = source.find("END_METHOD").unwrap() - 1;

    let (node, idx, is_last_before) = completion_descendant_at(&with_db, file, offset).unwrap();
    let req = CompletionRequest {
        offset,
        trigger_character: None,
        query: String::new(),
        node_index_pos: Some(idx),
        is_last_before,
    };
    let completions = node.completion(&with_db, &req).unwrap();
    let debug = format!("{completions:?}");

    // Should NOT suggest the method name as Self
    assert!(
        !debug.contains("(Self)"),
        "should not suggest Self for void method"
    );
}

/// Function with no VAR sections should still provide body completions (scope items, statements).
#[rstest]
pub fn function_no_vars_body_completion(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION my_func : INT
    my;
END_FUNCTION
"#;

    let file = add_source(&mut with_db, source);

    let offset = source.find("my;").unwrap();

    let (node, idx, is_last_before) = completion_descendant_at(&with_db, file, offset).unwrap();
    let req = CompletionRequest {
        offset,
        trigger_character: None,
        query: String::new(),
        node_index_pos: Some(idx),
        is_last_before,
    };
    let completions = node.completion(&with_db, &req).unwrap();
    let debug = format!("{completions:?}");

    // Should have body completions (statements, self-return) not just VAR snippets
    assert!(
        debug.contains("my_func"),
        "should suggest function name for return value"
    );
    assert!(debug.contains("Self"), "should mark as Self");
    assert!(debug.contains("IF"), "should suggest statements like IF");
}

/// When the cursor is on an unresolved identifier (PathExpr with Type::Never), the self-return item should still appear.
#[rstest]
pub fn function_self_return_on_unresolved_path(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION my_func : INT
VAR
    x : INT;
END_VAR
    my;
END_FUNCTION
"#;

    let file = add_source(&mut with_db, source);

    // Cursor on "my" — an unresolved identifier that hits the PathExpr fallback path
    let offset = source.find("my;").unwrap();

    let (node, idx, is_last_before) = completion_descendant_at(&with_db, file, offset).unwrap();
    let req = CompletionRequest {
        offset,
        trigger_character: None,
        query: String::new(),
        node_index_pos: Some(idx),
        is_last_before,
    };
    let completions = node.completion(&with_db, &req).unwrap();
    let debug = format!("{completions:?}");

    assert!(
        debug.contains("my_func"),
        "should suggest function name even on PathExpr"
    );
    assert!(debug.contains("Self"), "should mark as Self");
}

/// When the cursor is on an unresolved identifier inside a method, the self-return item should appear.
#[rstest]
pub fn method_self_return_on_unresolved_path(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK my_fb
METHOD my_method : INT
VAR
    x : INT;
END_VAR
    my;
END_METHOD
END_FUNCTION_BLOCK
"#;

    let file = add_source(&mut with_db, source);

    // Cursor on "my" — unresolved identifier in method body
    let offset = source.find("my;").unwrap();

    let (node, idx, is_last_before) = completion_descendant_at(&with_db, file, offset).unwrap();
    let req = CompletionRequest {
        offset,
        trigger_character: None,
        query: String::new(),
        node_index_pos: Some(idx),
        is_last_before,
    };
    let completions = node.completion(&with_db, &req).unwrap();
    let debug = format!("{completions:?}");

    assert!(
        debug.contains("my_method"),
        "should suggest method name even on PathExpr"
    );
    assert!(debug.contains("Self"), "should mark as Self");
}

/// Cursor after END_FUNCTION should NOT resolve to a node inside the function.
/// It should return None so the server shows top-level (POU) snippets.
#[rstest]
pub fn completion_after_end_function_returns_none(mut with_db: RootDatabase) {
    let sources: &[&str] = &[r#"
FUNCTION fn2

    test_fn();

END_FUNCTION


"#];

    add_sources(&mut with_db, sources);
    let file = *with_db.get_files().iter().last().unwrap();

    // Cursor 2 lines after END_FUNCTION — the last HIR node in fn2 is
    // the PathExpr for `test_fn`, which must NOT be returned here.
    let offset = sources[0].find("END_FUNCTION").unwrap() + "END_FUNCTION".len() + 2;

    let result = completion_descendant_at(&with_db, file, offset);
    assert!(
        result.is_none(),
        "cursor after END_FUNCTION should not resolve to any node"
    );
}
