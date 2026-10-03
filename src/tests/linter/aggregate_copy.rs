// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

const TYPES: &str = r#"
FUNCTION_BLOCK Motor
VAR n : INT; END_VAR
END_FUNCTION_BLOCK

TYPE Pt : STRUCT x : INT; y : INT; END_STRUCT; END_TYPE
TYPE Row : ARRAY[0..9] OF INT; END_TYPE
TYPE Holder : STRUCT m : Motor; k : INT; END_STRUCT; END_TYPE
"#;

/// Every assignment that copies an aggregate whole: an instance, on its own
/// or as an element or a field, a STRUCT, an array named or not, and an
/// element that is itself an array.
#[rstest]
#[case::instance("i1 := i2;", "the assignment copies the whole 'Motor' instance")]
#[case::instance_element("arr1[0] := i1;", "the assignment copies the whole 'Motor' instance")]
#[case::instance_field("h1.m := i1;", "the assignment copies the whole 'Motor' instance")]
#[case::instances(
    "arr1 := arr2;",
    "the assignment copies the whole 'ARRAY [0..1] OF Motor'"
)]
#[case::struct_("p1 := p2;", "the assignment copies the whole 'Pt' structure")]
#[case::struct_holding_an_instance(
    "h1 := h2;",
    "the assignment copies the whole 'Holder' structure"
)]
#[case::named_array("r1 := r2;", "the assignment copies the whole 'Row' array")]
#[case::array(
    "raw1 := raw2;",
    "the assignment copies the whole 'ARRAY [0..3] OF INT'"
)]
#[case::array_element("rows[1] := r1;", "the assignment copies the whole 'Row' array")]
fn an_aggregate_assignment_is_reported(
    mut with_db: RootDatabase,
    #[case] stmt: &str,
    #[case] message: &str,
) {
    let source = format!(
        r#"{TYPES}
PROGRAM P
VAR
    i1 : Motor; i2 : Motor; arr1 : ARRAY[0..1] OF Motor; arr2 : ARRAY[0..1] OF Motor;
    h1 : Holder; h2 : Holder; p1 : Pt; p2 : Pt; r1 : Row; r2 : Row;
    raw1 : ARRAY[0..3] OF INT; raw2 : ARRAY[0..3] OF INT; rows : ARRAY[0..2] OF Row;
END_VAR
    {stmt}
END_PROGRAM
"#
    );
    let rendered = test_single_lint(&mut with_db, &[&source], "aggregate-copy");
    assert_eq!(
        rendered.matches("[L0214]").count(),
        1,
        "one report, got:\n{rendered}"
    );
    assert!(
        rendered.contains(message),
        "expected `{message}`, got:\n{rendered}"
    );
}

/// A scalar, a STRING, which copies its text only, a field or an element of
/// a scalar type, and a reference are not copies of an aggregate.
#[rstest]
fn a_scalar_or_a_string_is_not_reported(mut with_db: RootDatabase) {
    let source = format!(
        r#"{TYPES}
PROGRAM P
VAR
    n : INT; p1 : Pt; p2 : Pt; r1 : Row; s1 : STRING; s2 : STRING[200];
    i1 : Motor; pr : REF_TO Pt; pr2 : REF_TO Pt;
END_VAR
    n := p1.x;
    p1.x := p2.y;
    r1[3] := n;
    s1 := s2;
    s2 := 'text';
    i1.n := n;
    pr := pr2;
    pr := REF(p1);
    pr^.x := 1;
END_PROGRAM
"#
    );
    assert_snapshot!(test_single_lint(&mut with_db, &[&source], "aggregate-copy"), @"");
}

