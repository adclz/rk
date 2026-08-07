//! `MOD` operand typing.
//!
//! IEC: MOD takes ANY_INT. That is narrower than the other multiplicative
//! operators, and codegen depends on it — MOD lowers to an integer remainder,
//! which has no float form. Judged by the generic numeric rule instead, a REAL
//! operand passed `rk check` and produced a module that fails wasm validation.

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

/// Every integer width is accepted, signed and unsigned.
#[rstest]
fn valid_mod_with_integer_operands(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION fn1 : INT
        VAR
            a : INT := 17;
            b : INT := 5;
            u : UINT := 9;
            l : LINT := 100;
            r : INT;
        END_VAR
            r := a MOD b;
            r := 17 MOD 5;
            u := u MOD UINT#4;
            l := l MOD LINT#7;
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

/// A REAL operand is rejected.
#[rstest]
fn invalid_mod_with_real_operands(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION fn1 : REAL
        VAR
            a : REAL := 7.5;
            b : REAL := 2.0;
            r : REAL;
        END_VAR
            r := a MOD b;
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0318] Error: type mismatch
       ,-[ file:///test0.st:8:18 ]
       |
     8 |             r := a MOD b;
       |                  ^^^|^^^
       |                     `----- operator 'MOD' cannot be applied to type 'REAL'
    ---'
    ");
}

/// One real operand is enough: the join is what used to be judged, and it hid
/// a real operand behind an integer one.
#[rstest]
fn invalid_mod_with_one_real_operand(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION fn1 : REAL
        VAR
            a : INT := 7;
            b : REAL := 2.0;
            r : REAL;
        END_VAR
            r := a MOD b;
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0318] Error: type mismatch
       ,-[ file:///test0.st:8:18 ]
       |
     8 |             r := a MOD b;
       |                  ^^^|^^^
       |                     `----- operator 'MOD' cannot be applied to type 'REAL'
    ---'
    ");
}

/// A subrange takes its base type's rule: `INT (0..100)` is an integer, so it
/// is a valid MOD operand.
#[rstest]
fn subrange_follows_its_base_type(mut with_db: RootDatabase) {
    let source = r#"
        TYPE
            Small : INT (0..100);
        END_TYPE

        FUNCTION fn1 : INT
        VAR
            s : Small := 17;
            r : INT;
        END_VAR
            r := s MOD 5;
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}
