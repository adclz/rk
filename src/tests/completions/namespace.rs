use auto_lsp::{default::db::BaseDatabase, lsp_types::Url};
use db::RootDatabase;
use hir::HirNodeInfo;
use ide_proto::{
    handlers::{
        CompletionHandler, CompletionRequest,
        completions_utils::{CompletionCtx, QueryMode},
    },
    walk::{completion_descendant_at, descendant_at},
};
use rstest::rstest;

use crate::tests::utils::{add_source, add_sources, find_pou_with_name, with_db};

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
    let file = with_db
        .get_file(&Url::parse("file:///test1.st").unwrap())
        .unwrap();
    let offset = body_source.find("System.").unwrap() + "System.".len();
    let (node, node_key, is_last_before) =
        completion_descendant_at(&with_db, file, offset).unwrap();
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
    let file = with_db
        .get_file(&Url::parse("file:///test1.st").unwrap())
        .unwrap();
    let offset = body_source.find("System.").unwrap() + "System.".len();
    let (node, node_key, is_last_before) =
        completion_descendant_at(&with_db, file, offset).unwrap();
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
    let file = with_db
        .get_file(&Url::parse("file:///test1.st").unwrap())
        .unwrap();
    let offset = body_source.find("System.Math.").unwrap() + "System.Math.".len();
    let (node, node_key, is_last_before) =
        completion_descendant_at(&with_db, file, offset).unwrap();
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

    let file = add_source(&mut with_db, source);
    let pou = find_pou_with_name(&with_db, file, "fn1").unwrap();

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

    let file = add_source(&mut with_db, source);
    let pou = find_pou_with_name(&with_db, file, "fn1").unwrap();

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

    let file = add_source(&mut with_db, source);
    let pou = find_pou_with_name(&with_db, file, "fn1").unwrap();

    let offset = source.find("x : REAL;\nEND_VAR").unwrap() + "x : REAL;\nEND_VAR\n".len();
    let mut ctx = CompletionCtx::new(offset, QueryMode::Body);
    ctx.scope_completion(pou.get_scope_id(&with_db), "", &with_db);
    let completions = ctx.take_items();

    assert!(
        format!("{completions:?}").contains("System"),
        "expected 'System' namespace in body scope completions: {completions:?}"
    );
}

/// Editing a namespace fragment (e.g. `System.M|`) should show namespace
/// children (POUs + sub-namespace fragments), not fall back to scope items.
#[rstest]
pub fn editing_namespace_fragment(mut with_db: RootDatabase) {
    let ns_source = r#"
NAMESPACE System
    FUNCTION Sin : REAL
    VAR_INPUT
        x : REAL;
    END_VAR
    END_FUNCTION
END_NAMESPACE

NAMESPACE System.Math
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
    x := System.M
END_FUNCTION
"#;

    add_sources(&mut with_db, &[ns_source, body_source]);
    let file = with_db
        .get_file(&Url::parse("file:///test1.st").unwrap())
        .unwrap();
    // Cursor at the end of `System.M`
    let offset = body_source.find("System.M").unwrap() + "System.M".len();
    let node = descendant_at(&with_db, file, offset).unwrap();
    let req = CompletionRequest {
        offset,
        trigger_character: None,
        query: "".into(),
        node_index_pos: None,
        is_last_before: false,
    };
    let completions = node.completion(&with_db, &req).unwrap();

    // Should contain POUs from System (Sin) and sub-namespace fragment (Math)
    assert!(
        format!("{completions:?}").contains("Sin"),
        "expected 'Sin' (POU in System) in completions: {completions:?}"
    );
    assert!(
        format!("{completions:?}").contains("Math"),
        "expected 'Math' (sub-namespace fragment) in completions: {completions:?}"
    );
}

/// Editing a deeper namespace fragment (e.g. `System.Math.S|`) should show
/// POUs from System.Math, not from System or scope items.
#[rstest]
pub fn editing_deep_namespace_fragment(mut with_db: RootDatabase) {
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
    x := System.Math.S
END_FUNCTION
"#;

    add_sources(&mut with_db, &[ns_source, body_source]);
    let file = with_db
        .get_file(&Url::parse("file:///test1.st").unwrap())
        .unwrap();
    let offset = body_source.find("System.Math.S").unwrap() + "System.Math.S".len();
    let node = descendant_at(&with_db, file, offset).unwrap();
    let req = CompletionRequest {
        offset,
        trigger_character: None,
        query: "".into(),
        node_index_pos: None,
        is_last_before: false,
    };
    let completions = node.completion(&with_db, &req).unwrap();

    // Should show POUs from System.Math (Sin, Cos), NOT Log from System
    assert!(
        format!("{completions:?}").contains("Sin"),
        "expected 'Sin' in completions: {completions:?}"
    );
    assert!(
        format!("{completions:?}").contains("Cos"),
        "expected 'Cos' in completions: {completions:?}"
    );
    assert!(
        !format!("{completions:?}").contains("Log"),
        "should NOT contain 'Log' from parent namespace: {completions:?}"
    );
}

