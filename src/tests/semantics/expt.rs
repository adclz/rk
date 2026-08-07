//! `**` operand typing.
//!
//! IEC: `IN1 ** IN2` takes IN1 of ANY_REAL and IN2 of ANY_NUM, and the result
//! is IN1's type. That is narrower than the other arithmetic operators, and
//! codegen depends on it — `**` lowers to a float pow, which has no integer
//! form. Judged by the operand join instead, an integer base passed `rk check`
//! and produced a module that fails wasm validation.

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

/// A REAL or LREAL base is accepted, and the exponent may be any numeric —
/// including an INT variable, which is cast to the base's type.
#[rstest]
fn valid_power_with_real_base(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION fn1 : REAL
        VAR
            a : REAL := 2.0;
            b : LREAL := 3.0;
            n : INT := 3;
            r : REAL;
            l : LREAL;
        END_VAR
            r := a ** 3.0;
            r := a ** n;
            l := b ** 2.0;
            r := 2.0 ** 3.0;
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

/// A declared integer base is rejected: IEC requires ANY_REAL for IN1, and
/// there is no integer pow to lower it to.
#[rstest]
fn invalid_power_with_integer_base(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION fn1 : REAL
        VAR
            i : INT := 2;
            r : REAL;
        END_VAR
            r := i ** 3.0;
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0318] Error: type mismatch
       ,-[ file:///test0.st:7:18 ]
       |
     4 |             i : INT := 2;
       |             |
       |             `-- type is declared by variable 'i' here
       |
     7 |             r := i ** 3.0;
       |                  ^^^^|^^^
       |                      `----- operator '**' cannot be applied to type 'INT'
    ---'
    ");
}

/// An integer LITERAL base is rejected too: a bare `2` takes its default
/// type, INT. Write `2.0 ** 3.0`. The exponent resolves in its own context,
/// so nothing is said about it.
#[rstest]
fn invalid_power_with_integer_literal_base(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION fn1 : REAL
        VAR
            r : REAL;
        END_VAR
            r := 2 ** 3.0;
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0318] Error: type mismatch
       ,-[ file:///test0.st:6:18 ]
       |
     6 |             r := 2 ** 3.0;
       |                  ^^^^|^^^
       |                      `----- operator '**' cannot be applied to type 'INT'
    ---'
    ");
}
