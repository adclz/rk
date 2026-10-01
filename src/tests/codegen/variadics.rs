//! Variadic parameters end to end: a pack is specialized per argument count
//! unrolls over the parameters that specialization expanded it into.
//!
//! The hir tests next door assert only that HIR accepts a fold — they
//! pass a `test_diagnostics` snapshot and never lower. These run the wasm, so
//! they are what pins the VALUES a fold computes.

use crate::tests::codegen::with_db;
use rstest::*;

/// The same variadic function called at three arities: three specializations,
/// each folding over exactly its own parameters.
#[rstest]
fn fold_add_at_several_arities(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION sum_all : INT
        VAR_INPUT args : INT...; END_VAR
            sum_all := ...args+;
        END_FUNCTION

        FUNCTION test : INT
            test := sum_all(1, 2, 3) * 100 + sum_all(10, 20) + sum_all(7);
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    // 6 * 100 + 30 + 7
    assert_eq!(
        result, 637,
        "arities 3, 2 and 1 each fold over their own pack"
    );
}

/// A single argument folds to itself — there is no operator to apply.
#[rstest]
fn fold_of_one_argument_is_that_argument(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION prod : INT
        VAR_INPUT args : INT...; END_VAR
            prod := ...args*;
        END_FUNCTION

        FUNCTION test : INT
            test := prod(9);
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(result, 9);
}

#[rstest]
fn fold_multiply(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION prod : INT
        VAR_INPUT args : INT...; END_VAR
            prod := ...args*;
        END_FUNCTION

        FUNCTION test : INT
            test := prod(2, 3, 4);
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(result, 24);
}