// --- Head context (type spec) namespace completions ---

#[rstest]
pub fn head_namespace_dot_trigger(mut with_db: RootDatabase) {
    // Typing `System.` in a VAR type spec should show POUs inside System namespace
    let ns_source = r#"
NAMESPACE System
    FUNCTION_BLOCK Controller
    END_FUNCTION_BLOCK

    FUNCTION_BLOCK Sensor
    END_FUNCTION_BLOCK
END_NAMESPACE
"#;

    let body_source = r#"
FUNCTION fn1
VAR
    x : System.
END_VAR
END_FUNCTION
"#;

    add_sources(&mut with_db, &[ns_source, body_source]);
    let file = with_db
        .get_file(&Url::parse("file:///test1.st").unwrap())
        .unwrap();
    let offset = body_source.find("System.").unwrap() + "System.".len();
    let (node, node_key, is_last_before) =
        completion_descendant_at(&with_db, file, offset).unwrap();
    let req = CompletionRequest {
        offset,
        trigger_character: Some(".".into()),
        query: "".into(),
        node_index_pos: Some(node_key),
        is_last_before,
    };
    let completions = node.completion(&with_db, &req).unwrap();

    assert!(
        format!("{completions:?}").contains("Controller"),
        "expected 'Controller' in completions: {completions:?}"
    );
    assert!(
        format!("{completions:?}").contains("Sensor"),
        "expected 'Sensor' in completions: {completions:?}"
    );
}

