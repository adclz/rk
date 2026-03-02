use auto_lsp::{default::db::BaseDatabase, lsp_types::Url};
use db::RootDatabase;
use ide_proto::{
    handlers::{CompletionHandler, CompletionRequest},
    walk::completion_descendant_at,
};
use rstest::rstest;

use crate::tests::utils::{add_sources, with_db};

#[rstest]
pub fn namespace_pou_completion(mut with_db: RootDatabase) {
    // Typing `System.` inside a function body should show POUs inside the System namespace
    let ns_source = r#"
NAMESPACE System
    FUNCTION Sin : REAL
    VAR_INPUT
        x : REAL;
    END_VAR
    END_FUNCTION

    FUNCTION Cos : REAL
    VAR_INPUT
        x : REAL;
    END_VAR
    END_FUNCTION
END_NAMESPACE
"#;

    let body_source = r#"
FUNCTION fn1 : REAL
VAR
    x : REAL;
END_VAR
    x := System.
END_FUNCTION
"#;

    add_sources(&mut with_db, &[ns_source, body_source]);
    // body_source is the second file (index 1)
    let file = with_db.get_file(&Url::parse("file:///test1.st").unwrap()).unwrap();
    let offset = body_source.find("System.").unwrap() + "System.".len();
    let (node, node_key, is_last_before) = completion_descendant_at(&with_db, file, offset).unwrap();
    let req = CompletionRequest {
        offset,
        trigger_character: Some(".".into()),
        query: "".into(),
        node_index_pos: Some(node_key),
        is_last_before,
    };
    let completions = node.completion(&with_db, &req).unwrap();

    assert!(
        format!("{completions:?}").contains("Sin"),
        "expected 'Sin' in completions: {completions:?}"
    );
    assert!(
        format!("{completions:?}").contains("Cos"),
        "expected 'Cos' in completions: {completions:?}"
    );
}

#[rstest]
pub fn namespace_sub_namespace_fragments(mut with_db: RootDatabase) {
    // Typing `System.` should also show sub-namespace fragments like "Math"
    let ns_source = r#"
NAMESPACE System.Math
    FUNCTION Sin : REAL
    VAR_INPUT
        x : REAL;
    END_VAR
    END_FUNCTION
END_NAMESPACE
"#;

    let body_source = r#"
FUNCTION fn1 : REAL
VAR
    x : REAL;
END_VAR
    x := System.
END_FUNCTION
"#;

    add_sources(&mut with_db, &[ns_source, body_source]);
    let file = with_db.get_file(&Url::parse("file:///test1.st").unwrap()).unwrap();
    let offset = body_source.find("System.").unwrap() + "System.".len();
    let (node, node_key, is_last_before) = completion_descendant_at(&with_db, file, offset).unwrap();
    let req = CompletionRequest {
        offset,
        trigger_character: Some(".".into()),
        query: "".into(),
        node_index_pos: Some(node_key),
        is_last_before,
    };
    let completions = node.completion(&with_db, &req).unwrap();

    assert!(
        format!("{completions:?}").contains("Math"),
        "expected 'Math' sub-namespace fragment in completions: {completions:?}"
    );
}

#[rstest]
pub fn nested_namespace_pou_completion(mut with_db: RootDatabase) {
    // Typing `System.Math.` should show POUs inside System.Math
    let ns_source = r#"
NAMESPACE System
    FUNCTION Log : REAL
    VAR_INPUT
        x : REAL;
    END_VAR
    END_FUNCTION
END_NAMESPACE

NAMESPACE System.Math
    FUNCTION Sin : REAL
    VAR_INPUT
        x : REAL;
    END_VAR
    END_FUNCTION
END_NAMESPACE
"#;

    let body_source = r#"
FUNCTION fn1 : REAL
VAR
    x : REAL;
END_VAR
    x := System.Math.
END_FUNCTION
"#;

    add_sources(&mut with_db, &[ns_source, body_source]);
    let file = with_db.get_file(&Url::parse("file:///test1.st").unwrap()).unwrap();
    let offset = body_source.find("System.Math.").unwrap() + "System.Math.".len();
    let (node, node_key, is_last_before) = completion_descendant_at(&with_db, file, offset).unwrap();
    let req = CompletionRequest {
        offset,
        trigger_character: Some(".".into()),
        query: "".into(),
        node_index_pos: Some(node_key),
        is_last_before,
    };
    let completions = node.completion(&with_db, &req).unwrap();

    // Should show Sin from System.Math, NOT Log from System
    assert!(
        format!("{completions:?}").contains("Sin"),
        "expected 'Sin' in completions: {completions:?}"
    );
    assert!(
        !format!("{completions:?}").contains("Log"),
        "should NOT contain 'Log' from parent namespace: {completions:?}"
    );
}