/// A call copies an aggregate into a VAR_INPUT and out of a VAR_OUTPUT,
/// bound by name or by position, in a statement or inside an expression. A
/// VAR_IN_OUT binds by reference and is not reported.
#[rstest]
fn a_call_binding_is_reported(mut with_db: RootDatabase) {
    let source = format!(
        r#"{TYPES}
FUNCTION sum : INT
VAR_INPUT p : Pt; r : Row; END_VAR
VAR_OUTPUT o : Pt; END_VAR
    sum := p.x + r[0];
END_FUNCTION

FUNCTION_BLOCK Taker
VAR_INPUT cfg : Pt; END_VAR
VAR_OUTPUT out : Row; END_VAR
VAR_IN_OUT shared : Pt; END_VAR
END_FUNCTION_BLOCK

PROGRAM P
VAR p1 : Pt; p2 : Pt; r1 : Row; r2 : Row; n : INT; t : Taker; END_VAR
    n := sum(p1, r1, o => p2);
    t(cfg := p1, out => r2, shared := p2);
END_PROGRAM
"#
    );
    assert_snapshot!(test_single_lint(&mut with_db, &[&source], "aggregate-copy"), @r"
    [L0214] Info: aggregate copy
        ,-[ file:///test0.st:24:14 ]
        |
     24 |     n := sum(p1, r1, o => p2);
        |              ^|
        |               `-- the argument copies the whole 'Pt' structure into 'p'
        |
        | Help: to share the value, pass it as a VAR_IN_OUT or keep a REF_TO it
        |
        | Note 1: a copy takes every element and field, nested ones included, and costs as much as the type is large
        |
        | Note 2: lint rule: aggregate-copy
    ----'
    [L0214] Info: aggregate copy
        ,-[ file:///test0.st:24:18 ]
        |
     24 |     n := sum(p1, r1, o => p2);
        |                  ^|
        |                   `-- the argument copies the whole 'Row' array into 'r'
        |
        | Help: to share the value, pass it as a VAR_IN_OUT or keep a REF_TO it
        |
        | Note 1: a copy takes every element and field, nested ones included, and costs as much as the type is large
        |
        | Note 2: lint rule: aggregate-copy
    ----'
    [L0214] Info: aggregate copy
        ,-[ file:///test0.st:24:22 ]
        |
     24 |     n := sum(p1, r1, o => p2);
        |                      ^^^|^^^
        |                         `----- the binding copies the whole 'Pt' structure out of 'o'
        |
        | Help: to share the value, pass it as a VAR_IN_OUT or keep a REF_TO it
        |
        | Note 1: a copy takes every element and field, nested ones included, and costs as much as the type is large
        |
        | Note 2: lint rule: aggregate-copy
    ----'
    [L0214] Info: aggregate copy
        ,-[ file:///test0.st:25:7 ]
        |
     25 |     t(cfg := p1, out => r2, shared := p2);
        |       ^^^^|^^^^
        |           `------ the argument copies the whole 'Pt' structure into 'cfg'
        |
        | Help: to share the value, pass it as a VAR_IN_OUT or keep a REF_TO it
        |
        | Note 1: a copy takes every element and field, nested ones included, and costs as much as the type is large
        |
        | Note 2: lint rule: aggregate-copy
    ----'
    [L0214] Info: aggregate copy
        ,-[ file:///test0.st:25:18 ]
        |
     25 |     t(cfg := p1, out => r2, shared := p2);
        |                  ^^^^|^^^^
        |                      `------ the binding copies the whole 'Row' array out of 'out'
        |
        | Help: to share the value, pass it as a VAR_IN_OUT or keep a REF_TO it
        |
        | Note 1: a copy takes every element and field, nested ones included, and costs as much as the type is large
        |
        | Note 2: lint rule: aggregate-copy
    ----'
    ");
}

/// How the report reads on an assignment.
#[rstest]
fn aggregate_copy_rendering(mut with_db: RootDatabase) {
    let source = r#"
TYPE Samples : ARRAY[0..999] OF REAL; END_TYPE

PROGRAM P
VAR last : Samples; current : Samples; END_VAR
    last := current;
END_PROGRAM
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "aggregate-copy"), @r"
    [L0214] Info: aggregate copy
       ,-[ file:///test0.st:6:5 ]
       |
     6 |     last := current;
       |     ^^^^^^^|^^^^^^^
       |            `--------- the assignment copies the whole 'Samples' array
       |
       | Help: to share the value, pass it as a VAR_IN_OUT or keep a REF_TO it
       |
       | Note 1: a copy takes every element and field, nested ones included, and costs as much as the type is large
       |
       | Note 2: lint rule: aggregate-copy
    ---'
    ");
}
