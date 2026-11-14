// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! Variadic parameters end to end: a pack is specialized per argument count
//! unrolls over the parameters that specialization expanded it into.
//!
//! The hir tests next door assert only that HIR accepts a fold — they
//! pass a `test_diagnostics` snapshot and never lower. These run the wasm, so
//! they are what pins the VALUES a fold computes. Each operator has a module of
//! its own, with a case per type it takes; what is here is true of every fold.

use crate::tests::codegen::{execute_wasm, run, with_db};
use rstest::*;

mod add;
mod and;
mod div;
mod element;
mod eq;
mod ge;
mod gt;
mod le;
mod lt;
mod max;
mod min;
mod modulo;
mod mul;
mod mux;
mod ne;
mod or;
mod power;
mod selection;
mod sub;
mod xor;

/// An integer type by its name, bit strings included: its literals and its edges.
struct Int {
    name: &'static str,
    bits: u32,
    signed: bool,
}

impl Int {
    fn of(name: &'static str) -> Self {
        let (bits, signed) = match name {
            "SINT" => (8, true),
            "INT" => (16, true),
            "DINT" => (32, true),
            "LINT" => (64, true),
            "USINT" | "BYTE" => (8, false),
            "UINT" | "WORD" => (16, false),
            "UDINT" | "DWORD" => (32, false),
            "ULINT" | "LWORD" => (64, false),
            _ => panic!("{name} is no integer type"),
        };
        Self { name, bits, signed }
    }

    fn lit(&self, value: i128) -> String {
        format!("{}#{value}", self.name)
    }

    /// Literals for a call's arguments.
    fn args(&self, values: &[i128]) -> String {
        let literals: Vec<String> = values.iter().map(|v| self.lit(*v)).collect();
        literals.join(", ")
    }

    fn min(&self) -> i128 {
        if self.signed {
            -(1 << (self.bits - 1))
        } else {
            0
        }
    }

    fn max(&self) -> i128 {
        if self.signed {
            (1 << (self.bits - 1)) - 1
        } else {
            (1 << self.bits) - 1
        }
    }

    /// Every bit set.
    fn all(&self) -> i128 {
        if self.signed { -1 } else { self.max() }
    }

    /// The top bit alone.
    fn top(&self) -> i128 {
        if self.signed {
            self.min()
        } else {
            1 << (self.bits - 1)
        }
    }

    /// `value` as the type holds it.
    fn wrap(&self, value: i128) -> i128 {
        let wrapped = value.rem_euclid(1 << self.bits);
        if wrapped > self.max() {
            wrapped - (1 << self.bits)
        } else {
            wrapped
        }
    }
}

/// Three values of `ty`, in ascending order, at the edges of the type where it
/// has edges, so that an unsigned type compared as signed fails. BOOL has two.
fn ordered(ty: &'static str) -> [String; 3] {
    let values: [&str; 3] = match ty {
        "BOOL" => ["FALSE", "TRUE", "TRUE"],
        "REAL" => ["REAL#-1.5", "REAL#0.5", "REAL#2.5"],
        "LREAL" => ["LREAL#-1.5", "LREAL#0.5", "LREAL#2.5"],
        "TIME" => ["T#-1s", "T#0s", "T#1s"],
        "LTIME" => ["LT#-1s", "LT#0s", "LT#1s"],
        "DATE" => ["D#2020-01-01", "D#2020-01-02", "D#2020-01-03"],
        "LDATE" => ["LD#2020-01-01", "LD#2020-01-02", "LD#2020-01-03"],
        "TOD" => ["TOD#01:00:00", "TOD#02:00:00", "TOD#03:00:00"],
        "LTOD" => ["LTOD#01:00:00", "LTOD#02:00:00", "LTOD#03:00:00"],
        "DT" => [
            "DT#2020-01-01-01:00:00",
            "DT#2020-01-02-01:00:00",
            "DT#2020-01-03-01:00:00",
        ],
        "LDT" => [
            "LDT#2020-01-01-01:00:00",
            "LDT#2020-01-02-01:00:00",
            "LDT#2020-01-03-01:00:00",
        ],
        "CHAR" => ["CHAR#'a'", "CHAR#'b'", "CHAR#'c'"],
        _ => {
            let t = Int::of(ty);
            let mid = if t.signed { 0 } else { 100 };
            return [t.lit(t.min()), t.lit(mid), t.lit(t.max())];
        }
    };
    values.map(str::to_string)
}

/// A call a comparison fold answers TRUE.
fn holds(arguments: String) -> (String, String) {
    (arguments, "TRUE".to_string())
}

