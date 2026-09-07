use auto_lsp::default::db::BaseDatabase;
use db::RootDatabase;
use ide_proto::{
    handlers::{CompletionHandler, CompletionRequest},
    walk::{completion_descendant_at, descendant_at},
};
use rstest::rstest;

use crate::tests::utils::{add_sources, with_db};

#[rstest]
pub fn struct_field_completion(mut with_db: RootDatabase) {
    let source = r#"
TYPE Engine :
    STRUCT
        oil : REAL;
        fuel: INT;
    END_STRUCT
END_TYPE


FUNCTION_BLOCK fb
    VAR
        my_var: Engine;
    END_VAR

    my_var.
END_FUNCTION_BLOCK
"#;

    add_sources(&mut with_db, &[source]);
    // Resolved the way the server resolves it: the flags decide whether the
    // cursor is ON a name or past it, onto a receiver.
    let file = *with_db.get_files().iter().last().unwrap();
    // Right after the dot: a receiver, so its members are what is wanted.
    // The offset used to fall INSIDE `my_var`, where the answer is the scope.
    let offset = source.rfind("my_var.").expect("the probe") + "my_var.".len();
    let (path_expr, node_key, is_last_before) =
        completion_descendant_at(&with_db, file, offset).unwrap();
    let req = CompletionRequest {
        offset,
        trigger_character: Some(".".into()),
        query: "".into(),
        node_index_pos: Some(node_key),
        is_last_before,
    };
    let completions = path_expr.completion(&with_db, &req).unwrap();

    assert_eq!(completions.len(), 2);
    assert!(format!("{completions:?}").contains("oil"));
    assert!(format!("{completions:?}").contains("fuel"));
}

#[rstest]
pub fn nested_struct_field_completion(mut with_db: RootDatabase) {
    let source = r#"
TYPE Inner :
    STRUCT
        depth : INT;
        width : REAL;
    END_STRUCT
END_TYPE

TYPE Outer :
    STRUCT
        inner : Inner;
        name : INT;
    END_STRUCT
END_TYPE

FUNCTION_BLOCK fb
    VAR
        my_var: Outer;
    END_VAR

    my_var.inner.
END_FUNCTION_BLOCK
"#;

    add_sources(&mut with_db, &[source]);
    // Offset right after the trailing dot — no node contains this position,
    // so completion_descendant_at falls back to the closest preceding node.
    let offset = source.find("my_var.inner.").unwrap() + "my_var.inner.".len();
    let file = *with_db.get_files().iter().last().unwrap();
    let (path_expr, node_key, is_last_before) =
        completion_descendant_at(&with_db, file, offset).unwrap();
    let req = CompletionRequest {
        offset,
        trigger_character: Some(".".into()),
        query: "".into(),
        node_index_pos: Some(node_key),
        is_last_before,
    };
    let completions = path_expr.completion(&with_db, &req).unwrap();

    // Should show fields of Inner (depth, width), NOT fields of Outer (inner, name)
    assert!(
        format!("{completions:?}").contains("depth"),
        "expected 'depth' in completions: {completions:?}"
    );
    assert!(
        format!("{completions:?}").contains("width"),
        "expected 'width' in completions: {completions:?}"
    );
    assert!(
        !format!("{completions:?}").contains("name"),
        "should NOT contain 'name' from Outer: {completions:?}"
    );
}

