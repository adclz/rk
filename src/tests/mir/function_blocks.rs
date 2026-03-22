use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::with_db;
use super::utils::mir_exports;

#[rstest]
fn fb_body_exported(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK SR
VAR_INPUT S1, R : BOOL; END_VAR
VAR_OUTPUT Q1 : BOOL; END_VAR
    Q1 := S1 OR ((NOT R) AND Q1);
END_FUNCTION_BLOCK
    "#;
    assert_snapshot!(mir_exports(&mut with_db, &[source]), @"export SR$__body__(*struct(SR))");
}

#[rstest]
fn fb_with_methods(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Counter
VAR
    count : INT;
END_VAR

    count := count + 1;

METHOD GetCount : INT
    GetCount := count;
END_METHOD

END_FUNCTION_BLOCK
    "#;
    assert_snapshot!(mir_exports(&mut with_db, &[source]), @"export Counter$GetCount(*struct(Counter)) -> Int");
}

#[rstest]
fn fb_in_namespace(mut with_db: RootDatabase) {
    let source = r#"
NAMESPACE Std.Bistable
    FUNCTION_BLOCK RS
    VAR_INPUT S, R1 : BOOL; END_VAR
    VAR_OUTPUT Q1 : BOOL; END_VAR
        Q1 := (NOT R1) AND (S OR Q1);
    END_FUNCTION_BLOCK
END_NAMESPACE
    "#;
    assert_snapshot!(mir_exports(&mut with_db, &[source]), @"export RS$__body__(*struct(RS))");
}

#[rstest]
fn fb_instantiation_in_function(mut with_db: RootDatabase) {
    // A function instantiates an FB and calls it
    let source = r#"
FUNCTION_BLOCK SR
VAR_INPUT S1, R : BOOL; END_VAR
VAR_OUTPUT Q1 : BOOL; END_VAR
    Q1 := S1 OR ((NOT R) AND Q1);
END_FUNCTION_BLOCK

FUNCTION test : BOOL
VAR latch : SR; END_VAR
    latch(S1 := TRUE, R := FALSE);
    test := latch.Q1;
END_FUNCTION
    "#;
    assert_snapshot!(mir_exports(&mut with_db, &[source]), @r"
    export SR$__body__(*struct(SR))
    export test() -> Bool
    ");
}

#[rstest]
fn fb_multiple_instances(mut with_db: RootDatabase) {
    // Two separate FB instances should each have their own memory
    let source = r#"
FUNCTION_BLOCK Counter
VAR count : INT; END_VAR
    count := count + 1;
END_FUNCTION_BLOCK

FUNCTION test
VAR c1 : Counter; c2 : Counter; END_VAR
    c1();
    c2();
    c1();
END_FUNCTION
    "#;
    assert_snapshot!(mir_exports(&mut with_db, &[source]), @r"
    export Counter$__body__(*struct(Counter))
    export test()
    ");
}

#[rstest]
fn fb_nested_in_fb(mut with_db: RootDatabase) {
    // FB containing another FB instance
    let source = r#"
FUNCTION_BLOCK Inner
VAR_INPUT x : INT; END_VAR
VAR_OUTPUT y : INT; END_VAR
    y := x * 2;
END_FUNCTION_BLOCK

FUNCTION_BLOCK Outer
VAR inner_fb : Inner; END_VAR
VAR_OUTPUT result : INT; END_VAR
    inner_fb(x := 5);
    result := inner_fb.y;
END_FUNCTION_BLOCK
    "#;
    assert_snapshot!(mir_exports(&mut with_db, &[source]), @r"
    export Inner$__body__(*struct(Inner))
    export Outer$__body__(*struct(Outer))
    ");
}
