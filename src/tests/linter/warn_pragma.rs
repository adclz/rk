// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{add_library_sources, test_single_lint, with_db};

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

    assert_snapshot!(test_single_lint(&mut with_db, &[source], "warn-pragma"), @r"
    [L0002] Warning: {warn} notice
       ,-[ file:///test0.st:8:10 ]
       |
     2 | {warn = 'this function is deprecated, use fn2 instead'}
       | ^^^^^^^^^^^^^^^^^^^^^^^^^^^|^^^^^^^^^^^^^^^^^^^^^^^^^^^
       |                            `----------------------------- the pragma is declared here
       |
     8 |     x := fn1();
       |          ^|^
       |           `--- this function is deprecated, use fn2 instead
       |
       | Note: lint rule: warn-pragma
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

    assert_snapshot!(test_single_lint(&mut with_db, &[source], "warn-pragma"), @r"
    [L0001] Info: {info} notice
       ,-[ file:///test0.st:8:10 ]
       |
     2 | {info = 'prefer new_fn for better performance'}
       | ^^^^^^^^^^^^^^^^^^^^^^^|^^^^^^^^^^^^^^^^^^^^^^^
       |                        `------------------------- the pragma is declared here
       |
     8 |     x := old_fn();
       |          ^^^|^^
       |             `---- prefer new_fn for better performance
       |
       | Note: lint rule: warn-pragma
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

    assert_snapshot!(test_single_lint(&mut with_db, &[source], "warn-pragma"), @r"
    [L0002] Warning: {warn} notice
       ,-[ file:///test0.st:9:5 ]
       |
     2 | {warn = 'use NewFB instead'}
       | ^^^^^^^^^^^^^^|^^^^^^^^^^^^^
       |               `--------------- the pragma is declared here
       |
     9 |     fb(_x := 1);
       |     ^|
       |      `-- use NewFB instead
       |
       | Note: lint rule: warn-pragma
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

    assert_snapshot!(test_single_lint(&mut with_db, &[source], "warn-pragma"), @r"
    [L0002] Warning: {warn} notice
        ,-[ file:///test0.st:11:13 ]
        |
      4 |     {warn = 'this method is deprecated'}
        |     ^^^^^^^^^^^^^^^^^^|^^^^^^^^^^^^^^^^^
        |                       `------------------- the pragma is declared here
        |
     11 |     y := fb.doStuff();
        |             ^^^|^^^
        |                `----- this method is deprecated
        |
        | Note: lint rule: warn-pragma
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

    assert_snapshot!(test_single_lint(&mut with_db, &[source], "warn-pragma"), @r"
    [L0002] Warning: {warn} notice
       ,-[ file:///test0.st:8:10 ]
       |
     2 | {warn = 'deprecated'}
       | ^^^^^^^^^^|^^^^^^^^^^
       |           `------------ the pragma is declared here
       |
     8 |     x := old();
       |          ^|^
       |           `--- deprecated
       |
       | Note: lint rule: warn-pragma
    ---'
    [L0002] Warning: {warn} notice
        ,-[ file:///test0.st:13:10 ]
        |
      2 | {warn = 'deprecated'}
        | ^^^^^^^^^^|^^^^^^^^^^
        |           `------------ the pragma is declared here
        |
     13 |     y := old();
        |          ^|^
        |           `--- deprecated
        |
        | Note: lint rule: warn-pragma
    ----'
    ");
}

/// `REAL_TO_DWORD` and `LREAL_TO_LWORD` copy the bits, and lose nothing: they
/// carried the stdlib's narrowing notice, so every caller got an info, and
/// E1431 now sends a REAL's partial access to them. A real narrowing keeps it.
#[rstest]
fn a_bit_copy_is_not_a_narrowing(mut with_db: RootDatabase) {
    add_library_sources(&mut with_db, &[include_str!("../../../stdlib/Convert.st")]);
    let source = r#"
USING Std.Convert;

FUNCTION caller : BOOL
VAR r : REAL; lr : LREAL; d : DWORD; l : LWORD; s : USINT; END_VAR
    d := REAL_TO_DWORD(r);
    l := LREAL_TO_LWORD(lr);
    s := LINT_TO_USINT(LINT#300);
    caller := d.31 OR l.63 OR s.0;
END_FUNCTION"#;

    assert_snapshot!(test_single_lint(&mut with_db, &[source], "warn-pragma"), @r"
    [L0001] Info: {info} notice
         ,-[ file:///test0.st:8:10 ]
         |
       8 |     s := LINT_TO_USINT(LINT#300);
         |          ^^^^^^|^^^^^^
         |                `-------- narrowing conversion, possible loss of value
         |
         |-[ file:///lib0.st:369:2 ]
         |
     369 |     {info = 'narrowing conversion, possible loss of value'}
         |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^|^^^^^^^^^^^^^^^^^^^^^^^^^^^
         |                                `----------------------------- the pragma is declared here
         |
         | Note: lint rule: warn-pragma
    -----'
    ");
}
