use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

#[rstest]
fn duplicate_integer_selector(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR x : INT; END_VAR
    CASE x OF
        1: test := 10;
        2: test := 20;
        1: test := 30;
    END_CASE;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "duplicate-case"), @r"
    [L0127] Warning: duplicate CASE selector
       ,-[ file:///test0.st:7:9 ]
       |
     5 |         1: test := 10;
       |         |
       |         `-- CASE selector is already defined here
       |
     7 |         1: test := 30;
       |         |
       |         `-- CASE selector '1' is duplicated, second branch is unreachable
       |
       | Note: lint rule: duplicate-case
    ---'
    ");
}

#[rstest]
fn duplicate_zero(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR x : INT; END_VAR
    CASE x OF
        0: test := 10;
        1: test := 20;
        0: test := 30;
    END_CASE;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "duplicate-case"), @r"
    [L0127] Warning: duplicate CASE selector
       ,-[ file:///test0.st:7:9 ]
       |
     5 |         0: test := 10;
       |         |
       |         `-- CASE selector is already defined here
       |
     7 |         0: test := 30;
       |         |
       |         `-- CASE selector '0' is duplicated, second branch is unreachable
       |
       | Note: lint rule: duplicate-case
    ---'
    ");
}

#[rstest]
fn three_duplicates(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR x : INT; END_VAR
    CASE x OF
        5: test := 10;
        5: test := 20;
        5: test := 30;
    END_CASE;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "duplicate-case"), @r"
    [L0127] Warning: duplicate CASE selector
       ,-[ file:///test0.st:6:9 ]
       |
     5 |         5: test := 10;
       |         |
       |         `-- CASE selector is already defined here
     6 |         5: test := 20;
       |         |
       |         `-- CASE selector '5' is duplicated, second branch is unreachable
       |
       | Note: lint rule: duplicate-case
    ---'
    [L0127] Warning: duplicate CASE selector
       ,-[ file:///test0.st:7:9 ]
       |
     6 |         5: test := 20;
       |         |
       |         `-- CASE selector is already defined here
     7 |         5: test := 30;
       |         |
       |         `-- CASE selector '5' is duplicated, second branch is unreachable
       |
       | Note: lint rule: duplicate-case
    ---'
    ");
}

#[rstest]
fn typed_literal_duplicate(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR x : INT; END_VAR
    CASE x OF
        INT#1: test := 10;
        1: test := 20;
    END_CASE;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "duplicate-case"), @"");
}

#[rstest]
fn subrange_duplicate(mut with_db: RootDatabase) {
    let source = r#"
        TYPE
            Range: UINT (0..10);
        END_TYPE

FUNCTION test : INT
VAR x : Range; END_VAR
    CASE x OF
        0..2: test := 10;
        0..2: test := 20;
    END_CASE;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "duplicate-case"), @r"
    [L0127] Warning: duplicate CASE selector
        ,-[ file:///test0.st:10:9 ]
        |
      9 |         0..2: test := 10;
        |         |
        |         `-- CASE selector is already defined here
     10 |         0..2: test := 20;
        |         |
        |         `-- CASE range '0..2' is duplicated, second branch is unreachable
        |
        | Note: lint rule: duplicate-case
    ----'
    ");
}

#[rstest]
fn enum_duplicate(mut with_db: RootDatabase) {
    let source = r#"
TYPE STATE: INT(A, B, C) END_TYPE
FUNCTION test : INT
VAR x : STATE; END_VAR
    CASE x OF
        STATE#A: test := 10;
        STATE#B: test := 20;
        STATE#A: test := 20;
    END_CASE;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "duplicate-case"), @r"
    [L0127] Warning: duplicate CASE selector
       ,-[ file:///test0.st:8:9 ]
       |
     6 |         STATE#A: test := 10;
       |         ^^^|^^^
       |            `----- CASE selector is already defined here
       |
     8 |         STATE#A: test := 20;
       |         ^^^|^^^
       |            `----- CASE selector 'STATE#A' is duplicated, second branch is unreachable
       |
       | Note: lint rule: duplicate-case
    ---'
    ");
}

// should have no errors since STATE and STATE2 are not the same enums
#[rstest]
fn mixed_enum_duplicate(mut with_db: RootDatabase) {
    let source = r#"
TYPE STATE: INT(A, B, C) END_TYPE
TYPE STATE2: INT(A, B, C) END_TYPE

FUNCTION test : INT
VAR x : STATE; END_VAR
    CASE x OF
        STATE#A: test := 10;
        STATE#B: test := 20;
        STATE2#A: test := 20;
    END_CASE;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "duplicate-case"), @"");
}
