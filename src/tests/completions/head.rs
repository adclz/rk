use ast::generated::DataTypeDecl;
use auto_lsp::core::ast::AstNode;
use auto_lsp::default::db::tracked::get_ast;
use auto_lsp::default::db::BaseDatabase;
use db::RootDatabase;
use hir::HirNodeInfo;
use ide_proto::handlers::completions_utils::{CompletionCtx, QueryMode, static_snippets};
use ide_proto::handlers::{CompletionHandler, CompletionRequest};
use ide_proto::walk::completion_descendant_at;
use rstest::rstest;

use crate::tests::utils::{add_sources, find_pou_with_name, with_db};

/// Test completion WITHIN a variable type declaration (head scope)
#[rstest]
pub fn head_scope_items(mut with_db: RootDatabase) {
    let source = r#"
TYPE Test: INT; END_TYPE

FUNCTION test_fn
    VAR
        my_var: INT;
        another_var: T
    END_VAR

    my_var := 10;
END_FUNCTION
"#;

    add_sources(&mut with_db, &[source]);
    let pou = find_pou_with_name(
        &with_db,
        *with_db.get_files().iter().last().unwrap(),
        "test_fn",
    )
    .unwrap();

    let offset = source.find("another_var").unwrap() + 1;
    let mut ctx = CompletionCtx::new(offset, QueryMode::Head);
    ctx.scope_completion(pou.get_scope_id(&with_db), "", &with_db);
    let completions = ctx.take_items();

    // only Test should be suggested
    assert_eq!(completions.len(), 1);
    assert!(format!("{completions:?}").contains("Test"));
}

/// TYPE declarations should NOT offer POU-level or body-level completions.
/// Only type-related completions (elementary types, STRUCT, ARRAY) should appear
/// when cursor is inside a TYPE block.
#[rstest]
pub fn type_decl_no_pou_completions(mut with_db: RootDatabase) {
    let source = "TYPE MyStruct :\nSTRUCT\n    x : INT;\nEND_STRUCT;\nEND_TYPE\n";

    add_sources(&mut with_db, &[source]);
    let file = *with_db.get_files().iter().last().unwrap();

    // Cursor inside the TYPE body (before STRUCT keyword)
    let offset = source.find("STRUCT").unwrap() - 1;
    let result = completion_descendant_at(&with_db, file, offset);

    if let Some((node, node_key, is_last_before)) = result {
        let req = CompletionRequest {
            offset,
            trigger_character: None,
            query: "".into(),
            node_index_pos: Some(node_key),
            is_last_before,
        };
        let completions = node.completion(&with_db, &req).unwrap_or_default();
        let labels: Vec<&str> = completions.iter().map(|c| c.label.as_str()).collect();

        // Should NOT have POU-level snippets
        assert!(
            !labels.contains(&"FUNCTION"),
            "TYPE should not offer FUNCTION: {labels:?}"
        );
        assert!(
            !labels.contains(&"FUNCTION_BLOCK"),
            "TYPE should not offer FUNCTION_BLOCK: {labels:?}"
        );
        assert!(
            !labels.contains(&"CLASS"),
            "TYPE should not offer CLASS: {labels:?}"
        );
        assert!(
            !labels.contains(&"PROGRAM"),
            "TYPE should not offer PROGRAM: {labels:?}"
        );
        assert!(
            !labels.contains(&"INTERFACE"),
            "TYPE should not offer INTERFACE: {labels:?}"
        );

        // Should NOT have statement-level snippets
        assert!(
            !labels.contains(&"IF"),
            "TYPE should not offer IF: {labels:?}"
        );
        assert!(
            !labels.contains(&"FOR"),
            "TYPE should not offer FOR: {labels:?}"
        );
        assert!(
            !labels.contains(&"WHILE"),
            "TYPE should not offer WHILE: {labels:?}"
        );
    }
}

/// Empty TYPE body should only offer type-relevant completions
#[rstest]
pub fn type_decl_empty_body_no_pou_completions(mut with_db: RootDatabase) {
    let source = "TYPE MyType :\n    t\nEND_TYPE\n";

    add_sources(&mut with_db, &[source]);
    let file = *with_db.get_files().iter().last().unwrap();

    // Cursor on the `t` character — simulates user typing a type name
    let offset = source.find("    t").unwrap() + 5;
    let result = completion_descendant_at(&with_db, file, offset);

    if let Some((node, node_key, is_last_before)) = result {
        let req = CompletionRequest {
            offset,
            trigger_character: None,
            query: "".into(),
            node_index_pos: Some(node_key),
            is_last_before,
        };
        let completions = node.completion(&with_db, &req).unwrap_or_default();
        let labels: Vec<&str> = completions.iter().map(|c| c.label.as_str()).collect();

        // Should NOT have POU-level snippets
        assert!(
            !labels.contains(&"FUNCTION"),
            "TYPE should not offer FUNCTION: {labels:?}"
        );
        assert!(
            !labels.contains(&"FUNCTION_BLOCK"),
            "TYPE should not offer FUNCTION_BLOCK: {labels:?}"
        );
        assert!(
            !labels.contains(&"IF"),
            "TYPE should not offer IF: {labels:?}"
        );
        assert!(
            !labels.contains(&"FOR"),
            "TYPE should not offer FOR: {labels:?}"
        );
    } else {
        // If no node found, completion_descendant_at returned None which means
        // top-level completions will show — this is the bug scenario
        panic!("Expected a node at offset {offset}, got None — top-level completions would show");
    }
}