#[rstest]
pub fn head_namespace_sub_fragments(mut with_db: RootDatabase) {
    // Typing `System.` in a VAR type spec should also show sub-namespace fragments
    let ns_source = r#"
NAMESPACE System.Math
    FUNCTION_BLOCK Calculator
    END_FUNCTION_BLOCK
END_NAMESPACE
"#;

    let body_source = r#"
FUNCTION fn1
VAR
    x : System.
END_VAR
END_FUNCTION
"#;

    add_sources(&mut with_db, &[ns_source, body_source]);
    let file = with_db
        .get_file(&Url::parse("file:///test1.st").unwrap())
        .unwrap();
    let offset = body_source.find("System.").unwrap() + "System.".len();
    let (node, node_key, is_last_before) =
        completion_descendant_at(&with_db, file, offset).unwrap();
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
pub fn head_nested_namespace_dot_trigger(mut with_db: RootDatabase) {
    // Typing `System.Math.` in a VAR type spec should show POUs in System.Math only
    let ns_source = r#"
NAMESPACE System
    FUNCTION_BLOCK Controller
    END_FUNCTION_BLOCK
END_NAMESPACE

NAMESPACE System.Math
    FUNCTION_BLOCK Calculator
    END_FUNCTION_BLOCK
END_NAMESPACE
"#;

    let body_source = r#"
FUNCTION fn1
VAR
    x : System.Math.
END_VAR
END_FUNCTION
"#;

    add_sources(&mut with_db, &[ns_source, body_source]);
    let file = with_db
        .get_file(&Url::parse("file:///test1.st").unwrap())
        .unwrap();
    let offset = body_source.find("System.Math.").unwrap() + "System.Math.".len();
    let (node, node_key, is_last_before) =
        completion_descendant_at(&with_db, file, offset).unwrap();
    let req = CompletionRequest {
        offset,
        trigger_character: Some(".".into()),
        query: "".into(),
        node_index_pos: Some(node_key),
        is_last_before,
    };
    let completions = node.completion(&with_db, &req).unwrap();

    assert!(
        format!("{completions:?}").contains("Calculator"),
        "expected 'Calculator' in completions: {completions:?}"
    );
    assert!(
        !format!("{completions:?}").contains("Controller"),
        "should NOT contain 'Controller' from parent namespace: {completions:?}"
    );
}

#[rstest]
pub fn head_namespace_editing_target(mut with_db: RootDatabase) {
    // Typing `System.C` in a VAR type spec should show items from System namespace
    let ns_source = r#"
NAMESPACE System
    FUNCTION_BLOCK Controller
    END_FUNCTION_BLOCK

    FUNCTION_BLOCK Sensor
    END_FUNCTION_BLOCK
END_NAMESPACE
"#;

    let body_source = r#"
FUNCTION fn1
VAR
    x : System.C
END_VAR
END_FUNCTION
"#;

    add_sources(&mut with_db, &[ns_source, body_source]);
    let file = with_db
        .get_file(&Url::parse("file:///test1.st").unwrap())
        .unwrap();
    let offset = body_source.find("System.C").unwrap() + "System.C".len();
    let node = descendant_at(&with_db, file, offset).unwrap();
    let req = CompletionRequest {
        offset,
        trigger_character: None,
        query: "".into(),
        node_index_pos: None,
        is_last_before: false,
    };
    let completions = node.completion(&with_db, &req).unwrap();

    // Should contain POUs from System (Controller, Sensor) — VS Code will filter by "C"
    assert!(
        format!("{completions:?}").contains("Controller"),
        "expected 'Controller' in completions: {completions:?}"
    );
    assert!(
        format!("{completions:?}").contains("Sensor"),
        "expected 'Sensor' in completions: {completions:?}"
    );
}

#[rstest]
pub fn extends_namespace_dot_trigger(mut with_db: RootDatabase) {
    // Typing `EXTENDS System.` should show POUs in System namespace
    let ns_source = r#"
NAMESPACE System
    FUNCTION_BLOCK BaseController
    END_FUNCTION_BLOCK
END_NAMESPACE
"#;

    let body_source = r#"
FUNCTION_BLOCK MyFB EXTENDS System.
END_FUNCTION_BLOCK
"#;

    add_sources(&mut with_db, &[ns_source, body_source]);
    let file = with_db
        .get_file(&Url::parse("file:///test1.st").unwrap())
        .unwrap();
    let offset = body_source.find("System.").unwrap() + "System.".len();
    let (node, node_key, is_last_before) =
        completion_descendant_at(&with_db, file, offset).unwrap();
    let req = CompletionRequest {
        offset,
        trigger_character: Some(".".into()),
        query: "".into(),
        node_index_pos: Some(node_key),
        is_last_before,
    };
    let completions = node.completion(&with_db, &req).unwrap();

    assert!(
        format!("{completions:?}").contains("BaseController"),
        "expected 'BaseController' in completions: {completions:?}"
    );
}

#[rstest]
pub fn head_namespace_editing_middle_fragment(mut with_db: RootDatabase) {
    // Editing a middle fragment (e.g., Std.C|.Timers) should show items from the parent namespace
    let ns_source = r#"
NAMESPACE Std
    FUNCTION_BLOCK DirectFB
    END_FUNCTION_BLOCK
END_NAMESPACE

NAMESPACE Std.Convert
    FUNCTION_BLOCK Timers
    END_FUNCTION_BLOCK
END_NAMESPACE
"#;

    let body_source = r#"
FUNCTION fn1
VAR
    x : Std.C.Timers
END_VAR
END_FUNCTION
"#;

    add_sources(&mut with_db, &[ns_source, body_source]);
    let file = with_db
        .get_file(&Url::parse("file:///test1.st").unwrap())
        .unwrap();
    // Cursor at Std.C| (after "C", in the middle fragment)
    let offset = body_source.find("Std.C").unwrap() + "Std.C".len();
    let node = descendant_at(&with_db, file, offset).unwrap();
    let req = CompletionRequest {
        offset,
        trigger_character: None,
        query: "".into(),
        node_index_pos: None,
        is_last_before: false,
    };
    let completions = node.completion(&with_db, &req).unwrap();

    // Should contain items from Std namespace (DirectFB, Convert sub-namespace)
    assert!(
        format!("{completions:?}").contains("DirectFB"),
        "expected 'DirectFB' (POU in Std) in completions: {completions:?}"
    );
    assert!(
        format!("{completions:?}").contains("Convert"),
        "expected 'Convert' (sub-namespace) in completions: {completions:?}"
    );
}

/// Completing after a RELATIVE namespace path lists what the resolved
/// namespace holds: `Impl.` inside `NAMESPACE Lib` completes `Lib.Impl`.
#[rstest]
pub fn relative_namespace_pou_completion(mut with_db: RootDatabase) {
    let source = r#"
NAMESPACE Lib
    NAMESPACE Impl
        FUNCTION hidden : INT
            hidden := 1;
        END_FUNCTION
    END_NAMESPACE
    FUNCTION api : INT
        api := Impl.
    END_FUNCTION
END_NAMESPACE
"#;

    add_sources(&mut with_db, &[source]);
    let file = with_db
        .get_file(&Url::parse("file:///test0.st").unwrap())
        .unwrap();
    let offset = source.find("Impl.").unwrap() + "Impl.".len();
    let (node, node_key, is_last_before) =
        completion_descendant_at(&with_db, file, offset).unwrap();
    let req = CompletionRequest {
        offset,
        trigger_character: Some(".".into()),
        query: "".into(),
        node_index_pos: Some(node_key),
        is_last_before,
    };
    let completions = node.completion(&with_db, &req).unwrap();
    assert!(
        format!("{completions:?}").contains("hidden"),
        "expected 'hidden' from Lib.Impl: {completions:?}"
    );
}
