// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

#[rstest]
fn case_with_else_no_warning(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION test : INT
        VAR
            x : INT;
        END_VAR
            CASE x OF
                1: test := 1;
                2: test := 2;
            ELSE
                test := 0;
            END_CASE;
        END_FUNCTION
    "#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "case-without-else"), @r"");
}

#[rstest]
fn case_without_else_warning(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION test : INT
        VAR
            x : INT;
        END_VAR
            CASE x OF
                1: test := 1;
                2: test := 2;
            END_CASE;
        END_FUNCTION
    "#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "case-without-else"), @r"
    [L0209] Info: CASE without ELSE
       ,-[ file:///test0.st:6:13 ]
       |
     6 | ,->             CASE x OF
       : :
     9 | |->             END_CASE;
       | |
       | `--------------------------- CASE statement has no ELSE branch
       |
       |     Note: lint rule: case-without-else
    ---'
    ");
}

#[rstest]
fn nested_case_inner_missing_else(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION test : INT
        VAR
            x : INT;
            y : INT;
        END_VAR
            CASE x OF
                1:
                    CASE y OF
                        10: test := 10;
                    END_CASE;
                2: test := 2;
            ELSE
                test := 0;
            END_CASE;
        END_FUNCTION
    "#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "case-without-else"), @r"
    [L0209] Info: CASE without ELSE
        ,-[ file:///test0.st:9:21 ]
        |
      9 | ,->                     CASE y OF
        : :
     11 | |->                     END_CASE;
        | |
        | `----------------------------------- CASE statement has no ELSE branch
        |
        |     Note: lint rule: case-without-else
    ----'
    ");
}

#[rstest]
fn case_in_program(mut with_db: RootDatabase) {
    let source = r#"
        PROGRAM prog1
        VAR
            x : INT;
            result : INT;
        END_VAR
            CASE x OF
                1: result := 1;
            END_CASE;
        END_PROGRAM
    "#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "case-without-else"), @r"
    [L0209] Info: CASE without ELSE
       ,-[ file:///test0.st:7:13 ]
       |
     7 | ,->             CASE x OF
       : :
     9 | |->             END_CASE;
       | |
       | `--------------------------- CASE statement has no ELSE branch
       |
       |     Note: lint rule: case-without-else
    ---'
    ");
}

/// A CASE over an enum that names every variant runs a branch for every
/// value: no ELSE is missing.
#[rstest]
fn every_enum_variant_handled(mut with_db: RootDatabase) {
    let source = r#"
        TYPE Color : (Red, Green, Blue); END_TYPE

        FUNCTION test : INT
        VAR c : Color; END_VAR
            CASE c OF
                Color#Red: test := 1;
                Color#GREEN: test := 2;
                Color#Blue: test := 3;
            END_CASE;
        END_FUNCTION
    "#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "case-without-else"), @r"");
}

/// The variants left out are named.
#[rstest]
fn missing_enum_variants_named(mut with_db: RootDatabase) {
    let source = r#"
        TYPE Color : (Red, Green, Blue, Black); END_TYPE

        FUNCTION test : INT
        VAR c : Color; END_VAR
            CASE c OF
                Color#Red: test := 1;
                Color#Black: test := 4;
            END_CASE;
        END_FUNCTION
    "#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "case-without-else"), @r"
    [L0209] Info: CASE without ELSE
       ,-[ file:///test0.st:6:13 ]
       |
     6 | ,->             CASE c OF
       : :
     9 | |->             END_CASE;
       | |
       | `--------------------------- CASE statement does not handle 'Color#Green', 'Color#Blue'
       |
       |     Help: add a branch for each, or an ELSE branch
       |
       |     Note: lint rule: case-without-else
    ---'
    ");
}

/// Labels that cover the selector's whole range, as single values and
/// ranges in any order, leave nothing for an ELSE: a SINT, a USINT split in
/// two, a subrange's bounds.
#[rstest]
fn whole_integer_range_handled(mut with_db: RootDatabase) {
    let source = r#"
        TYPE Level : INT (0..10); END_TYPE

        FUNCTION test : INT
        VAR s : SINT; u : USINT; l : Level; END_VAR
            CASE s OF
                -128..127: test := 1;
            END_CASE;
            CASE u OF
                128..255: test := 2;
                0: test := 3;
                1..127: test := 4;
            END_CASE;
            CASE l OF
                0..4: test := 5;
                5, 6..10: test := 6;
            END_CASE;
        END_FUNCTION
    "#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "case-without-else"), @r"");
}

/// One value short of the range is still a CASE without ELSE.
#[rstest]
fn integer_range_with_a_gap(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION test : INT
        VAR u : USINT; END_VAR
            CASE u OF
                0..99: test := 1;
                101..255: test := 2;
            END_CASE;
        END_FUNCTION
    "#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "case-without-else"), @r"
    [L0209] Info: CASE without ELSE
       ,-[ file:///test0.st:4:13 ]
       |
     4 | ,->             CASE u OF
       : :
     7 | |->             END_CASE;
       | |
       | `--------------------------- CASE statement has no ELSE branch
       |
       |     Note: lint rule: case-without-else
    ---'
    ");
}
