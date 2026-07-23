//! FUNCTION overload resolution — a call binds to the same-name overload whose
//! parameter count matches the call's argument count. The discriminant is the
//! arity (see `Function::param_count`); same-arity collisions are rejected by
//! the duplicate check (see `duplicates.rs`), so resolution is unambiguous.

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

// A 2-arg call binds to the 2-arg overload even though the 1-arg overload is
// declared first (and would be the naive first-match). The return types differ
// (INT vs STRING), so the assignment only type-checks if the *right* overload
// was selected.
#[rstest]
fn call_binds_to_arity_matching_overload(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION foo : INT
VAR_INPUT a : INT; END_VAR
    foo := a;
END_FUNCTION

FUNCTION foo : STRING
VAR_INPUT a : INT; b : INT; END_VAR
    foo := 'x';
END_FUNCTION

FUNCTION test : INT
VAR
    i : INT;
    s : STRING;
END_VAR
    i := foo(1);        // -> foo/1 : INT
    s := foo(1, 2);     // -> foo/2 : STRING
END_FUNCTION
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

// Selecting the wrong overload would surface as a type error: if `foo(1, 2)`
// bound to the 1-arg INT overload, assigning its result to a STRING fails.
#[rstest]
fn wrong_overload_would_type_error(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION foo : INT
VAR_INPUT a : INT; END_VAR
    foo := a;
END_FUNCTION

FUNCTION foo : STRING
VAR_INPUT a : INT; b : INT; END_VAR
    foo := 'x';
END_FUNCTION

FUNCTION test : INT
VAR i : INT; END_VAR
    i := foo(1, 2);     // foo/2 returns STRING -> STRING := INT is a type error
END_FUNCTION
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
        ,-[ file:///test0.st:14:10 ]
        |
     13 | VAR i : INT; END_VAR
        |     |
        |     `-- type is declared by variable 'i' here
     14 |     i := foo(1, 2);     // foo/2 returns STRING -> STRING := INT is a type error
        |          ^^^^|^^^^
        |              `------ expected 'INT', got 'STRING'
    ----'
    ");
}

// An argument count with no matching overload keeps the first-match, so the
// ordinary arity error still surfaces (no silent success).
#[rstest]
fn no_matching_arity_reports_error(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION foo : INT
VAR_INPUT a : INT; END_VAR
    foo := a;
END_FUNCTION

FUNCTION foo : INT
VAR_INPUT a : INT; b : INT; END_VAR
    foo := a + b;
END_FUNCTION

FUNCTION test : INT
VAR x : INT; END_VAR
    test := foo(x, x, x);   // no 3-arg overload
END_FUNCTION
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0205] Error: function call parameter mismatch
        ,-[ file:///test0.st:14:13 ]
        |
     14 |     test := foo(x, x, x);   // no 3-arg overload
        |             ^|^
        |              `--- 'foo' expects 1 parameter, but got 3
    ----'
    [E0206] Error: function call parameter mismatch
        ,-[ file:///test0.st:14:20 ]
        |
     14 |     test := foo(x, x, x);   // no 3-arg overload
        |                    |
        |                    `-- no parameter at index '1'
    ----'
    [E0206] Error: function call parameter mismatch
        ,-[ file:///test0.st:14:23 ]
        |
     14 |     test := foo(x, x, x);   // no 3-arg overload
        |                       |
        |                       `-- no parameter at index '2'
    ----'
    ");
}
