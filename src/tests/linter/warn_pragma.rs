use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_lint_diagnostics, with_db};

#[rstest]
fn warn_pragma_on_function(mut with_db: RootDatabase) {
    let source = r#"
{warn = 'this function is deprecated, use fn2 instead'}
FUNCTION fn1 : INT
END_FUNCTION

FUNCTION caller : INT
VAR x : INT; END_VAR
    x := fn1();
END_FUNCTION"#;

    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"
    [L0117] Warning: call site notice
       ,-[ file:///test0.st:8:10 ]
       |
     3 | FUNCTION fn1 : INT
       |          ^|^
       |           `--- notice emitted here
       |
     8 |     x := fn1();
       |          ^|^
       |           `--- this function is deprecated, use fn2 instead
    ---'
    ");
}

#[rstest]
fn info_pragma_on_function(mut with_db: RootDatabase) {
    let source = r#"
{info = 'prefer new_fn for better performance'}
FUNCTION old_fn : INT
END_FUNCTION

FUNCTION caller : INT
VAR x : INT; END_VAR
    x := old_fn();
END_FUNCTION"#;

    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"
    [L0117] Advice: call site notice
       ,-[ file:///test0.st:8:10 ]
       |
     3 | FUNCTION old_fn : INT
       |          ^^^|^^
       |             `---- notice emitted here
       |
     8 |     x := old_fn();
       |          ^^^|^^
       |             `---- prefer new_fn for better performance
    ---'
    ");
}

#[rstest]
fn warn_pragma_on_function_block(mut with_db: RootDatabase) {
    let source = r#"
{warn = 'use NewFB instead'}
FUNCTION_BLOCK OldFB
VAR_INPUT _x : INT; END_VAR
END_FUNCTION_BLOCK

FUNCTION caller : INT
VAR fb : OldFB; END_VAR
    fb(_x := 1);
END_FUNCTION"#;

    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"
    [L0117] Warning: call site notice
       ,-[ file:///test0.st:9:5 ]
       |
     3 | FUNCTION_BLOCK OldFB
       |                ^^|^^
       |                  `---- notice emitted here
       |
     9 |     fb(_x := 1);
       |     ^|
       |      `-- use NewFB instead
    ---'
    ");
}

#[rstest]
fn warn_pragma_on_method(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK MyFB
VAR_INPUT _x : INT; END_VAR
    {warn = 'this method is deprecated'}
    METHOD PUBLIC doStuff : INT
    END_METHOD
END_FUNCTION_BLOCK

FUNCTION caller : INT
VAR fb : MyFB; y : INT; END_VAR
    y := fb.doStuff();
END_FUNCTION"#;

    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"
    [L0117] Warning: call site notice
        ,-[ file:///test0.st:11:13 ]
        |
      5 |     METHOD PUBLIC doStuff : INT
        |                   ^^^|^^^
        |                      `----- notice emitted here
        |
     11 |     y := fb.doStuff();
        |             ^^^|^^^
        |                `----- this method is deprecated
    ----'
    ");
}

#[rstest]
fn warn_pragma_multiple_call_sites(mut with_db: RootDatabase) {
    let source = r#"
{warn = 'deprecated'}
FUNCTION old : INT
END_FUNCTION

FUNCTION caller1 : INT
VAR x : INT; END_VAR
    x := old();
END_FUNCTION

FUNCTION caller2 : INT
VAR y : INT; END_VAR
    y := old();
END_FUNCTION"#;

    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"
    [L0117] Warning: call site notice
       ,-[ file:///test0.st:8:10 ]
       |
     3 | FUNCTION old : INT
       |          ^|^
       |           `--- notice emitted here
       |
     8 |     x := old();
       |          ^|^
       |           `--- deprecated
    ---'
    [L0117] Warning: call site notice
        ,-[ file:///test0.st:13:10 ]
        |
      3 | FUNCTION old : INT
        |          ^|^
        |           `--- notice emitted here
        |
     13 |     y := old();
        |          ^|^
        |           `--- deprecated
    ----'
    ");
}
