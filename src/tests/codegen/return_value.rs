//! Inside a FUNCTION or METHOD, the callable's own name is its return value:
//! a variable of the return type. Read in an expression it used to stay the
//! callable, so arithmetic and comparisons on it picked the wrong operand
//! type (a REAL literal added to an LREAL result, TIME compared with LTIME
//! unscaled), a VAR_IN_OUT or a REF() could not take it, and a METHOD's
//! result crashed the lowering.

use crate::tests::codegen::{run, with_db};
use rstest::*;

/// The literal takes the result's type, as it does with a local: an LREAL
/// 0.1, not a REAL one widened afterwards.
#[rstest]
fn a_literal_added_to_the_result_takes_its_type(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION AddTenth : LREAL
            AddTenth := 1.0;
            AddTenth := AddTenth + 0.1;
        END_FUNCTION

        FUNCTION test : BOOL
        VAR x : LREAL := 1.0; END_VAR
            x := x + 0.1;
            test := AddTenth() = x;
        END_FUNCTION
    "#;
    let result: i32 = run(&mut with_db, source, "test", ());
    assert_eq!(result, 1, "AddTenth + 0.1 adds an LREAL 0.1");
}

/// A TIME result compared with an LTIME is widened to nanoseconds first.
#[rstest]
fn a_time_result_compared_with_an_ltime_is_widened(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION LaterThan : TIME
        VAR l : LTIME := LT#1s; END_VAR
            LaterThan := T#2s;
            IF LaterThan > l THEN
                LaterThan := T#1ms;
            END_IF;
        END_FUNCTION

        FUNCTION test : BOOL
            test := LaterThan() = T#1ms;
        END_FUNCTION
    "#;
    let result: i32 = run(&mut with_db, source, "test", ());
    assert_eq!(result, 1, "T#2s > LT#1s");
}

/// A METHOD's result read in an operator, a comparison and a unary minus.
#[rstest]
fn a_method_result_is_read_in_expressions(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Counter
        VAR_INPUT n : INT; END_VAR
            METHOD PUBLIC Double : INT
                Double := n;
                Double := Double + n;
            END_METHOD

            METHOD PUBLIC Capped : INT
                Capped := n;
                IF Capped > 2 THEN
                    Capped := 2;
                END_IF;
            END_METHOD

            METHOD PUBLIC Neg : INT
                Neg := n;
                Neg := -Neg;
            END_METHOD
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR c : Counter; END_VAR
            c(n := 3);
            test := c.Double() * 100 + c.Capped() * 10 - c.Neg();
        END_FUNCTION
    "#;
    let result: i32 = run(&mut with_db, source, "test", ());
    assert_eq!(result, 623, "Double = 6, Capped = 2, Neg = -3");
}

/// The bare name is the result and a call with arguments recurses: `Sum`
/// holds `n` when `Sum(n - 1)` is added to it.
#[rstest]
fn the_bare_name_is_the_result_and_a_call_recurses(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION Sum : INT
        VAR_INPUT n : INT; END_VAR
            Sum := n;
            IF n > 0 THEN
                Sum := Sum + Sum(n - 1);
            END_IF;
        END_FUNCTION

        FUNCTION_BLOCK Fb
            METHOD PUBLIC Down : INT
            VAR_INPUT n : INT; END_VAR
                Down := n;
                IF n > 0 THEN
                    Down := Down + Down(n - 1);
                END_IF;
            END_METHOD
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR f : Fb; END_VAR
            test := Sum(3) * 10 + f.Down(2);
        END_FUNCTION
    "#;
    let result: i32 = run(&mut with_db, source, "test", ());
    assert_eq!(result, 63, "Sum(3) = 6, Down(2) = 3");
}
