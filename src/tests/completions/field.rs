// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use db::RootDatabase;
use ide_proto::{
    handlers::{CompletionHandler, CompletionRequest},
    walk::{completion_descendant_at, descendant_at},
};
use rstest::rstest;

use crate::tests::utils::{add_source, with_db};

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

    let file = add_source(&mut with_db, source);
    // Resolved the way the server resolves it: the flags decide whether the
    // cursor is ON a name or past it, onto a receiver.
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

    let file = add_source(&mut with_db, source);
    // Offset right after the trailing dot — no node contains this position,
    // so completion_descendant_at falls back to the closest preceding node.
    let offset = source.find("my_var.inner.").unwrap() + "my_var.inner.".len();
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

    let file = add_source(&mut with_db, source);
    // Resolved the way the server resolves it: the flags decide whether the
    // cursor is ON a name or past it, onto a receiver.
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

    let file = add_source(&mut with_db, source);
    let expr = descendant_at(&with_db, file, 112).unwrap();
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

    let file = add_source(&mut with_db, source);
    let expr = descendant_at(&with_db, file, 108).unwrap();
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

    let file = add_source(&mut with_db, source);
    let offset = source.find("0.").unwrap() + "0.".len();
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
    list : ARRAY[0..1] OF Engine;
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
    list : ARRAY[0..1] OF Engine;
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
    list : ARRAY[0..1] OF Engine;
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
        list : ARRAY[0..1] OF Engine;
    END_VAR
        {probe}
    END_METHOD
