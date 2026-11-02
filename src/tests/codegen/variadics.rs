// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

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

/// `MIN` and `MAX` on each elementary type but STRING, with the extreme in each
/// place: three values, then two and four, at the edges of the type where it has
/// edges, so that an unsigned type compared as signed fails. Each wrong call sets
/// a bit of its own.
#[rstest]
#[case::bool_("BOOL", "FALSE", "TRUE", "TRUE")]
#[case::sint("SINT", "SINT#-128", "SINT#0", "SINT#127")]
#[case::int("INT", "INT#-32768", "INT#0", "INT#32767")]
#[case::dint("DINT", "DINT#-2147483648", "DINT#0", "DINT#2147483647")]
#[case::lint(
    "LINT",
    "LINT#-9223372036854775808",
    "LINT#0",
    "LINT#9223372036854775807"
)]
#[case::usint("USINT", "USINT#0", "USINT#100", "USINT#200")]
#[case::uint("UINT", "UINT#0", "UINT#100", "UINT#60000")]
#[case::udint("UDINT", "UDINT#0", "UDINT#100", "UDINT#4000000000")]
#[case::ulint("ULINT", "ULINT#0", "ULINT#100", "ULINT#18000000000000000000")]
#[case::byte("BYTE", "BYTE#0", "BYTE#16#7F", "BYTE#16#FF")]
#[case::word("WORD", "WORD#0", "WORD#16#7FFF", "WORD#16#FFFF")]
#[case::dword("DWORD", "DWORD#0", "DWORD#16#7FFF_FFFF", "DWORD#16#FFFF_FFFF")]
#[case::lword(
    "LWORD",
    "LWORD#0",
    "LWORD#16#7FFF_FFFF_FFFF_FFFF",
    "LWORD#16#FFFF_FFFF_FFFF_FFFF"
)]
#[case::real("REAL", "REAL#-1.5", "REAL#0.5", "REAL#2.5")]
#[case::lreal("LREAL", "LREAL#-1.5", "LREAL#0.5", "LREAL#2.5")]
#[case::time("TIME", "T#-1s", "T#0s", "T#1s")]
#[case::ltime("LTIME", "LT#-1s", "LT#0s", "LT#1s")]
#[case::date("DATE", "D#2020-01-01", "D#2020-01-02", "D#2020-01-03")]
#[case::ldate("LDATE", "LD#2020-01-01", "LD#2020-01-02", "LD#2020-01-03")]
#[case::tod("TOD", "TOD#01:00:00", "TOD#02:00:00", "TOD#03:00:00")]
#[case::ltod("LTOD", "LTOD#01:00:00", "LTOD#02:00:00", "LTOD#03:00:00")]
#[case::dt(
    "DT",
    "DT#2020-01-01-01:00:00",
    "DT#2020-01-02-01:00:00",
    "DT#2020-01-03-01:00:00"
)]
#[case::ldt(
    "LDT",
    "LDT#2020-01-01-01:00:00",
    "LDT#2020-01-02-01:00:00",
    "LDT#2020-01-03-01:00:00"
)]
#[case::char_("CHAR", "CHAR#'a'", "CHAR#'b'", "CHAR#'c'")]
fn fold_min_max_on_each_type(
    mut with_db: db::RootDatabase,
    #[case] ty: &str,
    #[case] low: &str,
    #[case] mid: &str,
    #[case] high: &str,
) {
    let calls = [
        (format!("Lo({low}, {mid}, {high})"), low),
        (format!("Lo({mid}, {low}, {high})"), low),
        (format!("Lo({mid}, {high}, {low})"), low),
        (format!("Hi({high}, {mid}, {low})"), high),
        (format!("Hi({mid}, {high}, {low})"), high),
        (format!("Hi({low}, {mid}, {high})"), high),
        (format!("Lo({high}, {low})"), low),
        (format!("Hi({low}, {high})"), high),
        (format!("Lo({high}, {mid}, {mid}, {low})"), low),
        (format!("Hi({low}, {mid}, {mid}, {high})"), high),
        (format!("Lo({mid})"), mid),
        (format!("Hi({mid})"), mid),
    ];
    let mut body = String::new();
    for (i, (call, want)) in calls.iter().enumerate() {
        body += &format!(
            "    IF {call} <> {want} THEN test := test + {}; END_IF;\n",
            1 << i
        );
    }
    let source = format!(
        "FUNCTION Lo : {ty}\nVAR_INPUT v : {ty}...; END_VAR\n    Lo := ...v MIN;\nEND_FUNCTION\n\
         FUNCTION Hi : {ty}\nVAR_INPUT v : {ty}...; END_VAR\n    Hi := ...v MAX;\nEND_FUNCTION\n\
         FUNCTION test : DINT\n{body}END_FUNCTION\n"
    );
    let result: i32 = super::run(&mut with_db, &source, "test", ());
    assert_eq!(result, 0, "{ty}: each wrong call sets its own bit");
}

