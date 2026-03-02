use auto_lsp::{default::db::BaseDatabase, lsp_types::Url};
use db::RootDatabase;
use hir::HirNodeInfo;
use ide_proto::{
    handlers::{CompletionHandler, CompletionRequest, completions_utils::{CompletionCtx, QueryMode}},
    walk::completion_descendant_at,
};
use rstest::rstest;

use crate::tests::utils::{add_sources, find_pou_with_name, with_db};

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

#[rstest]
pub fn head_namespace_root_fragment(mut with_db: RootDatabase) {
    // Typing a type spec in a VAR section should show namespace root fragments
    let source = r#"
NAMESPACE System
    FUNCTION_BLOCK Controller
    END_FUNCTION_BLOCK
END_NAMESPACE

FUNCTION fn1
VAR
    x : INT;
END_VAR
END_FUNCTION
"#;

    add_sources(&mut with_db, &[source]);
    let pou = find_pou_with_name(
        &with_db,
        *with_db.get_files().iter().last().unwrap(),
        "fn1",
    )
    .unwrap();

    let offset = source.find("x : INT").unwrap() + 1;
    let mut ctx = CompletionCtx::new(offset, QueryMode::Head);
    ctx.scope_completion(pou.get_scope_id(&with_db), "", &with_db);
    let completions = ctx.take_items();

    assert!(
        format!("{completions:?}").contains("System"),
        "expected 'System' namespace in head completions: {completions:?}"
    );
    assert!(
        format!("{completions:?}").contains("Controller"),
        "expected 'Controller' FB in head completions: {completions:?}"
    );
}

#[rstest]
pub fn head_namespace_filtered_by_query(mut with_db: RootDatabase) {
    // Typing `Sys` in a type spec should filter to show only matching namespaces
    let source = r#"
NAMESPACE System
    FUNCTION_BLOCK Controller
    END_FUNCTION_BLOCK
END_NAMESPACE

NAMESPACE Other
    FUNCTION_BLOCK Widget
    END_FUNCTION_BLOCK
END_NAMESPACE

FUNCTION fn1
VAR
    x : INT;
END_VAR
END_FUNCTION
"#;

    add_sources(&mut with_db, &[source]);
    let pou = find_pou_with_name(
        &with_db,
        *with_db.get_files().iter().last().unwrap(),
        "fn1",
    )
    .unwrap();

    let offset = source.find("x : INT").unwrap() + 1;
    let mut ctx = CompletionCtx::new(offset, QueryMode::Head);
    ctx.scope_completion(pou.get_scope_id(&with_db), "S", &with_db);
    let completions = ctx.take_items();

    assert!(
        format!("{completions:?}").contains("System"),
        "expected 'System' namespace in head completions: {completions:?}"
    );
    // "Other" should not appear with "S" query
    assert!(
        !format!("{completions:?}").contains("Other"),
        "should NOT contain 'Other' with 'S' query: {completions:?}"
    );
}

#[rstest]
pub fn body_namespace_root_in_scope(mut with_db: RootDatabase) {
    // Typing in a body should show namespace root fragments in scope completion
    let source = r#"
NAMESPACE System
    FUNCTION Sin : REAL
    VAR_INPUT
        x : REAL;
    END_VAR
    END_FUNCTION
END_NAMESPACE

FUNCTION fn1
VAR
    x : REAL;
END_VAR
END_FUNCTION
"#;

    add_sources(&mut with_db, &[source]);
    let pou = find_pou_with_name(
        &with_db,
        *with_db.get_files().iter().last().unwrap(),
        "fn1",
    )
    .unwrap();

    let offset = source.find("x : REAL;\nEND_VAR").unwrap() + "x : REAL;\nEND_VAR\n".len();
    let mut ctx = CompletionCtx::new(offset, QueryMode::Body);
    ctx.scope_completion(pou.get_scope_id(&with_db), "", &with_db);
    let completions = ctx.take_items();

    assert!(
        format!("{completions:?}").contains("System"),
        "expected 'System' namespace in body scope completions: {completions:?}"
    );
}
