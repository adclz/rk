use auto_lsp::default::db::BaseDatabase;
use db::RootDatabase;
use ide_proto::handlers::{CompletionHandler, CompletionRequest};
use ide_proto::walk::completion_descendant_at;
use rstest::rstest;

use crate::tests::utils::{add_sources, with_db};

/// Test completion inside an empty CONFIGURATION body
#[rstest]
pub fn config_completion_empty(mut with_db: RootDatabase) {
    let source = r#"
CONFIGURATION MyConfig

END_CONFIGURATION
"#;

    add_sources(&mut with_db, &[source]);
    let file = *with_db.get_files().iter().last().unwrap();

    let offset = source.find("END_CONFIGURATION").unwrap() - 1;
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

    assert!(
        labels.contains(&"VAR_GLOBAL"),
        "missing VAR_GLOBAL: {labels:?}"
    );
    assert!(
        labels.contains(&"RESOURCE"),
        "missing RESOURCE: {labels:?}"
    );
    assert!(labels.contains(&"TASK"), "missing TASK: {labels:?}");
    assert!(
        labels.contains(&"PROGRAM (config)"),
        "missing PROGRAM (config): {labels:?}"
    );
    assert!(
        labels.contains(&"VAR_ACCESS"),
        "missing VAR_ACCESS: {labels:?}"
    );

    // Should NOT have POU-level snippets
    assert!(
        !labels.contains(&"FUNCTION"),
        "config should not offer FUNCTION"
    );
    assert!(
        !labels.contains(&"FUNCTION_BLOCK"),
        "config should not offer FUNCTION_BLOCK"
    );
    assert!(!labels.contains(&"CLASS"), "config should not offer CLASS");
}

/// Test completion inside a RESOURCE body
#[rstest]
pub fn resource_completion(mut with_db: RootDatabase) {
    let source = r#"
CONFIGURATION MyConfig
    RESOURCE MyRes ON CPU

    END_RESOURCE
END_CONFIGURATION
"#;

    add_sources(&mut with_db, &[source]);
    let file = *with_db.get_files().iter().last().unwrap();

    let offset = source.find("END_RESOURCE").unwrap() - 1;
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

    assert!(
        labels.contains(&"VAR_GLOBAL"),
        "missing VAR_GLOBAL: {labels:?}"
    );
    assert!(labels.contains(&"TASK"), "missing TASK: {labels:?}");
    assert!(
        labels.contains(&"PROGRAM (config)"),
        "missing PROGRAM (config): {labels:?}"
    );

    // RESOURCE inside RESOURCE should NOT be offered
    assert!(
        !labels.contains(&"RESOURCE"),
        "resource should not offer RESOURCE"
    );
    // VAR_ACCESS only in CONFIGURATION, not RESOURCE
    assert!(
        !labels.contains(&"VAR_ACCESS"),
        "resource should not offer VAR_ACCESS"
    );
}

/// Test completion inside a CONFIGURATION that already has VAR_GLOBAL
#[rstest]
pub fn config_completion_with_globals(mut with_db: RootDatabase) {
    let source = r#"
CONFIGURATION MyConfig
    VAR_GLOBAL
        x : INT;
    END_VAR

END_CONFIGURATION
"#;

    add_sources(&mut with_db, &[source]);
    let file = *with_db.get_files().iter().last().unwrap();

    // Position after END_VAR, still inside CONFIGURATION
    let offset = source.find("\nEND_CONFIGURATION").unwrap() - 1;
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

    // Should still offer all config-level items
    assert!(
        labels.contains(&"RESOURCE"),
        "missing RESOURCE: {labels:?}"
    );
    assert!(labels.contains(&"TASK"), "missing TASK: {labels:?}");
    assert!(
        labels.contains(&"PROGRAM (config)"),
        "missing PROGRAM (config): {labels:?}"
    );
    assert!(
        labels.contains(&"VAR_ACCESS"),
        "missing VAR_ACCESS: {labels:?}"
    );
}