/// A bitwise fold over WORD.
#[rstest]
fn fold_bitwise_or(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION bits : WORD
        VAR_INPUT args : WORD...; END_VAR
            bits := ...args|;
        END_FUNCTION

        FUNCTION test : INT
        VAR r : INT := 0; END_VAR
            IF bits(WORD#16#1, WORD#16#2, WORD#16#8) = WORD#16#B THEN r := 1; END_IF;
            test := r;
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(result, 1, "1 OR 2 OR 8 = 0xB");
}

/// A comparison fold is the conjunction of ADJACENT PAIRS, not a left fold:
/// `...args=` means "all equal", so it stays BOOL instead of comparing a BOOL
/// against the next element. HIR types it that way; this pins the lowering.
#[rstest]
fn fold_comparison_is_pairwise(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION all_eq : BOOL
        VAR_INPUT args : INT...; END_VAR
            all_eq := ...args=;
        END_FUNCTION

        FUNCTION test : INT
        VAR r : INT := 0; END_VAR
            IF all_eq(5, 5, 5) THEN r := r + 1; END_IF;
            IF all_eq(5, 5, 6) THEN r := r + 10; END_IF;
            test := r;
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(
        result, 1,
        "all-equal holds for (5,5,5) and fails for (5,5,6)"
    );
}

/// `...args<` over three values tests EVERY adjacent pair, so a non-monotonic
/// middle element fails. A left fold could not express this at all.
#[rstest]
fn fold_ordering_checks_every_pair(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION ascending : BOOL
        VAR_INPUT args : INT...; END_VAR
            ascending := ...args<;
        END_FUNCTION

        FUNCTION test : INT
        VAR r : INT := 0; END_VAR
            IF ascending(1, 2, 3) THEN r := r + 1; END_IF;
            IF ascending(1, 3, 2) THEN r := r + 10; END_IF;
            test := r;
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(result, 1, "(1,3,2) is not ascending — the 3 < 2 pair fails");
}

/// The pack's element type drives the fold: REAL folds in the float lane.
#[rstest]
fn fold_over_reals(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION fsum : REAL
        VAR_INPUT args : REAL...; END_VAR
            fsum := ...args+;
        END_FUNCTION

        FUNCTION test : INT
        VAR r : INT := 0; END_VAR
            IF fsum(1.5, 2.5) = 4.0 THEN r := 1; END_IF;
            test := r;
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(result, 1);
}

/// A variadic called from a PROGRAM body, which is lowered on a different path
/// from a FUNCTION body and reaches the specialization through the same
/// call-site arity lookup.
#[rstest]
fn variadic_called_from_a_program(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION sum_all : INT
        VAR_INPUT args : INT...; END_VAR
            sum_all := ...args+;
        END_FUNCTION

        PROGRAM Main
        VAR RETAIN total : INT; END_VAR
            total := sum_all(4, 5, 6);
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : Main;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    use crate::tests::codegen::TestPlc;
    let (_mir, wasm) = crate::tests::codegen::compile_to_mir_and_wasm(&mut with_db, source);
    let mut plc = TestPlc::load(&wasm).expect("load");
    plc.run(1).expect("scan");
    let r = plc.read_retain();
    assert_eq!(i32::from_le_bytes(r[0..4].try_into().unwrap()), 15);
}

/// One argument is where the two fold families DIVERGE. An arithmetic fold of
/// a single element is that element; a comparison fold has no adjacent pair to
/// test, so it is vacuously TRUE and the argument goes unread.
#[rstest]
fn comparison_fold_of_one_argument_is_vacuously_true(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION all_eq : BOOL
        VAR_INPUT args : INT...; END_VAR
            all_eq := ...args=;
        END_FUNCTION

        FUNCTION ascending : BOOL
        VAR_INPUT args : INT...; END_VAR
            ascending := ...args<;
        END_FUNCTION

        FUNCTION test : INT
        VAR r : INT := 0; END_VAR
            IF all_eq(5) THEN r := r + 1; END_IF;
            IF ascending(5) THEN r := r + 10; END_IF;
            test := r;
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(
        result, 11,
        "a single element is trivially all-equal and ordered"
    );
}

/// A pack collects every POSITIONAL argument wherever it sits in the list; a
/// named argument binds by name and does not terminate the pack. Both calls
/// below fold (1, 2, 3), so neither drops the argument that follows
/// `accumulator := acc`.
#[rstest]
fn positional_arguments_reach_the_pack_whatever_the_order(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION sum_all : INT
        VAR_INPUT args : INT...; END_VAR
        VAR_IN_OUT accumulator : INT; END_VAR
            sum_all := ...args+;
        END_FUNCTION

        FUNCTION test : INT
        VAR acc : INT; END_VAR
            test := sum_all(1, 2, accumulator := acc, 3) * 100
                  + sum_all(accumulator := acc, 1, 2, 3);
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(result, 606, "both spellings fold (1,2,3) = 6");
}

/// A variadic METHOD is specialized per argument count like a FUNCTION:
/// called at two counts, bare inside its class and on an instance, it folds
/// over each pack. One never called is emitted at none; lowered once,
/// generically, it stopped the build with an internal error even uncalled.
#[rstest]
fn a_variadic_method_is_specialized_per_argument_count(mut with_db: db::RootDatabase) {
    let source = r#"
        CLASS Acc
            METHOD PUBLIC Sum : DINT VAR_INPUT args : DINT...; END_VAR
                Sum := ...args+;
            END_METHOD
            METHOD PUBLIC Both : DINT
                Both := Sum(1) + Sum(10, 20);
            END_METHOD
        END_CLASS

        FUNCTION_BLOCK Spare
            METHOD PUBLIC Unused : DINT VAR_INPUT args : DINT...; END_VAR
                Unused := ...args*;
            END_METHOD
        END_FUNCTION_BLOCK

        FUNCTION test : DINT
        VAR a : Acc; s : Spare; END_VAR
            test := a.Sum(1, 2, 3) * 100 + a.Both();
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(result, 631, "6 * 100 + 1 + 30");
}

/// A variadic method runs as the copy of the instance that calls it: one
/// inherited, an override, `THIS.Sum` in a base's code run on a derived
/// instance (the override), and `SUPER.Sum` (the base's).
#[rstest]
fn a_variadic_method_runs_as_the_instances_copy(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Base
            METHOD PUBLIC Sum : DINT VAR_INPUT args : DINT...; END_VAR
                Sum := ...args+;
            END_METHOD
            METHOD PUBLIC Twice : DINT
                Twice := THIS.Sum(1, 2) * 2;
            END_METHOD
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK Derived EXTENDS Base
            METHOD PUBLIC OVERRIDE Sum : DINT VAR_INPUT args : DINT...; END_VAR
                Sum := ...args+ + 100;
            END_METHOD
            METHOD PUBLIC Plain : DINT
                Plain := SUPER.Sum(4, 5, 6);
            END_METHOD
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK Child EXTENDS Base
        END_FUNCTION_BLOCK

        FUNCTION test : DINT
        VAR c : Child; d : Derived; END_VAR
            test := c.Sum(1, 2) + d.Plain() * 10 + d.Sum(1) * 1000 + d.Twice() * 1000000;
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(result, 206101153, "3, 15, 101 and (3 + 100) * 2");
}

/// A call through an interface runs the copy of whichever implementer is
/// bound, at the call's argument count: each implementer has one, an
/// implementer deriving from another included.
#[rstest]
fn a_variadic_call_through_an_interface_runs_the_implementers_copy(mut with_db: db::RootDatabase) {
    let source = r#"
        INTERFACE ISum
            METHOD Sum : DINT VAR_INPUT args : DINT...; END_VAR END_METHOD
        END_INTERFACE

        FUNCTION_BLOCK Acc IMPLEMENTS ISum
            METHOD PUBLIC Sum : DINT VAR_INPUT args : DINT...; END_VAR
                Sum := ...args+;
            END_METHOD
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK Child EXTENDS Acc
        END_FUNCTION_BLOCK

        CLASS Other IMPLEMENTS ISum
            METHOD PUBLIC Sum : DINT VAR_INPUT args : DINT...; END_VAR
                Sum := ...args+ * 1000;
            END_METHOD
        END_CLASS

        FUNCTION through : DINT
        VAR_IN_OUT s : ISum; END_VAR
            through := s.Sum(1, 2);
        END_FUNCTION

        FUNCTION test : DINT
        VAR a : Acc; c : Child; o : Other; END_VAR
            test := through(a) + through(c) * 10 + through(o);
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(result, 3033, "3 + 3 * 10 + 3 * 1000");
}

/// A FUNCTION or METHOD both variadic and taking an interface gets a copy
/// per implementer and argument count (`f$@Pump$2`). The interface copy was
/// lowered without a count, an internal error at its fold.
#[rstest]
fn a_variadic_function_or_method_on_an_interface(mut with_db: db::RootDatabase) {
    let source = r#"
        INTERFACE IDev
            METHOD Id : DINT END_METHOD
        END_INTERFACE

        FUNCTION_BLOCK Pump IMPLEMENTS IDev
            METHOD PUBLIC Id : DINT
                Id := 7;
            END_METHOD
        END_FUNCTION_BLOCK

        FUNCTION f : DINT
        VAR_IN_OUT d : IDev; END_VAR
        VAR_INPUT args : DINT...; END_VAR
            f := d.Id() + ...args+;
        END_FUNCTION

        FUNCTION_BLOCK Calc
            METHOD PUBLIC M : DINT
            VAR_IN_OUT d : IDev; END_VAR
            VAR_INPUT args : DINT...; END_VAR
                M := d.Id() + ...args+;
            END_METHOD
        END_FUNCTION_BLOCK

        FUNCTION test : DINT
        VAR p : Pump; c : Calc; END_VAR
            test := f(p, 1, 2) * 100 + c.M(p, 1, 2, 3);
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(result, 1013, "(7 + 3) * 100 + 7 + 6");
}
