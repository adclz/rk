//! FUNCTION overload resolution — a call binds to the same-name overload whose
//! signature (ordered parameter types, see `function_signature`) matches the
//! call's argument types. Overloads that are exact-on-every-arg win; when an
//! argument fits several by widening and none is exact, the call is ambiguous
//! (E0237) rather than guessed. Identical signatures are rejected as duplicates
//! (see `duplicates.rs`).

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

// Same arity, different parameter TYPE is a legal overload set (not a
// duplicate), and a call binds by argument type. Distinct return types (INT vs
// BOOL) make a wrong pick a type error, so a clean run proves correct selection.
#[rstest]
fn overload_by_parameter_type(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION describe : INT
VAR_INPUT x : INT; END_VAR
    describe := 1;
END_FUNCTION

FUNCTION describe : BOOL
VAR_INPUT x : REAL; END_VAR
    describe := TRUE;
END_FUNCTION

FUNCTION test : INT
VAR i : INT; r : REAL; n : INT; b : BOOL; END_VAR
    n := describe(i);   // -> describe(INT) : INT
    b := describe(r);   // -> describe(REAL) : BOOL
    test := n;
END_FUNCTION
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

// When an argument fits several overloads and none is an exact match, the
// compiler refuses to guess and reports E0237. `pick(1)` — the untyped literal
// widens to both DINT and LINT.
#[rstest]
fn ambiguous_overload_is_rejected(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION pick : INT
VAR_INPUT x : DINT; END_VAR
    pick := 1;
END_FUNCTION

FUNCTION pick : INT
VAR_INPUT x : LINT; END_VAR
    pick := 2;
END_FUNCTION

FUNCTION test : INT
    test := pick(1);
END_FUNCTION
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0237] Error: ambiguous overloaded call
        ,-[ file:///test0.st:13:13 ]
        |
      2 | ,---> FUNCTION pick : INT
        : :
      5 | |---> END_FUNCTION
        | |
        | `-------------------- candidate overload declared here
        |
      7 |   ,-> FUNCTION pick : INT
        :   :
     10 |   |-> END_FUNCTION
        |   |
        |   `------------------ candidate overload declared here
        |
     13 |           test := pick(1);
        |                   ^^|^
        |                     `--- call to 'pick' is ambiguous: 2 overloads accept these arguments: disambiguate with an explicit cast
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