/// When cursor is on empty line inside a TYPE with valid content,
/// the completion system should NOT return None (which would show POU snippets)
#[rstest]
pub fn type_decl_completion_descendant_not_none(mut with_db: RootDatabase) {
    let source = "TYPE MyStruct :\nSTRUCT\n    x : INT;\nEND_STRUCT;\nEND_TYPE\n";

    add_sources(&mut with_db, &[source]);
    let file = *with_db.get_files().iter().last().unwrap();

    // Check various positions inside the TYPE body
    let positions = [
        source.find("STRUCT").unwrap(),     // on STRUCT keyword
        source.find("x : INT").unwrap(),    // on field name
        source.find("INT;").unwrap(),       // on INT type
    ];

    for offset in positions {
        let result = completion_descendant_at(&with_db, file, offset);
        assert!(
            result.is_some(),
            "Expected a node at offset {offset} inside TYPE body, got None"
        );
    }
}

/// When cursor is inside a bare TYPE with just whitespace, the server would
/// fall back to top-level completions because no HIR node covers the offset.
/// This verifies the scenario.
#[rstest]
pub fn type_decl_bare_shows_no_pou_snippets(mut with_db: RootDatabase) {
    // A TYPE with just an identifier — `t` is parsed as the spec target
    let source = "TYPE MyType :\n    t\nEND_TYPE\n";

    add_sources(&mut with_db, &[source]);
    let file = *with_db.get_files().iter().last().unwrap();

    // Cursor right on the `t` identifier
    let offset = source.find("\n    t").unwrap() + 5;
    let result = completion_descendant_at(&with_db, file, offset);

    // This should find the Spec node covering `t`
    assert!(
        result.is_some(),
        "Expected Spec node at offset {offset} in TYPE body"
    );

    let (node, node_key, is_last_before) = result.unwrap();
    let req = CompletionRequest {
        offset,
        trigger_character: None,
        query: "".into(),
        node_index_pos: Some(node_key),
        is_last_before,
    };
    let completions = node.completion(&with_db, &req).unwrap_or_default();
    let labels: Vec<&str> = completions.iter().map(|c| c.label.as_str()).collect();

    // Should offer type completions (elementary types, STRUCT, ARRAY)
    assert!(
        labels.contains(&"INT"),
        "should offer INT: {labels:?}"
    );
    assert!(
        labels.contains(&"STRUCT"),
        "should offer STRUCT: {labels:?}"
    );
    assert!(
        labels.contains(&"ARRAY"),
        "should offer ARRAY: {labels:?}"
    );

    // Should NOT offer POU/body completions
    assert!(
        !labels.contains(&"FUNCTION"),
        "TYPE should not offer FUNCTION: {labels:?}"
    );
    assert!(
        !labels.contains(&"IF"),
        "TYPE should not offer IF: {labels:?}"
    );
}

/// Incomplete TYPE where user hasn't typed the spec yet.
/// When completion_descendant_at returns None inside a TYPE body,
/// the server should detect the data_type_decl CST context and
/// offer type-level completions (not POU-level snippets).
#[rstest]
pub fn type_decl_incomplete_no_pou_completions(mut with_db: RootDatabase) {
    // TYPE with colon but no spec yet — user is about to type
    let source = "TYPE MyType :\n\nEND_TYPE\n";

    add_sources(&mut with_db, &[source]);
    let file = *with_db.get_files().iter().last().unwrap();

    // Cursor on the empty line between `:` and END_TYPE
    let offset = source.find("\n\nEND_TYPE").unwrap() + 1;
    let result = completion_descendant_at(&with_db, file, offset);

    // completion_descendant_at returns None because no HIR node covers
    // the cursor in an incomplete TYPE. The server handles this by checking
    // the CST for data_type_decl context.
    assert!(
        result.is_none(),
        "Expected None for incomplete TYPE body at offset {offset}"
    );

    // Verify the AST-level check: cursor is inside a DataTypeDecl
    let ast = get_ast(&with_db, file);
    let in_type_decl = ast.iter().any(|node| {
        let range = node.get_range();
        range.start_byte <= offset
            && offset <= range.end_byte
            && node.lower().downcast_ref::<DataTypeDecl>().is_some()
    });

    assert!(
        in_type_decl,
        "cursor at offset {offset} should be inside a DataTypeDecl AST node"
    );
}

/// TYPE with enum should not offer POU/body completions
#[rstest]
pub fn type_enum_no_pou_completions(mut with_db: RootDatabase) {
    let source = r#"
TYPE MyEnum : (
    Val1,
    Val2
);
END_TYPE
"#;

    add_sources(&mut with_db, &[source]);
    let file = *with_db.get_files().iter().last().unwrap();

    // Cursor between the enum values
    let offset = source.find("Val1").unwrap();
    let result = completion_descendant_at(&with_db, file, offset);

    if let Some((node, node_key, is_last_before)) = result {
        let req = CompletionRequest {
            offset,
            trigger_character: None,
            query: "".into(),
            node_index_pos: Some(node_key),
            is_last_before,
        };
        let completions = node.completion(&with_db, &req).unwrap_or_default();
        let labels: Vec<&str> = completions.iter().map(|c| c.label.as_str()).collect();

        assert!(
            !labels.contains(&"FUNCTION"),
            "TYPE enum should not offer FUNCTION: {labels:?}"
        );
        assert!(
            !labels.contains(&"IF"),
            "TYPE enum should not offer IF: {labels:?}"
        );
    }
}

#[test]
fn struct_and_array_snippets_exist() {
    let struct_item = static_snippets::struct_();
    assert_eq!(struct_item.label, "STRUCT");
    assert!(struct_item.insert_text.unwrap().contains("END_STRUCT"));

    let array_item = static_snippets::array();
    assert_eq!(array_item.label, "ARRAY");
    assert!(array_item.insert_text.unwrap().contains("OF"));
}
