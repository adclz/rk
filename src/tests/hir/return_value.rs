// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! Inside a FUNCTION or METHOD, the callable's own name is its return value
//! and is checked as a local of the return type would be.

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_diagnostics, with_db};

/// `INT + DINT` is a DINT, which does not fit back into an INT result.
#[rstest]
fn a_result_widened_by_its_operand_is_not_narrowed_back(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION Narrowed : INT
VAR d : DINT := 100000; END_VAR
    Narrowed := 1;
    Narrowed := Narrowed + d;
END_FUNCTION"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:5:17 ]
       |
     2 | FUNCTION Narrowed : INT
       |          ^^^^|^^^
       |              `----- FUNCTION 'Narrowed' is declared here, with return type 'INT'
       |
     5 |     Narrowed := Narrowed + d;
       |                 ^^^^^^|^^^^^
       |                       `------- expected 'INT', got 'DINT'
       |
       | Help: insert explicit cast 'DINT_TO_INT(Narrowed + d)'
    ---'
    ");
}

/// A literal next to an unsigned result takes the result's type.
#[rstest]
fn an_unsigned_result_takes_a_literal(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION Big : UDINT
    Big := 7;
    IF Big > 5 THEN
        Big := 5;
    END_IF;
END_FUNCTION

FUNCTION Twice : ULINT
    Twice := 3;
    Twice := Twice * 2;
END_FUNCTION"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

/// The literal is checked against the result's type.
#[rstest]
fn a_literal_that_does_not_fit_the_result_is_refused(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION Small : SINT
    Small := 1;
    Small := Small + 300;
END_FUNCTION"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0306] Error: literal out of range
       ,-[ file:///test0.st:4:22 ]
       |
     4 |     Small := Small + 300;
       |              ^^|^^   ^|^
       |                `---------- 'SINT' is expected due to this
       |                       |
       |                       `--- the value does not fit in SINT
       |
       | Note: SINT holds -128 to 127
    ---'
    ");
}

/// A subrange result keeps its bounds.
#[rstest]
fn a_subrange_result_keeps_its_bounds(mut with_db: RootDatabase) {
    let source = r#"
TYPE Pct : INT (0..100); END_TYPE

FUNCTION RetPct : Pct
    RetPct := 500;
END_FUNCTION"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0702] Error: value outside the subrange
       ,-[ file:///test0.st:5:15 ]
       |
     5 |     RetPct := 500;
       |               ^|^
       |                `--- value 500 is outside the subrange 0..100
       |
       | Note: the declared range only admits values from 0 to 100
    ---'
    ");
}

/// Outside its own body, a FUNCTION's or METHOD's name is no value, and is
/// reported once: the expression around it does not report it again.
#[rstest]
fn another_callables_name_is_not_a_value(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION Other : INT
    Other := 1;
END_FUNCTION

FUNCTION UsesOther : INT
    UsesOther := Other + 1;
END_FUNCTION

FUNCTION_BLOCK Fb
    METHOD PUBLIC A : INT
        A := B;
    END_METHOD

    METHOD PUBLIC B : INT
        B := 1;
    END_METHOD
END_FUNCTION_BLOCK"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0317] Error: type name used as a value
       ,-[ file:///test0.st:7:18 ]
       |
     7 |     UsesOther := Other + 1;
       |                  ^^|^^
       |                    `---- 'Other' is not a value
    ---'
    [E0317] Error: type name used as a value
        ,-[ file:///test0.st:12:14 ]
        |
     12 |         A := B;
        |              |
        |              `-- 'B' is not a value
    ----'
    ");
}

/// A METHOD without a return type has no result to assign.
#[rstest]
fn a_void_methods_name_is_not_assignable(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Fb
    METHOD PUBLIC Reset
        Reset := 0;
    END_METHOD
END_FUNCTION_BLOCK"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0319] Error: assignment to a missing return value
       ,-[ file:///test0.st:4:9 ]
       |
     3 |     METHOD PUBLIC Reset
       |                   ^^|^^
       |                     `---- METHOD 'Reset' is declared here
     4 |         Reset := 0;
       |         ^^|^^
       |           `---- 'Reset' has no return value to assign
    ---'
    ");
}