#[rstest]
pub fn class_var_and_methods_completion(mut with_db: RootDatabase) {
    let source = r#"
CLASS Engine
    VAR
        oil : REAL;
        fuel: INT;
    END_VAR

    METHOD Start
    END_METHOD

END_CLASS

FUNCTION_BLOCK fb
    VAR
        my_var: Engine;
    END_VAR

    my_var.
END_FUNCTION_BLOCK
"#;

    add_sources(&mut with_db, &[source]);
    // Resolved the way the server resolves it: the flags decide whether the
    // cursor is ON a name or past it, onto a receiver.
    let file = *with_db.get_files().iter().last().unwrap();
    // Right after the dot: a receiver, so its members are what is wanted.
    // The offset used to fall INSIDE `my_var`, where the answer is the scope.
    let offset = source.rfind("my_var.").expect("the probe") + "my_var.".len();
    let (path_expr, node_key, is_last_before) =
        completion_descendant_at(&with_db, file, offset).unwrap();
    let req = CompletionRequest {
        offset,
        trigger_character: Some(".".into()),
        query: "".into(),
        node_index_pos: Some(node_key),
        is_last_before,
    };
    let completions = path_expr.completion(&with_db, &req).unwrap();

    assert_eq!(completions.len(), 3);
    assert!(format!("{completions:?}").contains("oil"));
    assert!(format!("{completions:?}").contains("fuel"));
    assert!(format!("{completions:?}").contains("Start()"));
}

#[rstest]
pub fn enum_variants_completion(mut with_db: RootDatabase) {
    let source = r#"
TYPE
    List: UINT (A, B, C);
END_TYPE

FUNCTION fn1
    VAR
        test: List;
    END_VAR

    test := List#

END_FUNCTION
"#;

    add_sources(&mut with_db, &[source]);
    let expr = descendant_at(&with_db, *with_db.get_files().iter().last().unwrap(), 112).unwrap();
    let req = CompletionRequest {
        offset: 112,
        trigger_character: None,
        query: "".into(),
        node_index_pos: None,
        is_last_before: false,
    };
    let completions = expr.completion(&with_db, &req).unwrap();

    assert_eq!(completions.len(), 3);
    assert!(format!("{completions:?}").contains("A"));
    assert!(format!("{completions:?}").contains("B"));
    assert!(format!("{completions:?}").contains("C"));
}

#[rstest]
pub fn invocation_completion(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        test: INT;
    END_VAR

    METHOD doWork 
    
    END_METHOD

    THIS.

END_FUNCTION_BLOCK
"#;

    add_sources(&mut with_db, &[source]);
    let expr = descendant_at(&with_db, *with_db.get_files().iter().last().unwrap(), 108).unwrap();
    let req = CompletionRequest {
        offset: 108,
        trigger_character: None,
        query: "".into(),
        node_index_pos: None,
        is_last_before: false,
    };
    let completions = expr.completion(&with_db, &req).unwrap();

    assert_eq!(completions.len(), 2);
    assert!(format!("{completions:?}").contains("test"));
    assert!(format!("{completions:?}").contains("doWork()"));
}

/// Typing `my_var := 0.` should NOT trigger field completions.
/// The dot after a numeric literal is part of a REAL literal (e.g. `0.0`),
/// not a field access.
#[rstest]
pub fn no_completion_after_numeric_literal_dot(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb
    VAR
        my_var: INT;
    END_VAR

    my_var := 0.
END_FUNCTION_BLOCK
"#;

    add_sources(&mut with_db, &[source]);
    let offset = source.find("0.").unwrap() + "0.".len();
    let file = *with_db.get_files().iter().last().unwrap();
    let result = completion_descendant_at(&with_db, file, offset);

    if let Some((node, node_key, is_last_before)) = result {
        let req = CompletionRequest {
            offset,
            trigger_character: Some(".".into()),
            query: "".into(),
            node_index_pos: Some(node_key),
            is_last_before,
        };
        let completions = node.completion(&with_db, &req).unwrap_or_default();
        // Should be empty or at least not contain all scope items
        assert!(
            completions.len() <= 1,
            "dot after numeric literal should not trigger completions, got {} items: {:?}",
            completions.len(),
            completions.iter().map(|c| &c.label).collect::<Vec<_>>()
        );
    }
}

/// A trailing dot completes what the receiver holds, in EVERY kind of body,
/// and a variable whose name reads like a namespace is still a variable.
///
/// `s.` on a `s : Engine` used to list the children of `Std`, because the
/// completion asked "is this a namespace prefix?" before asking what the path
/// resolves to, and `s` is a prefix of `Std`. Name resolution has always let a
/// variable shadow a namespace; this is that rule, in the IDE. `Std` is
/// declared in the fixture for exactly that reason.
#[rstest]
#[case::function(
    r#"FUNCTION fn
