//! A constant expression folds to what the program computes: each operation
//! at its type, wrapping at that type's width, wherever it is written — an
//! initializer, a CASE label, a FOR step, a bound.

use crate::tests::codegen::with_db;
use rstest::*;

/// `200 * 200` multiplies two INTs, so it is -25536 in an initializer as in
/// an assignment. The initializer used to fold it in 64 bits to 40000, and
/// the parenthesized one did not fold at all, so the three disagreed.
#[rstest]
fn an_initializer_folds_as_the_assignment_computes(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION run : DINT
        VAR
            a : DINT := 200 * 200;
            b : DINT := (200 * 200);
            c : DINT;
            d : DINT := DINT#200 * 200;
        END_VAR
            c := 200 * 200;
            IF a = c AND b = c AND d = 40000 THEN
                run := a;
            END_IF;
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "run", ());
    assert_eq!(
        result, -25536,
        "INT arithmetic wraps in all three; DINT#200 * 200 is 40000"
    );
}

/// A label computes at its type like the selector's value: `K + 1` on a
/// SINT K = 127 is -128, and `K * 3` is 125. They were 128 and 381, which no
/// SINT selector holds, so the branches never ran.
#[rstest]
fn a_case_label_wraps_like_the_value_it_matches(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION pick : DINT
        VAR_INPUT s : SINT; END_VAR
        VAR CONSTANT K : SINT := 127; END_VAR
            CASE s OF
                K + 1: pick := 1;
                K * 3: pick := 2;
            ELSE
                pick := 0;
            END_CASE;
        END_FUNCTION

        FUNCTION run : DINT
        VAR CONSTANT K : SINT := 127; END_VAR
        VAR s : SINT; END_VAR
            s := K + 1;
            run := pick(s := s) * 10;
            s := K * 3;
            run := run + pick(s := s);
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "run", ());
    assert_eq!(result, 12, "K + 1 picks 1, K * 3 picks 2");
}

/// A ULINT label above `i64::MAX` is a constant like any other.
#[rstest]
fn a_ulint_label_above_i64_max_matches(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION pick : DINT
        VAR_INPUT v : ULINT; END_VAR
            CASE v OF
                18446744073709551615: pick := 1;
                9223372036854775808: pick := 2;
            ELSE
                pick := 0;
            END_CASE;
        END_FUNCTION

        FUNCTION run : DINT
            run := pick(v := ULINT#18446744073709551615) * 10
                + pick(v := ULINT#9223372036854775808);
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "run", ());
    assert_eq!(result, 12, "the maximum picks 1, 2**63 picks 2");
}

/// Parentheses and a sign fold around a CONSTANT, not only around a literal:
/// in a bound, a subrange, a label and a step.
#[rstest]
fn parentheses_and_a_sign_fold_around_a_constant(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION run : DINT
        VAR CONSTANT K : INT := 5; END_VAR
        VAR
            a : ARRAY[-K..(K + 1) * 2] OF INT;
            s : INT(-K..K);
            x : INT := 12;
            y : INT := -5;
            i : INT;
        END_VAR
            a[-5] := 1;
            a[12] := 2;
            s := y;
            CASE x OF (K + 1) * 2: run := run + 10; END_CASE;
            CASE y OF -K: run := run + 100; END_CASE;
            FOR i := 10 TO 0 BY -K DO
                run := run + 1000;
            END_FOR;
            run := run + a[-5] + a[12] + s;
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "run", ());
    assert_eq!(
        result, 3108,
        "both labels match, the step runs 10, 5, 0, and a[-5] + a[12] + s is -2"
    );
}

/// A CONSTANT defined from CONSTANTs is a label and a step: its initializer
/// folds where it is declared, not in the body that names it.
#[rstest]
fn a_constant_from_constants_is_a_label_and_a_step(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION pick : DINT
        VAR_INPUT x : INT; END_VAR
        VAR CONSTANT A : INT := 2; B : INT := A * 2; END_VAR
            CASE x OF
                A: pick := 1;
                B: pick := 2;
            ELSE
                pick := 0;
            END_CASE;
        END_FUNCTION

        FUNCTION run : DINT
        VAR CONSTANT ONE : INT := 1; TWO : INT := ONE + ONE; END_VAR
        VAR i : INT; END_VAR
            FOR i := 0 TO 6 BY TWO DO
                run := run + i;
            END_FOR;
            run := run + pick(x := 4) * 100;
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "run", ());
    assert_eq!(result, 212, "B is 4, and the step 2 sums 0 + 2 + 4 + 6");
}