END_CLASS
"#
)]
pub fn a_trailing_dot_completes_the_receiver_in_every_body(#[case] body: &str) {
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

    for (probe, expected) in [
        ("s.", "oil"),
        ("mot.", "rpm"),
        ("Std.", "Maths"),
        ("list[0].", "oil"),
    ] {
        let source = format!("{HEAD}{}", body.replace("{probe}", probe));
        // A database of its own for each probe: a clone shares the files.
        let mut db = with_db();
        let file = add_source(&mut db, &source);

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
    let file = add_source(&mut with_db, source);
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

/// What is written around a cursor, `|`, inside a FUNCTION holding these variables,
/// and what the completion offers there.
fn labels_at(probe: &str) -> Vec<String> {
    const HEAD: &str = r#"TYPE Inner : STRUCT depth : INT; END_STRUCT; END_TYPE
TYPE Engine : STRUCT fuel : INT; oil : REAL; inner : Inner; list : ARRAY[0..1] OF Inner; END_STRUCT; END_TYPE
FUNCTION_BLOCK Motor VAR_INPUT rpm : INT; END_VAR END_FUNCTION_BLOCK
FUNCTION probe : INT
VAR
    named : ARRAY[0..1] OF Engine;
    matrix : ARRAY[0..1, 0..1] OF Engine;
    motors : ARRAY[0..1] OF Motor;
    r : REF_TO Engine;
    refs : ARRAY[0..1] OF REF_TO Engine;
    ra : REF_TO ARRAY[0..1] OF Engine;
    e : Engine;
    i : INT;
END_VAR
"#;
    let cursor = probe.find('|').expect("the cursor");
    let source = format!("{HEAD}    {}\nEND_FUNCTION\n", probe.replacen('|', "", 1));
    let offset = HEAD.len() + "    ".len() + cursor;

    let mut db = with_db();
    let file = add_source(&mut db, &source);
    let (node, node_key, is_last_before) =
        completion_descendant_at(&db, file, offset).expect("a node at the cursor");
    let req = CompletionRequest {
        offset,
        trigger_character: probe[..cursor].ends_with('.').then(|| ".".into()),
        query: "".into(),
        node_index_pos: Some(node_key),
        is_last_before,
    };
    node.completion(&db, &req)
        .unwrap_or_default()
        .into_iter()
        .map(|c| c.label)
        .collect()
}

/// Past a subscript or a `^`, the members are those of what the path reads, the
/// element or the target, whether the name is being typed or not yet started. The
/// node of `rows[0]` spans `rows` alone, and the last node before the cursor is
/// what the subscript holds, so nothing was offered, on any array.
#[rstest]
#[case::element("named[0].|", &["fuel", "oil", "inner", "list"])]
#[case::element_being_typed("named[0].fu|", &["fuel", "oil", "inner", "list"])]
#[case::element_on_its_name("named[0].fuel| := 1;", &["fuel", "oil", "inner", "list"])]
#[case::element_inside_its_name("named[0].f|uel := 1;", &["fuel", "oil", "inner", "list"])]
#[case::a_variable_in_the_subscript("named[i].|", &["fuel", "oil", "inner", "list"])]
#[case::an_expression_in_the_subscript("named[i + 1].|", &["fuel", "oil", "inner", "list"])]
#[case::a_blank_before_the_dot("named[ 0 ] .|", &["fuel", "oil", "inner", "list"])]
#[case::a_blank_after_the_dot("named[0]. |", &["fuel", "oil", "inner", "list"])]
#[case::two_dimensions("matrix[0, 1].|", &["fuel", "oil", "inner", "list"])]
#[case::two_dimensions_being_typed("matrix[0, 1].fu|", &["fuel", "oil", "inner", "list"])]
#[case::two_brackets("matrix[0][1].|", &["fuel", "oil", "inner", "list"])]
#[case::two_brackets_being_typed("matrix[0][1].fu|", &["fuel", "oil", "inner", "list"])]
#[case::a_row_has_no_members("matrix[0].|", &[])]
#[case::an_instance("motors[0].|", &["rpm"])]
#[case::an_instance_being_typed("motors[0].rp|", &["rpm"])]
#[case::a_target("r^.|", &["fuel", "oil", "inner", "list"])]
#[case::a_target_being_typed("r^.fu|", &["fuel", "oil", "inner", "list"])]
#[case::the_target_of_an_element("refs[0]^.|", &["fuel", "oil", "inner", "list"])]
#[case::the_target_of_an_element_being_typed("refs[0]^.fu|", &["fuel", "oil", "inner", "list"])]
#[case::an_element_of_a_target("ra^[0].|", &["fuel", "oil", "inner", "list"])]
#[case::an_element_of_a_target_being_typed("ra^[0].fu|", &["fuel", "oil", "inner", "list"])]
#[case::a_field_of_an_element("named[0].inner.|", &["depth"])]
#[case::a_field_of_an_element_being_typed("named[0].inner.de|", &["depth"])]
#[case::an_element_of_a_field_of_an_element("named[0].list[1].|", &["depth"])]
#[case::an_element_of_a_field("e.list[1].|", &["depth"])]
#[case::an_element_of_a_field_being_typed("e.list[1].de|", &["depth"])]
#[case::a_value_being_assigned("i := named[0].|", &["fuel", "oil", "inner", "list"])]
#[case::a_value_being_typed("i := named[0].fu|", &["fuel", "oil", "inner", "list"])]
#[case::an_argument("i := ABS(named[0].|)", &["fuel", "oil", "inner", "list"])]
#[case::a_subscript_being_written("named[e.|]", &["fuel", "oil", "inner", "list"])]
#[case::a_statement_follows("named[0].|\n    i := 1;", &["fuel", "oil", "inner", "list"])]
#[case::a_statement_reads_like_a_field("named[0].|\n    fuel := 1;", &["fuel", "oil", "inner", "list"])]
#[case::a_field_reads_like_the_next_statement("e.|\n    fuel := 1;", &["fuel", "oil", "inner", "list"])]
#[case::a_field("e.|", &["fuel", "oil", "inner", "list"])]
#[case::a_field_being_typed("e.fu|", &["fuel", "oil", "inner", "list"])]
#[case::a_field_of_a_field("e.inner.|", &["depth"])]
#[case::a_subscript_of_a_name_that_is_nothing("x[0].|", &[])]
#[case::a_subscript_of_a_field_that_is_nothing("e.nope[0].|", &[])]
#[case::a_field_that_is_nothing("e.nope.|", &[])]
#[case::a_field_of_an_element_that_is_nothing("named[0].nope.|", &[])]
fn a_dot_past_a_path_completes_what_it_reads(#[case] probe: &str, #[case] expected: &[&str]) {
    assert_eq!(labels_at(probe), expected, "at `{probe}`");
}

/// With no dot, or with the cursor on the name, the path is no receiver. A name
/// being typed in front of its subscript completes the scope as any other does.
#[rstest]
#[case::a_subscript_with_no_dot("named[0]|", "named", "fuel")]
#[case::a_reference_with_no_dot("r^|", "named", "fuel")]
#[case::a_name_and_a_blank("e |", "named", "fuel")]
#[case::a_name_before_its_subscript("nam|ed[0]", "named", "fuel")]
#[case::a_name_before_its_reference("r|^", "named", "fuel")]
#[case::inside_a_subscript("named[|]", "i", "fuel")]
#[case::a_name_inside_a_subscript("named[i|]", "i", "fuel")]
#[case::a_field_before_its_subscript("e.li|st[0]", "list", "depth")]
fn no_dot_past_a_path_offers_no_members_of_it(
    #[case] probe: &str,
    #[case] offered: &str,
    #[case] not_offered: &str,
) {
    let labels = labels_at(probe);
    assert!(
        labels.iter().any(|l| l == offered),
        "`{offered}` at `{probe}`: {labels:?}"
    );
    assert!(
        !labels.iter().any(|l| l == not_offered),
        "no `{not_offered}` at `{probe}`: {labels:?}"
    );
}

/// Inside an array's initializer, a STRUCT's parentheses complete its members: what
/// they stand for is the element, and they were typed as the array.
#[rstest]
fn an_element_of_an_initializer_completes_its_fields(mut with_db: RootDatabase) {
    let source = r#"TYPE Engine : STRUCT fuel : INT; oil : REAL; END_STRUCT; END_TYPE
FUNCTION_BLOCK fb
VAR
    list : ARRAY[0..1] OF Engine := [(fuel := 1 )];
END_VAR
END_FUNCTION_BLOCK
"#;
    let file = add_source(&mut with_db, source);
    let labels = |db: &RootDatabase, at: &str| -> Vec<String> {
        let offset = source.find(at).expect("the probe") + at.len() - 1;
        let (node, node_key, is_last_before) =
            completion_descendant_at(db, file, offset).expect("a node");
        let req = CompletionRequest {
            offset,
            trigger_character: None,
            query: "".into(),
            node_index_pos: Some(node_key),
            is_last_before,
        };
        node.completion(db, &req)
            .unwrap_or_default()
            .into_iter()
            .map(|c| c.label)
            .collect()
    };
    let named = labels(&with_db, "fuel := 1 )");
    assert!(named.iter().any(|l| l == "oil"), "{named:?}");
}

const OBJECTS: &str = r#"TYPE Engine : STRUCT fuel : INT; END_STRUCT; END_TYPE
CLASS Root
VAR PUBLIC root_var : INT; END_VAR
METHOD PUBLIC RootM : INT
END_METHOD
END_CLASS
CLASS Base EXTENDS Root
VAR PUBLIC base_var : INT; shared : INT; END_VAR
VAR PROTECTED guarded : INT; END_VAR
VAR PRIVATE hidden : INT; END_VAR
METHOD PUBLIC BaseM : INT
END_METHOD
METHOD PROTECTED GuardedM : INT
END_METHOD
METHOD PRIVATE HiddenM : INT
END_METHOD
END_CLASS
CLASS Kid EXTENDS Base
VAR PUBLIC kid_var : Engine; list : ARRAY[0..1] OF Engine; shared : REAL; END_VAR
METHOD PUBLIC KidM : INT
VAR p : REF_TO Kid; END_VAR
    {inside}
END_METHOD
END_CLASS
FUNCTION outside : INT
VAR k : Kid; r : REF_TO Kid; ks : ARRAY[0..1] OF Kid; END_VAR
    {outside}
END_FUNCTION
"#;

fn object_labels(inside: &str, outside: &str) -> Vec<String> {
    let probe = format!("{inside}{outside}");
    let cursor = OBJECTS
        .replace("{inside}", inside)
        .replace("{outside}", outside);
    let offset = cursor.find('|').expect("the cursor");
    let source = cursor.replacen('|', "", 1);
    let mut db = with_db();
    let file = add_source(&mut db, &source);
    let (node, node_key, is_last_before) =
        completion_descendant_at(&db, file, offset).expect("a node at the cursor");
    let req = CompletionRequest {
        offset,
        trigger_character: probe.contains(".|").then(|| ".".into()),
        query: "".into(),
        node_index_pos: Some(node_key),
        is_last_before,
    };
    let mut labels: Vec<_> = node
        .completion(&db, &req)
        .unwrap_or_default()
        .into_iter()
        .map(|c| c.label)
        .collect();
    labels.sort_unstable();
    labels
}

// A specifier on a VAR section is not enforced, so the checker reaches `guarded`
// and `hidden` from anywhere, and what it reaches is offered.
const FROM_OUTSIDE: &[&str] = &[
    "BaseM", "KidM", "RootM", "base_var", "guarded", "hidden", "kid_var", "list", "root_var",
    "shared",
];
const FROM_INSIDE: &[&str] = &[
    "BaseM", "GuardedM", "KidM", "RootM", "base_var", "guarded", "hidden", "kid_var", "list",
    "root_var", "shared",
];
const OF_THE_BASE: &[&str] = &["BaseM", "GuardedM", "RootM"];

/// A block answers to what its bases declare, and the checker reaches it. Only what
/// the POU declares itself was offered. A name of its own hides a base's, and what
/// the asking POU may not name is not offered.
#[rstest]
#[case::an_instance("", "k.|", FROM_OUTSIDE)]
#[case::an_instance_being_typed("", "k.ba|", FROM_OUTSIDE)]
#[case::a_reference("", "r^.|", FROM_OUTSIDE)]
#[case::a_reference_being_typed("", "r^.ba|", FROM_OUTSIDE)]
#[case::an_element("", "ks[0].|", FROM_OUTSIDE)]
#[case::a_reference_inside("p^.|", "", FROM_INSIDE)]
#[case::this("THIS^.|", "", FROM_INSIDE)]
#[case::this_being_typed("THIS^.ba|", "", FROM_INSIDE)]
#[case::this_on_a_name("THIS^.base_var|", "", FROM_INSIDE)]
#[case::a_blank_after_this("THIS^. |", "", FROM_INSIDE)]
#[case::the_member_of_this("THIS^.kid_var.|", "", &["fuel"])]
#[case::the_element_of_this("THIS^.list[0].|", "", &["fuel"])]
#[case::the_element_of_this_being_typed("THIS^.list[0].fu|", "", &["fuel"])]
#[case::super_("SUPER^.|", "", OF_THE_BASE)]
#[case::super_being_typed("SUPER^.ba|", "", OF_THE_BASE)]
fn a_dot_completes_what_a_block_inherits(
    #[case] inside: &str,
    #[case] outside: &str,
    #[case] expected: &[&str],
) {
    assert_eq!(
        object_labels(inside, outside),
        expected,
        "at `{inside}{outside}`"
    );
}
