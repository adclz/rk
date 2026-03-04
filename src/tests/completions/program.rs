use auto_lsp::default::db::BaseDatabase;
use db::RootDatabase;
use hir::{HasName, hir_def::semantic_index::semantic_index};
use ide_proto::handlers::completions_utils::{CompletionCtx, QueryMode};
use ide_proto::handlers::{CompletionHandler, CompletionRequest};
use ide_proto::walk::completion_descendant_at;
use rstest::rstest;

use crate::tests::utils::{add_sources, with_db};

/// Test body completion inside an empty PROGRAM body (variables + statement keywords)
#[rstest]
pub fn program_body_completion(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM MyProg
    VAR
        my_var: INT;
        another_var: REAL;
    END_VAR

END_PROGRAM
"#;

    add_sources(&mut with_db, &[source]);
    let file = *with_db.get_files().iter().last().unwrap();

    // Position in the empty body area — hits the ProgramDecl itself
    let offset = source.find("END_PROGRAM").unwrap() - 1;
    let (node, node_key, is_last_before) =
        completion_descendant_at(&with_db, file, offset).unwrap();
    let req = CompletionRequest {
        offset,
        trigger_character: None,
        query: "".into(),
        node_index_pos: Some(node_key),
        is_last_before,
    };
    let completions = node.completion(&with_db, &req).unwrap();

    let labels: Vec<&str> = completions.iter().map(|c| c.label.as_str()).collect();

    // Should have variables in scope
    assert!(labels.contains(&"my_var"), "missing my_var: {labels:?}");
    assert!(
        labels.contains(&"another_var"),
        "missing another_var: {labels:?}"
    );

    // Should have statement keywords
    assert!(labels.contains(&"IF"), "missing IF: {labels:?}");
    assert!(labels.contains(&"FOR"), "missing FOR: {labels:?}");
    assert!(labels.contains(&"WHILE"), "missing WHILE: {labels:?}");
    assert!(labels.contains(&"REPEAT"), "missing REPEAT: {labels:?}");
}

/// Test head completion in empty PROGRAM (var section snippets)
#[rstest]
pub fn program_head_completion_empty(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM MyProg

END_PROGRAM
"#;

    add_sources(&mut with_db, &[source]);
    let file = *with_db.get_files().iter().last().unwrap();

    // Position inside the empty program (before END_PROGRAM)
    let offset = source.find("END_PROGRAM").unwrap() - 1;
    let (node, node_key, is_last_before) =
        completion_descendant_at(&with_db, file, offset).unwrap();
    let req = CompletionRequest {
        offset,
        trigger_character: None,
        query: "".into(),
        node_index_pos: Some(node_key),
        is_last_before,
    };
    let completions = node.completion(&with_db, &req).unwrap();

    let labels: Vec<&str> = completions.iter().map(|c| c.label.as_str()).collect();

    // Should have var section snippets
    assert!(
        labels.contains(&"VAR_INPUT"),
        "missing VAR_INPUT: {labels:?}"
    );
    assert!(
        labels.contains(&"VAR_OUTPUT"),
        "missing VAR_OUTPUT: {labels:?}"
    );
    assert!(
        labels.contains(&"VAR_IN_OUT"),
        "missing VAR_IN_OUT: {labels:?}"
    );
    assert!(labels.contains(&"VAR_TEMP"), "missing VAR_TEMP: {labels:?}");
    assert!(labels.contains(&"VAR"), "missing VAR: {labels:?}");

    // Should NOT have METHOD or EXTENDS/IMPLEMENTS
    assert!(
        !labels.contains(&"METHOD"),
        "programs should not offer METHOD"
    );
    assert!(
        !labels.contains(&"EXTENDS"),
        "programs should not offer EXTENDS"
    );
    assert!(
        !labels.contains(&"IMPLEMENTS"),
        "programs should not offer IMPLEMENTS"
    );
}

/// Test that scope_completion finds variables from a PROGRAM scope
#[rstest]
pub fn program_scope_completion(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM MyProg
    VAR
        x_count: INT;
        y_value: REAL;
    END_VAR

END_PROGRAM
"#;

    add_sources(&mut with_db, &[source]);
    let file = *with_db.get_files().iter().last().unwrap();
    let sema = semantic_index(&with_db, file);

    let prog = sema
        .programs
        .iter()
        .find(|p| p.get_name_ident(&with_db).text(&with_db).as_str() == "MyProg")
        .unwrap();

    let offset = source.find("END_PROGRAM").unwrap();
    let mut ctx = CompletionCtx::new(offset, QueryMode::Body);
    ctx.scope_completion(prog.scope_id(&with_db), "", &with_db);
    let completions = ctx.take_items();

    assert!(!completions.is_empty());
    assert!(
        format!("{completions:?}").contains("x_count"),
        "missing x_count"
    );
    assert!(
        format!("{completions:?}").contains("y_value"),
        "missing y_value"
    );
}