/// Each step keeps the value so far when `<=` (`>=`) holds of it and the next
/// argument, and takes that argument otherwise. A NaN compares false, so it gives
/// way when it is the value so far, and takes over when it is the next argument.
#[rstest]
#[case::min_nan_first("Lo(n, 1.0, 2.0) = 1.0")]
#[case::min_nan_between("Lo(1.0, n, 2.0) = 2.0")]
#[case::min_nan_last("Lo(1.0, 2.0, n) <> Lo(1.0, 2.0, n)")]
#[case::max_nan_first("Hi(n, 1.0, 2.0) = 2.0")]
#[case::max_nan_between("Hi(1.0, n, 2.0) = 2.0")]
#[case::max_nan_last("Hi(2.0, 1.0, n) <> Hi(2.0, 1.0, n)")]
fn fold_min_max_on_a_nan(mut with_db: db::RootDatabase, #[case] holds: &str) {
    let source = format!(
        r#"
        FUNCTION Lo : REAL
        VAR_INPUT v : REAL...; END_VAR
            Lo := ...v MIN;
        END_FUNCTION

        FUNCTION Hi : REAL
        VAR_INPUT v : REAL...; END_VAR
            Hi := ...v MAX;
        END_FUNCTION

        FUNCTION test : BOOL
        VAR zero : REAL := 0.0; n : REAL; END_VAR
            n := zero / zero;
            test := {holds};
        END_FUNCTION
    "#
    );
    let result: i32 = super::run(&mut with_db, &source, "test", ());
    assert_eq!(result, 1, "{holds}");
}

/// A step reads the value so far once per argument. Written as a nested `select`
/// it would read it twice, and forty arguments would take 2^40 loads.
#[rstest]
fn fold_min_max_of_many_arguments_stays_small(mut with_db: db::RootDatabase) {
    let arguments = (0..40)
        .map(|i| (100 - i).to_string())
        .collect::<Vec<_>>()
        .join(", ");
    let source = format!(
        r#"
        FUNCTION Lo : INT
        VAR_INPUT v : INT...; END_VAR
            Lo := ...v MIN;
        END_FUNCTION

        FUNCTION Hi : INT
        VAR_INPUT v : INT...; END_VAR
            Hi := ...v MAX;
        END_FUNCTION

        FUNCTION test : INT
            test := Lo({arguments}) * 100 + Hi({arguments});
        END_FUNCTION
    "#
    );
    let wasm = crate::tests::codegen::compile_to_wasm(&mut with_db, &source);
    assert!(wasm.len() < 30_000, "{} bytes", wasm.len());
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 61 * 100 + 100);
}

/// The scratch pair a step sets its operands aside in is the one a wrapping
/// division takes: each sets it once both operands are on the stack, so one
/// nested in the other reads its own.
#[rstest]
fn fold_min_max_beside_a_division(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION Mix : DINT
        VAR_INPUT v : DINT...; END_VAR
            Mix := 1000 / ...v MIN + ...v MAX / 2;
        END_FUNCTION

        FUNCTION Wrap : DINT
        VAR_INPUT v : DINT...; END_VAR
            Wrap := ...v MIN / ...v MAX;
        END_FUNCTION

        FUNCTION test : DINT
            IF Mix(10, 20, 5) <> 210 THEN test := test + 1; END_IF;
            IF Wrap(DINT#-2147483648, -1) <> DINT#-2147483648 THEN test := test + 2; END_IF;
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(result, 0, "each wrong call sets its own bit");
}

/// Wherever an expression stands in the body: a local's initial value, an
/// argument, a condition, and a METHOD, copied per argument count like a FUNCTION.
#[rstest]
fn fold_min_max_in_each_place(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION Twice : INT
        VAR_INPUT x : INT; END_VAR
            Twice := x * 2;
        END_FUNCTION

        FUNCTION Places : INT
        VAR_INPUT v : INT...; END_VAR
        VAR low : INT := ...v MIN; END_VAR
            Places := low + Twice(...v MAX);
            IF ...v MIN > 100 THEN
                Places := -1;
            END_IF;
        END_FUNCTION

        FUNCTION_BLOCK Block
        METHOD Range : INT
        VAR_INPUT v : INT...; END_VAR
            Range := ...v MAX - ...v MIN;
        END_METHOD
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR b : Block; END_VAR
            test := Places(3, 1, 2) * 100 + b.Range(5, 9, 7) * 10 + b.Range(4, 6);
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    // (1 + 3 * 2) * 100 + 4 * 10 + 2
    assert_eq!(result, 742);
}