VAR
    s : Engine;
    mot : Motor;
END_VAR
    {probe}
END_FUNCTION
"#
)]
#[case::function_block(
    r#"FUNCTION_BLOCK fb
VAR
    s : Engine;
    mot : Motor;
END_VAR
    {probe}
END_FUNCTION_BLOCK
"#
)]
#[case::program(
    r#"PROGRAM Main
VAR
    s : Engine;
    mot : Motor;
END_VAR
    {probe}
END_PROGRAM
"#
)]
#[case::method(
    r#"CLASS C
    METHOD M
    VAR
        s : Engine;
        mot : Motor;
    END_VAR
        {probe}
    END_METHOD
END_CLASS
"#
)]
pub fn a_trailing_dot_completes_the_receiver_in_every_body(
    with_db: RootDatabase,
    #[case] body: &str,
) {
    const HEAD: &str = r#"TYPE Engine : STRUCT
    oil : REAL;
END_STRUCT
END_TYPE

FUNCTION_BLOCK Motor
VAR_INPUT
    rpm : INT;
END_VAR
END_FUNCTION_BLOCK

NAMESPACE Std
    NAMESPACE Maths
        FUNCTION SQRT : REAL
        END_FUNCTION
    END_NAMESPACE
END_NAMESPACE

"#;

    for (probe, expected) in [("s.", "oil"), ("mot.", "rpm"), ("Std.", "Maths")] {
        let source = format!("{HEAD}{}", body.replace("{probe}", probe));
        let mut db = with_db.clone();
        add_sources(&mut db, &[&source]);
        let file = *db.get_files().iter().last().unwrap();

        // The cursor sits right after the dot.
        let line = source
            .lines()
            .find(|l| l.trim() == probe)
            .expect("the probe line");
        let offset = source.find(line).expect("the probe line") + line.len();

        let (node, node_key, is_last_before) =
            completion_descendant_at(&db, file, offset).expect("a node at the dot");
        let req = CompletionRequest {
            offset,
            trigger_character: Some(".".into()),
            query: "".into(),
            node_index_pos: Some(node_key),
            is_last_before,
        };
        let completions = node.completion(&db, &req).unwrap_or_default();
        let labels: Vec<_> = completions.iter().map(|c| c.label.as_str()).collect();
        assert!(
            labels.contains(&expected),
            "`{probe}` must complete `{expected}`, got {labels:?}"
        );
    }
}

/// A NAME being typed completes the scope; only a receiver completes members.
///
/// `f` on an `f : Engine` used to offer `oil`, because the handler asked what
/// the half-written name resolved to and listed ITS members. The cursor sits
/// on the name there, and past it after a dot, which is what tells the two
/// apart.
#[rstest]
fn a_name_being_typed_completes_the_scope_not_its_members(mut with_db: RootDatabase) {
    let source = r#"TYPE Engine : STRUCT
    oil : REAL;
    fuel : INT;
END_STRUCT
END_TYPE

FUNCTION_BLOCK fb
VAR
    f : Engine;
END_VAR
    f
END_FUNCTION_BLOCK
"#;
    add_sources(&mut with_db, &[source]);
    let file = *with_db.get_files().iter().last().unwrap();
    let offset = source.find("    f\n").expect("the probe line") + "    f".len();

    let (node, node_key, is_last_before) =
        completion_descendant_at(&with_db, file, offset).expect("a node at the name");
    let req = CompletionRequest {
        offset,
        trigger_character: None,
        query: "f".into(),
        node_index_pos: Some(node_key),
        is_last_before,
    };
    let labels: Vec<_> = node
        .completion(&with_db, &req)
        .unwrap_or_default()
        .into_iter()
        .map(|c| c.label)
        .collect();

    assert!(
        !labels.iter().any(|l| l == "oil" || l == "fuel"),
        "the fields of `f` are not in scope where `f` is being typed: {labels:?}"
    );
    assert!(
        labels.iter().any(|l| l == "f"),
        "the variable itself is: {labels:?}"
    );
}
