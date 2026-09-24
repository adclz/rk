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
    assert!(labels.contains(&"RESOURCE"), "missing RESOURCE: {labels:?}");
    assert!(
        labels.contains(&"VAR_ACCESS"),
        "missing VAR_ACCESS: {labels:?}"
    );

    // Tasks and program instances live in a RESOURCE (E1404), so offering
    // them here would insert code the compiler rejects.
    assert!(!labels.contains(&"TASK"), "config should not offer TASK");
    assert!(
        !labels.contains(&"PROGRAM (config)"),
        "config should not offer PROGRAM (config)"
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
    // A RESOURCE holds no variables: VAR_GLOBAL is CONFIGURATION-level (E0021).
    assert!(
        !labels.contains(&"VAR_GLOBAL"),
        "resource should not offer VAR_GLOBAL"
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
    assert!(labels.contains(&"RESOURCE"), "missing RESOURCE: {labels:?}");
    assert!(
        labels.contains(&"VAR_ACCESS"),
        "missing VAR_ACCESS: {labels:?}"
    );
}

/// Test completion inside an empty TASK init — all three params offered
#[rstest]
pub fn task_config_completion_empty(mut with_db: RootDatabase) {
    let source = r#"
CONFIGURATION MyConfig
    RESOURCE Res ON CPU
        TASK t1()
    END_RESOURCE
END_CONFIGURATION
"#;

    add_sources(&mut with_db, &[source]);
    let file = *with_db.get_files().iter().last().unwrap();

    let offset = source.find("t1(").unwrap() + "t1(".len();
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
    assert!(labels.contains(&"SINGLE"), "missing SINGLE: {labels:?}");
    assert!(labels.contains(&"INTERVAL"), "missing INTERVAL: {labels:?}");
    assert!(labels.contains(&"PRIORITY"), "missing PRIORITY: {labels:?}");
}

/// Test completion inside TASK with PRIORITY already set — only SINGLE and INTERVAL offered
#[rstest]
pub fn task_config_completion_with_priority(mut with_db: RootDatabase) {
    let source = r#"
CONFIGURATION MyConfig
    RESOURCE Res ON CPU
        TASK t1(PRIORITY := 5)
    END_RESOURCE
END_CONFIGURATION
"#;

    add_sources(&mut with_db, &[source]);
    let file = *with_db.get_files().iter().last().unwrap();

    let offset = source.find(')').unwrap();
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
    assert!(labels.contains(&"SINGLE"), "missing SINGLE: {labels:?}");
    assert!(labels.contains(&"INTERVAL"), "missing INTERVAL: {labels:?}");
    assert!(
        !labels.contains(&"PRIORITY"),
        "should not offer PRIORITY: {labels:?}"
    );
}

/// Test completion inside a fully specified TASK — no params offered
#[rstest]
pub fn task_config_completion_full(mut with_db: RootDatabase) {
    let source = r#"
CONFIGURATION MyConfig
    RESOURCE Res ON CPU
        TASK t1(SINGLE := %IX0.0, INTERVAL := T#20ms, PRIORITY := 3)
    END_RESOURCE
END_CONFIGURATION
"#;

    add_sources(&mut with_db, &[source]);
    let file = *with_db.get_files().iter().last().unwrap();

    let offset = source.find(')').unwrap();
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
    assert!(labels.is_empty(), "should have no completions: {labels:?}");
}

/// Inside a program configuration's list: the program's inputs, outputs and
/// function blocks where an element is named, a VAR_GLOBAL of the element's
/// type after `:=` or `=>`, and a task of the resource after `WITH`.
#[rstest]
#[case::an_empty_list("|", &["x1", "x2", "y1", "fb1"], &["n", "w"])]
#[case::after_an_element("x1 := b, |", &["x2", "y1", "fb1"], &["n", "w"])]
#[case::a_name_being_typed("x|", &["x1", "x2"], &["w"])]
#[case::a_source("x2 := |", &["w", "total"], &["b", "x1"])]
#[case::a_sink("y1 => |", &["w", "total"], &["b", "y1"])]
#[case::a_task("fb1 WITH |", &["T", "FAST"], &["w", "x1"])]
pub fn completion_in_a_program_configuration(
    mut with_db: RootDatabase,
    #[case] list: &str,
    #[case] offered: &[&str],
    #[case] not_offered: &[&str],
) {
    let source = format!(
        r#"
FUNCTION_BLOCK Counter
VAR_OUTPUT c : INT; END_VAR
END_FUNCTION_BLOCK

PROGRAM F
VAR_INPUT x1 : BOOL; x2 : UINT; END_VAR
VAR_OUTPUT y1 : UINT; END_VAR
VAR fb1 : Counter; n : INT; END_VAR
END_PROGRAM

CONFIGURATION Cfg
VAR_GLOBAL w : UINT; b : BOOL; total : UINT; END_VAR
    RESOURCE Res ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        TASK FAST(INTERVAL := T#5ms, PRIORITY := 0);
        PROGRAM P1 WITH T : F({list});
    END_RESOURCE
END_CONFIGURATION
"#
    );
    let offset = source.find('|').unwrap();
    let source = source.replacen('|', "", 1);
    add_sources(&mut with_db, &[&source]);
    let file = *with_db.get_files().iter().last().unwrap();

    let items = ide_proto::handlers::completions::complete(&with_db, file, offset, None);
    let labels: Vec<&str> = items.iter().map(|c| c.label.as_str()).collect();
    for label in offered {
        assert!(labels.contains(label), "missing `{label}`: {labels:?}");
    }
    for label in not_offered {
        assert!(!labels.contains(label), "offered `{label}`: {labels:?}");
    }
}

/// Inside VAR_CONFIG, the entry being written: a resource or a program
/// instance where a path starts, the instances of a resource after it, the
/// members of whatever the path reached, and the variable's own type after
/// the colon. It offered the CONFIGURATION's sections instead.
#[rstest]
#[case::an_empty_section("|", &["Res", "P1"], &["VAR_GLOBAL", "x"])]
#[case::after_an_entry("Res.P1.x AT %QW0 : INT;\n    |", &["Res", "P1"], &["VAR_GLOBAL"])]
#[case::after_a_resource("Res.|", &["P1"], &["Res", "x"])]
#[case::after_an_instance("Res.P1.|", &["x", "d"], &["P1", "out"])]
#[case::an_instance_without_its_resource("P1.|", &["x", "d"], &["P1"])]
#[case::through_a_member("Res.P1.d.|", &["out"], &["x", "d"])]
#[case::a_member_being_typed("Res.P1.o|", &["x", "d"], &["P1"])]
#[case::the_type("Res.P1.x AT %QW0 : |", &["INT"], &["x", "VAR_GLOBAL"])]
pub fn completion_in_var_config(
    mut with_db: RootDatabase,
    #[case] entry: &str,
    #[case] offered: &[&str],
    #[case] not_offered: &[&str],
) {
    let source = format!(
        r#"
FUNCTION_BLOCK Drive
VAR out AT %Q* : INT; END_VAR
END_FUNCTION_BLOCK

PROGRAM F
VAR x AT %Q* : INT; END_VAR
VAR d : Drive; END_VAR
END_PROGRAM

CONFIGURATION Cfg
VAR_CONFIG
    {entry}
END_VAR
    RESOURCE Res ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH T : F;
    END_RESOURCE
END_CONFIGURATION
"#
    );
    let offset = source.find('|').unwrap();
    let source = source.replacen('|', "", 1);
    add_sources(&mut with_db, &[&source]);
    let file = *with_db.get_files().iter().last().unwrap();

    let items = ide_proto::handlers::completions::complete(&with_db, file, offset, None);
    let labels: Vec<&str> = items.iter().map(|c| c.label.as_str()).collect();
    for label in offered {
        assert!(labels.contains(label), "missing `{label}`: {labels:?}");
    }
    for label in not_offered {
        assert!(!labels.contains(label), "offered `{label}`: {labels:?}");
    }
}
