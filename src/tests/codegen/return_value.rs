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

#[rstest]
fn the_result_is_passed_to_a_var_in_out(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION Inc
        VAR_IN_OUT io : INT; END_VAR
            io := io + 1;
        END_FUNCTION

        FUNCTION Twice : INT
        VAR_INPUT x : INT; END_VAR
            Twice := x;
            Inc(io := Twice);
            Inc(io := Twice);
        END_FUNCTION

        FUNCTION test : INT
            test := Twice(1);
        END_FUNCTION
    "#;
    let result: i32 = run(&mut with_db, source, "test", ());
    assert_eq!(result, 3, "both increments reach the result");
}

#[rstest]
fn a_reference_to_the_result_writes_it(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION Answer : INT
        VAR r : REF_TO INT; END_VAR
            r := REF(Answer);
            r^ := 42;
        END_FUNCTION

        FUNCTION test : INT
            test := Answer();
        END_FUNCTION
    "#;
    let result: i32 = run(&mut with_db, source, "test", ());
    assert_eq!(result, 42, "written through a reference to the result");
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

/// The same for a METHOD's result, of a FUNCTION_BLOCK and of a CLASS.
#[rstest]
fn a_method_result_is_referenced_and_passed_in_out(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION Inc
        VAR_IN_OUT io : INT; END_VAR
            io := io + 1;
        END_FUNCTION

        FUNCTION_BLOCK Fb
            METHOD PUBLIC Get : INT
            VAR r : REF_TO INT; END_VAR
                r := REF(Get);
                r^ := 40;
                Inc(io := Get);
            END_METHOD
        END_FUNCTION_BLOCK

        CLASS Cls
            METHOD PUBLIC Get : INT
            VAR r : REF_TO INT; END_VAR
                r := REF(Get);
                r^ := 1;
                Inc(io := Get);
            END_METHOD
        END_CLASS

        FUNCTION test : INT
        VAR f : Fb; c : Cls; END_VAR
            test := f.Get() + c.Get();
        END_FUNCTION
    "#;
    let result: i32 = run(&mut with_db, source, "test", ());
    assert_eq!(result, 43, "41 from the FUNCTION_BLOCK, 2 from the CLASS");
}

/// A STRING result passed to a VAR_IN_OUT, as a buffer and its capacity:
/// the callee reads it and writes it.
#[rstest]
fn a_string_result_is_passed_to_a_var_in_out(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION Extend
        VAR_IN_OUT s : STRING; END_VAR
            IF s = 'hi' THEN
                s := 'hi, and more';
            END_IF;
        END_FUNCTION

        FUNCTION Greet : STRING
            Greet := 'hi';
            Extend(s := Greet);
        END_FUNCTION

        FUNCTION test : BOOL
            test := Greet() = 'hi, and more';
        END_FUNCTION
    "#;
    let result: i32 = run(&mut with_db, source, "test", ());
    assert_eq!(result, 1, "the callee read 'hi' and wrote the result");
}

#[rstest]
fn a_struct_result_is_written_through_a_reference(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Pt : STRUCT x : INT; y : INT; END_STRUCT; END_TYPE

        FUNCTION MakePt : Pt
        VAR r : REF_TO Pt; END_VAR
            r := REF(MakePt);
            r^.x := 1;
            MakePt.y := 2;
        END_FUNCTION

        FUNCTION test : INT
        VAR p : Pt; END_VAR
            p := MakePt();
            test := p.x * 10 + p.y;
        END_FUNCTION
    "#;
    let result: i32 = run(&mut with_db, source, "test", ());
    assert_eq!(result, 12, "x through the reference, y directly");
}

#[rstest]
fn a_method_struct_result_is_referenced_and_passed_in_out(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Pt : STRUCT x : INT; y : INT; END_STRUCT; END_TYPE

        FUNCTION Shift
        VAR_IN_OUT p : Pt; END_VAR
            p.y := p.x + 1;
        END_FUNCTION

        FUNCTION_BLOCK Shape
        VAR base : INT := 10; END_VAR
            METHOD PUBLIC Corner : Pt
            VAR r : REF_TO Pt; END_VAR
                r := REF(Corner);
                r^.x := base;
                Shift(p := Corner);
            END_METHOD
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR s : Shape; p : Pt; END_VAR
            p := s.Corner();
            test := p.x * 100 + p.y;
        END_FUNCTION
    "#;
    let result: i32 = run(&mut with_db, source, "test", ());
    assert_eq!(result, 1011, "x = 10 through the reference, y = 11 in-out");
}

/// A reference taken in a declaration's initializer, which the address-taken
/// scan reads as well as the statements.
#[rstest]
fn a_reference_taken_in_an_initializer_writes_the_result(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION Answer : INT
        VAR r : REF_TO INT := REF(Answer); END_VAR
            r^ := 42;
        END_FUNCTION

        FUNCTION_BLOCK Fb
            METHOD PUBLIC Get : INT
            VAR r : REF_TO INT := REF(Get); END_VAR
                r^ := 1;
            END_METHOD
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR f : Fb; END_VAR
            test := Answer() + f.Get();
        END_FUNCTION
    "#;
    let result: i32 = run(&mut with_db, source, "test", ());
    assert_eq!(result, 43, "42 from the FUNCTION, 1 from the METHOD");
}

#[rstest]
fn the_result_is_bound_to_an_output(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION Get
        VAR_OUTPUT o : INT; END_VAR
            o := 9;
        END_FUNCTION

        FUNCTION ViaOutput : INT
            Get(o => ViaOutput);
        END_FUNCTION

        FUNCTION test : INT
            test := ViaOutput();
        END_FUNCTION
    "#;
    let result: i32 = run(&mut with_db, source, "test", ());
    assert_eq!(result, 9, "the output lands in the result");
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
