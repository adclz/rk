// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

#[rstest]
fn output_never_assigned(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR_OUTPUT
    result : INT;
END_VAR
    test := 0;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "uninitialized-output"), @r"
    [L0203] Info: uninitialized output
       ,-[ file:///test0.st:4:5 ]
       |
     4 |     result : INT;
       |     ^^^|^^
       |        `---- 1 VAR_OUTPUT is never assigned in the body: 'result'
       |        |
       |        `---- 'result' is declared here
       |
       | Note: lint rule: uninitialized-output
    ---'
    ");
}

#[rstest]
fn output_assigned(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR_OUTPUT
    result : INT;
END_VAR
    result := 42;
    test := 0;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "uninitialized-output"), @r"");
}

#[rstest]
fn output_assigned_in_if(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    x : INT;
END_VAR
VAR_OUTPUT
    result : INT;
END_VAR
    IF x > 0 THEN
        result := 1;
    END_IF;
    test := 0;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "uninitialized-output"), @r"");
}

#[rstest]
fn output_with_initializer_no_warning(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR_OUTPUT
    result : INT := 0;
END_VAR
    test := 0;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "uninitialized-output"), @r"");
}

#[rstest]
fn fb_output_never_assigned(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK MyFb
VAR_OUTPUT
    done : BOOL;
    value : INT;
END_VAR
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "uninitialized-output"), @r"
    [L0203] Info: uninitialized output
       ,-[ file:///test0.st:4:5 ]
       |
     4 |     done : BOOL;
       |     ^^|^
       |       `--- 2 VAR_OUTPUT are never assigned in the body: 'done', 'value'
       |       |
       |       `--- 'done' is declared here
     5 |     value : INT;
       |     ^^|^^
       |       `---- 'value' is declared here
       |
       | Note: lint rule: uninitialized-output
    ---'
    ");
}

/// In the order written, with the caret on the first. The map they are read from
/// has no order, and its order moves with the interned ids, so an edit moved the
/// caret and reordered the message.
#[rstest]
fn outputs_are_listed_in_the_order_written(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR_OUTPUT
    zeta : INT;
    alpha : INT;
    mid : INT;
    beta : INT;
    omega : INT;
    gamma : INT;
    delta : INT;
    kappa : INT;
END_VAR
    test := 0;
END_FUNCTION
"#;
    let report = test_single_lint(&mut with_db, &[source], "uninitialized-output");
    assert!(
        report.contains(
            "8 VAR_OUTPUT are never assigned in the body: 'zeta', 'alpha', 'mid', 'beta', 'omega', 'gamma', 'delta', 'kappa'"
        ),
        "{report}"
    );
    assert!(report.contains("file:///test0.st:4:5"), "{report}");
}

/// An output written by a chain is assigned.
#[rstest]
fn output_assigned_by_a_chain(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR_OUTPUT result : INT; END_VAR
    result := test := 42;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "uninitialized-output"), @"");
}

/// Every way a body writes an output, whole or in part. Only an assignment to the
/// whole of it counted, so a member, a call's binding, `REF()` and a counter read as
/// never assigned.
#[rstest]
#[case::whole("INT", "w := 1;")]
#[case::an_element("ARRAY[0..1] OF INT", "w[1] := 1;")]
#[case::a_member("S", "w.q := 1;")]
#[case::a_member_of_an_element("ARRAY[0..1] OF S", "w[1].q := 1;")]
#[case::a_bit("WORD", "w.0 := TRUE;")]
#[case::through_this("INT", "THIS^.w := 1;")]
#[case::bound_to_an_output("INT", "G(x => w);")]
#[case::an_element_bound_to_an_output("ARRAY[0..1] OF INT", "G(x => w[1]);")]
#[case::a_member_bound_to_an_output("S", "G(x => w.q);")]
#[case::an_instance_output_bound("INT", "b(o => w);")]
#[case::bound_to_an_in_out("INT", "H(x := w);")]
#[case::an_element_bound_to_an_in_out("ARRAY[0..1] OF INT", "H(x := w[0]);")]
#[case::in_a_nested_call("INT", "i := K(x := w);")]
#[case::referenced("INT", "r := REF(w);")]
#[case::a_member_referenced("S", "r := REF(w.q);")]
#[case::a_counter("INT", "FOR w := 0 TO 1 DO i := 1; END_FOR;")]
#[case::in_a_loop("ARRAY[0..1] OF INT", "FOR i := 0 TO 1 DO w[i] := i; END_FOR;")]
#[case::in_an_if("S", "IF i = 0 THEN w.q := 1; END_IF;")]
#[case::in_a_try("S", "__TRY w.q := 1; __CATCH i := 1; __END_TRY;")]
fn an_output_written_is_not_reported(
    mut with_db: RootDatabase,
    #[case] ty: &str,
    #[case] body: &str,
) {
    assert_eq!(
        test_single_lint(&mut with_db, &[&writes(ty, body)], "uninitialized-output"),
        ""
    );
}

/// A write that reaches something else leaves the output unassigned.
#[rstest]
#[case::not_at_all("INT", "i := 1;")]
#[case::read("INT", "i := w;")]
#[case::bound_to_an_input("INT", "i := I(x := w);")]
#[case::through_it("REF_TO INT", "w^ := 1;")]
#[case::an_instance_of_its_own("INT", "b.o := 1;")]
fn an_output_not_written_is_reported(
    mut with_db: RootDatabase,
    #[case] ty: &str,
    #[case] body: &str,
) {
    let report = test_single_lint(&mut with_db, &[&writes(ty, body)], "uninitialized-output");
    assert!(report.contains("[L0203]"), "{report}");
}

fn writes(ty: &str, body: &str) -> String {
    format!(
        r#"TYPE S : STRUCT q : INT; r : INT; END_STRUCT; END_TYPE
FUNCTION G
VAR_OUTPUT x : INT; END_VAR
x := 1;
END_FUNCTION
FUNCTION H
VAR_IN_OUT x : INT; END_VAR
x := 1;
END_FUNCTION
FUNCTION I : INT
VAR_INPUT x : INT; END_VAR
I := x;
END_FUNCTION
FUNCTION K : INT
VAR_IN_OUT x : INT; END_VAR
K := x;
END_FUNCTION
FUNCTION_BLOCK Other
VAR_OUTPUT o : INT; END_VAR
o := 1;
END_FUNCTION_BLOCK
FUNCTION_BLOCK Fb
VAR_OUTPUT w : {ty}; END_VAR
VAR i : INT; b : Other; r : REF_TO INT; END_VAR
{body}
END_FUNCTION_BLOCK
"#
    )
}
