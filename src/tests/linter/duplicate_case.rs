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
    [L0109] Warning: duplicate CASE selector
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
    [L0109] Warning: duplicate CASE selector
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
    [L0109] Warning: duplicate CASE selector
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
    [L0109] Warning: duplicate CASE selector
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
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "duplicate-case"), @r"
    [L0109] Warning: duplicate CASE selector
       ,-[ file:///test0.st:6:9 ]
       |
     5 |         INT#1: test := 10;
       |         ^^|^^
       |           `---- CASE selector is already defined here
     6 |         1: test := 20;
       |         |
       |         `-- CASE selector '1' is duplicated, second branch is unreachable
       |
       | Note: lint rule: duplicate-case
    ---'
    ");
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
    [L0109] Warning: duplicate CASE selector
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
    [L0109] Warning: duplicate CASE selector
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

#[rstest]
fn overlapping_ranges(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR x : INT; END_VAR
    CASE x OF
        1..5: test := 10;
        3..8: test := 20;
    END_CASE;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "duplicate-case"), @r"
    [L0109] Warning: duplicate CASE selector
       ,-[ file:///test0.st:6:9 ]
       |
     5 |         1..5: test := 10;
       |         |
       |         `-- overlapping range defined here
     6 |         3..8: test := 20;
       |         |
       |         `-- CASE range '3..8' overlaps with '1..5'
       |
       | Note: lint rule: duplicate-case
    ---'
    ");
}

#[rstest]
fn value_covered_by_range(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR x : INT; END_VAR
    CASE x OF
        1..10: test := 10;
        5: test := 20;
    END_CASE;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "duplicate-case"), @r"
    [L0109] Warning: duplicate CASE selector
       ,-[ file:///test0.st:6:9 ]
       |
     5 |         1..10: test := 10;
       |         |
       |         `-- range defined here
     6 |         5: test := 20;
       |         |
       |         `-- CASE selector '5' is already covered by range '1..10'
       |
       | Note: lint rule: duplicate-case
    ---'
    ");
}

#[rstest]
fn range_covers_existing_value(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR x : INT; END_VAR
    CASE x OF
        5: test := 10;
        1..10: test := 20;
    END_CASE;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "duplicate-case"), @r"
    [L0109] Warning: duplicate CASE selector
       ,-[ file:///test0.st:6:9 ]
       |
     5 |         5: test := 10;
       |         |
       |         `-- selector defined here
     6 |         1..10: test := 20;
       |         |
       |         `-- CASE range '1..10' covers already defined selector '5'
       |
       | Note: lint rule: duplicate-case
    ---'
    ");
}

#[rstest]
fn non_overlapping_ranges_no_warning(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR x : INT; END_VAR
    CASE x OF
        1..5: test := 10;
        6..10: test := 20;
    END_CASE;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "duplicate-case"), @r"");
}

/// Two string labels that DENOTE the same value are one label, however they
/// are written: the typed form, and an escape against the byte it decodes to.
/// The lint keys on the value HIR evaluated, so both domains — integers and
/// strings — collide by meaning rather than by spelling.
#[rstest]
fn string_labels_collide_by_value(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR s : STRING; y : INT; END_VAR
    CASE s OF
        'A': y := 1;
        '$41': y := 2;
    ELSE
        y := 0;
    END_CASE;
    test := y;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "duplicate-case"), @r"
    [L0109] Warning: duplicate CASE selector
       ,-[ file:///test0.st:6:9 ]
       |
     5 |         'A': y := 1;
       |         ^|^
       |          `--- CASE selector is already defined here
     6 |         '$41': y := 2;
       |         ^^|^^
       |           `---- CASE selector ''$41'' is duplicated, second branch is unreachable
       |
       | Note: lint rule: duplicate-case
    ---'
    ");
}
