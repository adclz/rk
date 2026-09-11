use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

#[rstest]
fn function_missing_return(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION foo : INT
VAR
    x : INT;
END_VAR
    x := 42;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "missing-return"), @r"
    [L0114] Warning: missing return assignment
       ,-[ file:///test0.st:2:10 ]
       |
     2 | FUNCTION foo : INT
       |          ^|^
       |           `--- FUNCTION 'foo' has a return type but never assigns a return value
       |
       | Note: lint rule: missing-return
    ---'
    ");
}

#[rstest]
fn function_with_return(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION foo : INT
    foo := 42;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "missing-return"), @r"");
}

#[rstest]
fn function_return_in_if(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION foo : INT
VAR
    x : BOOL;
END_VAR
    IF x THEN
        foo := 1;
    END_IF;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "missing-return"), @r"");
}

#[rstest]
fn method_missing_return(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK MyFB
    METHOD compute : INT
    VAR
        x : INT;
    END_VAR
        x := 42;
    END_METHOD
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "missing-return"), @r"
    [L0114] Warning: missing return assignment
       ,-[ file:///test0.st:3:12 ]
       |
     3 |     METHOD compute : INT
       |            ^^^|^^^
       |               `----- METHOD 'compute' has a return type but never assigns a return value
       |
       | Note: lint rule: missing-return
    ---'
    ");
}

#[rstest]
fn method_with_return(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK MyFB
    METHOD compute : INT
        compute := 42;
    END_METHOD
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "missing-return"), @r"");
}

/// A variable shadows the return name, the lint should still fire
/// because `MyFn := 0` assigns to the local variable, not the return.
#[rstest]
fn shadowed_name(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION MyFn : INT
VAR
    MyFn : INT;
END_VAR
    MyFn := 0;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "missing-return"), @r"
    [E0107] Error: duplicate definitions
       ,-[ file:///test0.st:4:5 ]
       |
     4 |     MyFn : INT;
       |     ^^|^
       |       `--- variable 'MyFn' is the FUNCTION's return value
    ---'
    [L0114] Warning: missing return assignment
       ,-[ file:///test0.st:2:10 ]
       |
     2 | FUNCTION MyFn : INT
       |          ^^|^
       |            `--- FUNCTION 'MyFn' has a return type but never assigns a return value
       |
       | Note: lint rule: missing-return
    ---'
    ");
}

/// An `{extern}` FUNCTION never assigns its return value in the body — the
/// import's result IS the return, and E1502 refuses a body anyway. The rule
/// exempts it like `empty-body` and `unused-variable` already do.
#[rstest]
fn extern_function_is_exempt(mut with_db: RootDatabase) {
    let source = r#"
        {extern 'acme:io@1' 'poll'}
        FUNCTION MB_POLL : DINT
        VAR_INPUT
            id : DINT;
        END_VAR
        END_FUNCTION
    "#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "missing-return"), @r"");
}

/// A `{wasm}` FUNCTION's body is the pragma, whose `(result NAME)` is the
/// assignment. Every intrinsic in the standard library was flagged.
#[rstest]
fn a_pragma_into_the_return_is_the_assignment(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION ROOT : REAL
        VAR_INPUT
            IN : REAL;
        END_VAR
            {wasm 'f32.sqrt' (params IN) (result ROOT)}
        END_FUNCTION
    "#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "missing-return"), @r"");
}

/// The pragma's `(result NAME)` is the assignment only when NAME is the
/// FUNCTION: one that writes a local and never the return is still missing
/// it.
#[rstest]
fn a_pragma_into_a_local_does_not_assign_the_return(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION ROOT : REAL
        VAR_INPUT
            IN : REAL;
        END_VAR
        VAR
            r : REAL;
        END_VAR
            {wasm 'f32.sqrt' (params IN) (result r)}
        END_FUNCTION
    "#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "missing-return"), @r"
    [L0114] Warning: missing return assignment
       ,-[ file:///test0.st:2:18 ]
       |
     2 |         FUNCTION ROOT : REAL
       |                  ^^|^
       |                    `--- FUNCTION 'ROOT' has a return type but never assigns a return value
       |
       | Note: lint rule: missing-return
    ---'
    ");
}
