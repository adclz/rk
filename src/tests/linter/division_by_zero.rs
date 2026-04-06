use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_lint_diagnostics, with_db};

#[rstest]
fn division_by_literal_zero(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    x : INT;
END_VAR
    x := 10 / 0;
END_FUNCTION
"#;
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"
    [L0118] Warning: division by zero
       ,-[ file:///test0.st:6:15 ]
       |
     6 |     x := 10 / 0;
       |               |
       |               `-- division by zero: right-hand side of '/' is 0
       |
       | Note: lint rule: division-by-zero
    ---'
    ");
}

#[rstest]
fn mod_by_literal_zero(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    x : INT;
END_VAR
    x := 10 MOD 0;
END_FUNCTION
"#;
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"
    [L0118] Warning: division by zero
       ,-[ file:///test0.st:6:17 ]
       |
     6 |     x := 10 MOD 0;
       |                 |
       |                 `-- division by zero: right-hand side of 'MOD' is 0
       |
       | Note: lint rule: division-by-zero
    ---'
    ");
}

#[rstest]
fn division_by_nonzero(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    x : INT;
END_VAR
    x := 10 / 2;
END_FUNCTION
"#;
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn division_by_variable(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    x : INT;
    y : INT;
END_VAR
    x := 10 / y;
END_FUNCTION
"#;
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn division_by_parenthesized_zero(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    x : INT;
END_VAR
    x := 10 / (0);
END_FUNCTION
"#;
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"
    [L0118] Warning: division by zero
       ,-[ file:///test0.st:6:15 ]
       |
     6 |     x := 10 / (0);
       |               ^|^
       |                `--- division by zero: right-hand side of '/' is 0
       |
       | Note: lint rule: division-by-zero
    ---'
    ");
}

#[rstest]
fn real_division_by_zero(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : REAL
VAR
    x : REAL;
END_VAR
    x := 10.0 / 0.0;
END_FUNCTION
"#;
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"
    [L0118] Warning: division by zero
       ,-[ file:///test0.st:6:17 ]
       |
     6 |     x := 10.0 / 0.0;
       |                 ^|^
       |                  `--- division by zero: right-hand side of '/' is 0
       |
       | Note: lint rule: division-by-zero
    ---'
    ");
}

#[rstest]
fn typed_numeric_literal_zeros(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    a : SINT;
    b : INT;
    c : DINT;
    d : LINT;
    e : USINT;
    f : UINT;
    g : UDINT;
    h : ULINT;
    i : BYTE;
    j : WORD;
    k : DWORD;
    l : LWORD;
    m : REAL;
    n : LREAL;
END_VAR
    a := 1 / SINT#0;
    b := 1 / INT#0;
    c := 1 / DINT#0;
    d := 1 / LINT#0;
    e := 1 / USINT#0;
    f := 1 / UINT#0;
    g := 1 / UDINT#0;
    h := 1 / ULINT#0;
    i := 1 / BYTE#0;
    j := 1 / WORD#0;
    k := 1 / DWORD#0;
    l := 1 / LWORD#0;
    m := 1.0 / REAL#0.0;
    n := 1.0 / LREAL#0.0;
END_FUNCTION
"#;
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"
    [L0118] Warning: division by zero
        ,-[ file:///test0.st:19:14 ]
        |
     19 |     a := 1 / SINT#0;
        |              ^^^|^^
        |                 `---- division by zero: right-hand side of '/' is 0
        |
        | Note: lint rule: division-by-zero
    ----'
    [L0118] Warning: division by zero
        ,-[ file:///test0.st:20:14 ]
        |
     20 |     b := 1 / INT#0;
        |              ^^|^^
        |                `---- division by zero: right-hand side of '/' is 0
        |
        | Note: lint rule: division-by-zero
    ----'
    [L0118] Warning: division by zero
        ,-[ file:///test0.st:21:14 ]
        |
     21 |     c := 1 / DINT#0;
        |              ^^^|^^
        |                 `---- division by zero: right-hand side of '/' is 0
        |
        | Note: lint rule: division-by-zero
    ----'
    [L0118] Warning: division by zero
        ,-[ file:///test0.st:22:14 ]
        |
     22 |     d := 1 / LINT#0;
        |              ^^^|^^
        |                 `---- division by zero: right-hand side of '/' is 0
        |
        | Note: lint rule: division-by-zero
    ----'
    [L0118] Warning: division by zero
        ,-[ file:///test0.st:23:14 ]
        |
     23 |     e := 1 / USINT#0;
        |              ^^^|^^^
        |                 `----- division by zero: right-hand side of '/' is 0
        |
        | Note: lint rule: division-by-zero
    ----'
    [L0118] Warning: division by zero
        ,-[ file:///test0.st:24:14 ]
        |
     24 |     f := 1 / UINT#0;
        |              ^^^|^^
        |                 `---- division by zero: right-hand side of '/' is 0
        |
        | Note: lint rule: division-by-zero
    ----'
    [L0118] Warning: division by zero
        ,-[ file:///test0.st:25:14 ]
        |
     25 |     g := 1 / UDINT#0;
        |              ^^^|^^^
        |                 `----- division by zero: right-hand side of '/' is 0
        |
        | Note: lint rule: division-by-zero
    ----'
    [L0118] Warning: division by zero
        ,-[ file:///test0.st:26:14 ]
        |
     26 |     h := 1 / ULINT#0;
        |              ^^^|^^^
        |                 `----- division by zero: right-hand side of '/' is 0
        |
        | Note: lint rule: division-by-zero
    ----'
    [L0118] Warning: division by zero
        ,-[ file:///test0.st:27:14 ]
        |
     27 |     i := 1 / BYTE#0;
        |              ^^^|^^
        |                 `---- division by zero: right-hand side of '/' is 0
        |
        | Note: lint rule: division-by-zero
    ----'
    [L0118] Warning: division by zero
        ,-[ file:///test0.st:28:14 ]
        |
     28 |     j := 1 / WORD#0;
        |              ^^^|^^
        |                 `---- division by zero: right-hand side of '/' is 0
        |
        | Note: lint rule: division-by-zero
    ----'
    [L0118] Warning: division by zero
        ,-[ file:///test0.st:29:14 ]
        |
     29 |     k := 1 / DWORD#0;
        |              ^^^|^^^
        |                 `----- division by zero: right-hand side of '/' is 0
        |
        | Note: lint rule: division-by-zero
    ----'
    [L0118] Warning: division by zero
        ,-[ file:///test0.st:30:14 ]
        |
     30 |     l := 1 / LWORD#0;
        |              ^^^|^^^
        |                 `----- division by zero: right-hand side of '/' is 0
        |
        | Note: lint rule: division-by-zero
    ----'
    [L0118] Warning: division by zero
        ,-[ file:///test0.st:31:16 ]
        |
     31 |     m := 1.0 / REAL#0.0;
        |                ^^^^|^^^
        |                    `----- division by zero: right-hand side of '/' is 0
        |
        | Note: lint rule: division-by-zero
    ----'
    [L0118] Warning: division by zero
        ,-[ file:///test0.st:32:16 ]
        |
     32 |     n := 1.0 / LREAL#0.0;
        |                ^^^^|^^^^
        |                    `------ division by zero: right-hand side of '/' is 0
        |
        | Note: lint rule: division-by-zero
    ----'
    ");
}

#[rstest]
fn base_literal_zeros(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    a : INT;
    b : INT;
    c : INT;
END_VAR
    a := 1 / 16#0;
    b := 1 / 2#0;
    c := 1 / 8#0;
END_FUNCTION
"#;
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"
    [L0118] Warning: division by zero
       ,-[ file:///test0.st:8:14 ]
       |
     8 |     a := 1 / 16#0;
       |              ^^|^
       |                `--- division by zero: right-hand side of '/' is 0
       |
       | Note: lint rule: division-by-zero
    ---'
    [L0118] Warning: division by zero
       ,-[ file:///test0.st:9:14 ]
       |
     9 |     b := 1 / 2#0;
       |              ^|^
       |               `--- division by zero: right-hand side of '/' is 0
       |
       | Note: lint rule: division-by-zero
    ---'
    [L0118] Warning: division by zero
        ,-[ file:///test0.st:10:14 ]
        |
     10 |     c := 1 / 8#0;
        |              ^|^
        |               `--- division by zero: right-hand side of '/' is 0
        |
        | Note: lint rule: division-by-zero
    ----'
    ");
}

#[rstest]
fn multiplication_by_zero_no_warning(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    x : INT;
END_VAR
    x := 10 * 0;
END_FUNCTION
"#;
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"");
}