/// A call a comparison fold answers FALSE.
fn fails(arguments: String) -> (String, String) {
    (arguments, "FALSE".to_string())
}

/// Each call of `F`, which folds a pack of `ty` with `op` into a `gives`, gives
/// the value beside it. `Nan()` stands in for a REAL NaN.
fn assert_fold(
    db: &mut db::RootDatabase,
    ty: &str,
    gives: &str,
    op: &str,
    calls: &[(String, String)],
) {
    let mut source = format!(
        "FUNCTION Nan : REAL\nVAR zero : REAL := 0.0; END_VAR\n    Nan := zero / zero;\nEND_FUNCTION\n\
         FUNCTION F : {gives}\nVAR_INPUT v : {ty}...; END_VAR\n    F := ...v{op};\nEND_FUNCTION\n\
         FUNCTION test : DINT\n"
    );
    for (i, (arguments, want)) in calls.iter().enumerate() {
        source += &format!(
            "    IF F({arguments}) <> {want} THEN test := {}; RETURN; END_IF;\n",
            i + 1
        );
    }
    source += "END_FUNCTION\n";
    let failed: i32 = run(db, &source, "test", ());
    if failed != 0 {
        let (arguments, want) = &calls[failed as usize - 1];
        panic!("...v{op} on {ty}: F({arguments}) is not {want}");
    }
}

/// The helper every operator's module rests on says so when a value is wrong.
#[rstest]
#[should_panic(expected = "...v+ on INT: F(1, 2) is not 4")]
fn assert_fold_reports_a_wrong_value(mut with_db: db::RootDatabase) {
    assert_fold(
        &mut with_db,
        "INT",
        "INT",
        "+",
        &[
            ("1, 2".to_string(), "3".to_string()),
            ("1, 2".to_string(), "4".to_string()),
        ],
    );
}

/// A pack of an alias folds at the type it names, through any number of aliases.
#[rstest]
#[case::int("TYPE T : INT; END_TYPE", "1", "3", "2")]
#[case::alias_of_an_alias("TYPE U : DINT; END_TYPE TYPE T : U; END_TYPE", "1", "3", "2")]
#[case::real("TYPE T : REAL; END_TYPE", "1.5", "3.5", "2.5")]
#[case::time("TYPE T : TIME; END_TYPE", "T#1s", "T#3s", "T#2s")]
#[case::bool_("TYPE T : BOOL; END_TYPE", "FALSE", "TRUE", "FALSE")]
#[case::char_("TYPE T : CHAR; END_TYPE", "CHAR#'a'", "CHAR#'c'", "CHAR#'b'")]
fn a_pack_of_an_alias(
    mut with_db: db::RootDatabase,
    #[case] declarations: &str,
    #[case] low: &str,
    #[case] high: &str,
    #[case] mid: &str,
) {
    let source = format!(
        "{declarations}\n\
         FUNCTION Hi : T\nVAR_INPUT v : T...; END_VAR\n    Hi := ...v MAX;\nEND_FUNCTION\n\
         FUNCTION Same : BOOL\nVAR_INPUT v : T...; END_VAR\n    Same := ...v=;\nEND_FUNCTION\n\
         FUNCTION test : DINT\n\
         IF Hi({low}, {high}, {mid}) <> {high} THEN test := 1; END_IF;\n\
         IF NOT Same({low}, {low}) THEN test := test + 2; END_IF;\n\
         IF Same({low}, {high}) THEN test := test + 4; END_IF;\n\
         END_FUNCTION\n"
    );
    let result: i32 = run(&mut with_db, &source, "test", ());
    assert_eq!(result, 0, "each wrong call sets its own bit");
}

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
    let result: i32 = run(&mut with_db, source, "test", ());
    // 6 * 100 + 30 + 7
    assert_eq!(
        result, 637,
        "arities 3, 2 and 1 each fold over their own pack"
    );
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
    assert_eq!(i16::from_le_bytes(r[0..2].try_into().unwrap()), 15);
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
    let result: i32 = run(&mut with_db, source, "test", ());
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
    let result: i32 = run(&mut with_db, source, "test", ());
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
    let result: i32 = run(&mut with_db, source, "test", ());
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
    let result: i32 = run(&mut with_db, source, "test", ());
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
    let result: i32 = run(&mut with_db, source, "test", ());
    assert_eq!(result, 1013, "(7 + 3) * 100 + 7 + 6");
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
    let result: i32 = run(&mut with_db, source, "test", ());
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
    let result: i32 = run(&mut with_db, source, "test", ());
    // (1 + 3 * 2) * 100 + 4 * 10 + 2
    assert_eq!(result, 742);
}
