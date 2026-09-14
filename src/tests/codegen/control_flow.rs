//! Control flow execution tests - IF, CASE, FOR, WHILE, REPEAT.

use crate::tests::codegen::{compile_to_wasm, with_db};
use rstest::*;

#[rstest]
fn test_if_else(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION abs_value : INT
        VAR_INPUT
            x : INT;
        END_VAR
            IF x < 0 THEN
                abs_value := -x;
            ELSE
                abs_value := x;
            END_IF;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);

    let engine = crate::tests::codegen::test_engine();
    let module = wasmtime::Module::new(&engine, &wasm_bytes).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());
    let instance = super::instantiate_with_memory(&mut store, &module);

    let abs_value = instance
        .get_typed_func::<i32, i32>(&mut store, "abs_value")
        .expect("Failed to get function");

    assert_eq!(abs_value.call(&mut store, -5).unwrap(), 5);
    assert_eq!(abs_value.call(&mut store, 3).unwrap(), 3);
    assert_eq!(abs_value.call(&mut store, 0).unwrap(), 0);
}

#[rstest]
fn test_nested_if(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION classify : INT
        VAR_INPUT
            x : INT;
        END_VAR
            IF x > 0 THEN
                IF x > 10 THEN
                    classify := 2;  // Large positive
                ELSE
                    classify := 1;  // Small positive
                END_IF;
            ELSIF x < 0 THEN
                classify := -1;     // Negative
            ELSE
                classify := 0;      // Zero
            END_IF;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);

    let engine = crate::tests::codegen::test_engine();
    let module = wasmtime::Module::new(&engine, &wasm_bytes).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());
    let instance = super::instantiate_with_memory(&mut store, &module);

    let classify = instance
        .get_typed_func::<i32, i32>(&mut store, "classify")
        .expect("Failed to get function");

    assert_eq!(
        classify.call(&mut store, 15).unwrap(),
        2,
        "15 is large positive"
    );
    assert_eq!(
        classify.call(&mut store, 5).unwrap(),
        1,
        "5 is small positive"
    );
    assert_eq!(classify.call(&mut store, -3).unwrap(), -1, "-3 is negative");
    assert_eq!(classify.call(&mut store, 0).unwrap(), 0, "0 is zero");
}

#[rstest]
fn test_case_statement(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION day_type : INT
        VAR_INPUT
            day : INT;
        END_VAR
            CASE day OF
                1, 2, 3, 4, 5:
                    day_type := 1;  // Weekday
                6, 7:
                    day_type := 0;  // Weekend
            ELSE
                day_type := -1;     // Invalid
            END_CASE;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);

    let engine = crate::tests::codegen::test_engine();
    let module = wasmtime::Module::new(&engine, &wasm_bytes).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());
    let instance = super::instantiate_with_memory(&mut store, &module);

    let day_type = instance
        .get_typed_func::<i32, i32>(&mut store, "day_type")
        .expect("Failed to get function");

    assert_eq!(
        day_type.call(&mut store, 1).unwrap(),
        1,
        "Monday is weekday"
    );
    assert_eq!(
        day_type.call(&mut store, 5).unwrap(),
        1,
        "Friday is weekday"
    );
    assert_eq!(
        day_type.call(&mut store, 6).unwrap(),
        0,
        "Saturday is weekend"
    );
    assert_eq!(
        day_type.call(&mut store, 7).unwrap(),
        0,
        "Sunday is weekend"
    );
    assert_eq!(day_type.call(&mut store, 0).unwrap(), -1, "0 is invalid");
    assert_eq!(day_type.call(&mut store, 8).unwrap(), -1, "8 is invalid");
}

#[rstest]
fn test_for_loop(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION sum_to_n : INT
        VAR_INPUT
            n : INT;
        END_VAR
        VAR
            sum : INT;
            i : INT;
        END_VAR
            sum := 0;
            FOR i := 1 TO n DO
                sum := sum + i;
            END_FOR;
            sum_to_n := sum;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);

    let engine = crate::tests::codegen::test_engine();
    let module = wasmtime::Module::new(&engine, &wasm_bytes).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());
    let instance = super::instantiate_with_memory(&mut store, &module);

    let sum_to_n = instance
        .get_typed_func::<i32, i32>(&mut store, "sum_to_n")
        .expect("Failed to get function");

    assert_eq!(sum_to_n.call(&mut store, 5).unwrap(), 15, "1+2+3+4+5 = 15");
    assert_eq!(
        sum_to_n.call(&mut store, 10).unwrap(),
        55,
        "Sum to 10 is 55"
    );
}

#[rstest]
fn test_while_loop(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION power_of_two : INT
        VAR_INPUT
            n : INT;
        END_VAR
        VAR
            result : INT;
            i : INT;
        END_VAR
            result := 1;
            i := 0;
            WHILE i < n DO
                result := result * 2;
                i := i + 1;
            END_WHILE;
            power_of_two := result;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);

    let engine = crate::tests::codegen::test_engine();
    let module = wasmtime::Module::new(&engine, &wasm_bytes).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());
    let instance = super::instantiate_with_memory(&mut store, &module);

    let power_of_two = instance
        .get_typed_func::<i32, i32>(&mut store, "power_of_two")
        .expect("Failed to get function");

    assert_eq!(power_of_two.call(&mut store, 0).unwrap(), 1, "2^0 = 1");
    assert_eq!(power_of_two.call(&mut store, 3).unwrap(), 8, "2^3 = 8");
    assert_eq!(power_of_two.call(&mut store, 5).unwrap(), 32, "2^5 = 32");
}

#[rstest]
fn test_repeat_loop(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION find_divisor : INT
        VAR_INPUT
            n : INT;
        END_VAR
        VAR
            i : INT;
        END_VAR
            i := 2;
            REPEAT
                IF n MOD i = 0 THEN
                    find_divisor := i;
                    RETURN;
                END_IF;
                i := i + 1;
            UNTIL i > n
            END_REPEAT;
            find_divisor := n;  // Prime or 1
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);

    let engine = crate::tests::codegen::test_engine();
    let module = wasmtime::Module::new(&engine, &wasm_bytes).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());
    let instance = super::instantiate_with_memory(&mut store, &module);

    let find_divisor = instance
        .get_typed_func::<i32, i32>(&mut store, "find_divisor")
        .expect("Failed to get function");

    assert_eq!(
        find_divisor.call(&mut store, 15).unwrap(),
        3,
        "15 divisible by 3"
    );
    assert_eq!(find_divisor.call(&mut store, 7).unwrap(), 7, "7 is prime");
    assert_eq!(
        find_divisor.call(&mut store, 12).unwrap(),
        2,
        "12 divisible by 2"
    );
}

/// Descending FOR: a constant negative `BY` flips the exit comparison.
/// Before the fix the loop compared ascending (`ctrl > end`) regardless of
/// direction, so `BY -1` exited before its first iteration.
#[rstest]
fn test_for_descending(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION count_down : INT
        VAR n : INT; i : INT; END_VAR
            FOR i := 10 TO 1 BY -1 DO
                n := n + 1;
            END_FOR;
            count_down := n;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let r: i32 = super::execute_wasm(&wasm, "count_down", ());
    assert_eq!(r, 10, "descending FOR runs all 10 iterations");
}

/// Descending FOR with a step that overshoots the bound: exits on the first
/// counter value strictly below `end`.
#[rstest]
fn test_for_descending_by_two(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION count_down2 : INT
        VAR n : INT; i : INT; END_VAR
            FOR i := 10 TO 1 BY -2 DO
                n := n + 1;
            END_FOR;
            count_down2 := n;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let r: i32 = super::execute_wasm(&wasm, "count_down2", ());
    assert_eq!(r, 5, "10,8,6,4,2 then 0 < 1 exits");
}

/// Ascending FOR whose range is empty must not iterate (guards that the
/// descending fix didn't disturb the ascending exit test).
#[rstest]
fn test_for_empty_ascending_range(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION no_iters : INT
        VAR n : INT; i : INT; END_VAR
            FOR i := 5 TO 1 DO
                n := n + 1;
            END_FOR;
            no_iters := n;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let r: i32 = super::execute_wasm(&wasm, "no_iters", ());
    assert_eq!(r, 0, "start > end with positive step never enters the body");
}

/// LINT (i64-lane) control variable. Before the fix the bound check and
/// increment hardcoded i32 ops, producing a module that failed wasm
/// validation outright.
#[rstest]
fn test_for_lint_control_var(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION lint_sum : LINT
        VAR n : LINT; i : LINT; END_VAR
            FOR i := LINT#1 TO LINT#3 DO
                n := n + i;
            END_FOR;
            lint_sum := n;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let r: i64 = super::execute_wasm(&wasm, "lint_sum", ());
    assert_eq!(r, 6, "i64 FOR loop sums 1+2+3");
}

/// Descending i64 FOR with bounds only representable beyond i32.
#[rstest]
fn test_for_lint_descending_beyond_i32(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION lint_down : LINT
        VAR n : LINT; i : LINT; END_VAR
            FOR i := LINT#5000000002 TO LINT#5000000000 BY LINT#-1 DO
                n := n + 1;
            END_FOR;
            lint_down := n;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let r: i64 = super::execute_wasm(&wasm, "lint_down", ());
    assert_eq!(r, 3, "descending i64 FOR with bounds beyond i32 range");
}

/// Sub-width control variable: the counter increment normalizes like any
/// other arithmetic, so the counter never escapes its type's domain. (An
/// upper bound at the type MAX wraps before the exit check and never
/// terminates, so bounds here stay below it.)
#[rstest]
fn test_for_subwidth_counter_stays_in_domain(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION usint_counter : USINT
        VAR i : USINT; n : USINT; END_VAR
            FOR i := 250 TO 254 DO
                n := n + 1;
            END_FOR;
            IF n <> 5 THEN
                usint_counter := 0;
            ELSE
                usint_counter := i;
            END_IF;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let r: i32 = super::execute_wasm(&wasm, "usint_counter", ());
    assert_eq!(r, 255, "5 iterations and the counter exits in-domain at 255");
}

/// A FOR counter that is a PROGRAM member — ordinary code. The counter
/// lives in the instance struct, not a wasm local, so the loop must read and
/// write it through its place. MIR used to reject the shape outright ("FOR
/// control variable must be a simple local") after `rk check` had passed it,
/// and the codegen's old arm would have silently DISCARDED the entire loop.
#[rstest]
fn for_counter_living_in_a_program_member(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR RETAIN total : DINT; END_VAR
        VAR i : INT; END_VAR
            total := 0;
            FOR i := 1 TO 5 DO
                total := total + i;
            END_FOR;
            (* the counter is observable state: after the loop it sits at 6 *)
            total := total * 100 + i;
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = crate::tests::codegen::compile_to_mir_and_wasm(&mut with_db, source);
    let mut plc = runtime::Plc::load(&wasm, runtime::Config::default()).expect("load");
    plc.run(1).expect("scan");
    let total = i32::from_le_bytes(plc.read_retain()[..4].try_into().unwrap());
    assert_eq!(total, 1506, "sum 15, counter left at 6 after the loop");
}

/// The body reads the SAME member the loop counts in — a synthetic-local
/// counter that only wrote back at the end would show the body stale values.
#[rstest]
fn the_body_observes_the_member_counter_each_iteration(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Acc
        VAR i : INT; END_VAR
        VAR_OUTPUT digits : DINT; END_VAR
            digits := 0;
            FOR i := 1 TO 3 DO
                digits := digits * 10 + i;
            END_FOR;
        END_FUNCTION_BLOCK

        FUNCTION run : DINT
        VAR a : Acc; END_VAR
            a();
            run := a.digits;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "run", ());
    assert_eq!(result, 123, "each iteration read the live counter: 1, 2, 3");
}

/// IEC 61131-3: a FOR statement's end and step expressions are evaluated
/// ONCE, at loop entry — the body mutating a variable used in the bound must
/// not change the iteration count. The bound used to be re-emitted inside
/// the loop's compare, so this ran TEN times; it is now snapshotted into a
/// per-statement scratch local at entry.
#[rstest]
fn for_bound_is_fixed_at_entry(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION run : INT
        VAR i : INT; n : INT; count : INT; END_VAR
            n := 3;
            FOR i := 1 TO n DO
                n := 10;
                count := count + 1;
            END_FOR;
            run := count;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let r: i32 = super::execute_wasm(&wasm, "run", ());
    assert_eq!(r, 3, "the bound is fixed at entry (IEC); re-evaluation would give 10");
}

/// Same question for a bound with a SIDE EFFECT: a function call in `TO`
/// must run once, not once per iteration.
#[rstest]
fn for_bound_call_runs_once(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Counter
        VAR calls : INT; END_VAR
            METHOD PUBLIC bound : INT
                calls := calls + 1;
                bound := 4;
            END_METHOD
        END_FUNCTION_BLOCK

        FUNCTION run : INT
        VAR i : INT; c : Counter; total : INT; END_VAR
            FOR i := 1 TO c.bound() DO
                total := total + 1;
            END_FOR;
            run := c.calls * 100 + total;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let r: i32 = super::execute_wasm(&wasm, "run", ());
    assert_eq!(r, 104, "bound() called once at entry, 4 iterations");
}

/// A CONSTANT variable as the step: the folded value drives the DIRECTION.
/// Lowered as a Load its sign was invisible, so `BY K` with `K = -1` read as
/// ascending and ran zero times. (A plain variable step is E1204 now.)
#[rstest]
fn for_descending_by_constant_variable(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION run : INT
        VAR CONSTANT K : INT := -1; END_VAR
        VAR i : INT; count : INT; END_VAR
            FOR i := 5 TO 1 BY K DO
                count := count + 1;
                IF count > 300 THEN EXIT; END_IF;
            END_FOR;
            run := count;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let r: i32 = super::execute_wasm(&wasm, "run", ());
    assert_eq!(r, 5, "K's folded sign makes the loop descend");
}

/// The snapshot locals are lane-typed: an LINT counter's non-constant bound
/// lives in an i64 scratch, not a truncating i32 one.
#[rstest]
fn for_lint_bound_snapshot_keeps_its_width(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION run : LINT
        VAR i : LINT; n : LINT; count : LINT; END_VAR
            n := 3000000000;
            FOR i := 2999999998 TO n DO
                n := 0;
                count := count + 1;
            END_FOR;
            run := count;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let r: i64 = super::execute_wasm(&wasm, "run", ());
    assert_eq!(r, 3, "an i64 bound beyond i32 range survives the snapshot");
}

/// Nested loops each own their snapshot: the inner loop's bound must not
/// clobber the outer's — the reason the scratch is per-statement rather
/// than one shared slot.
///
/// The expected count is 202, not 6: "once at entry" means once per
/// EXECUTION of the FOR statement, and the inner statement executes three
/// times. Its first entry snapshots `m = 2`; the body then sets `m := 100`,
/// so entries two and three snapshot 100 — giving 2 + 100 + 100 inner
/// iterations. The outer loop meanwhile runs EXACTLY three times, which is
/// the clobber proof: with one shared scratch slot, the inner loop's
/// snapshot would overwrite the outer bound and the outer loop would run
/// 100 times.
#[rstest]
fn nested_for_bounds_do_not_clobber_each_other(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION run : INT
        VAR i : INT; j : INT; n : INT; m : INT; count : INT; END_VAR
            n := 3;
            m := 2;
            FOR i := 1 TO n DO
                FOR j := 1 TO m DO
                    n := 100;
                    m := 100;
                    count := count + 1;
                END_FOR;
            END_FOR;
            run := count;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let r: i32 = super::execute_wasm(&wasm, "run", ());
    assert_eq!(r, 202, "outer fixed at 3; inner re-snapshots at each entry: 2 + 100 + 100");
}

/// EXIT under an IF must leave the LOOP, not the IF: the branch depth has
/// to account for every label the IF structure opened.
#[rstest]
fn exit_under_if_leaves_the_while_loop(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION run : INT
        VAR i : INT; count : INT; END_VAR
            WHILE i < 10 DO
                i := i + 1;
                IF i = 3 THEN EXIT; END_IF;
                count := count + 1;
            END_WHILE;
            run := i * 100 + count;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let r: i32 = super::execute_wasm(&wasm, "run", ());
    assert_eq!(r, 302, "exits at i = 3 with two iterations counted");
}

#[rstest]
fn continue_under_if_skips_to_the_next_while_iteration(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION run : INT
        VAR i : INT; count : INT; END_VAR
            WHILE i < 5 DO
                i := i + 1;
                IF i = 2 THEN CONTINUE; END_IF;
                count := count + 1;
            END_WHILE;
            run := count;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let r: i32 = super::execute_wasm(&wasm, "run", ());
    assert_eq!(r, 4, "iteration i = 2 skips the count");
}

#[rstest]
fn continue_under_if_still_increments_the_for_counter(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION run : INT
        VAR i : INT; count : INT; END_VAR
            FOR i := 1 TO 5 DO
                IF i = 3 THEN CONTINUE; END_IF;
                count := count + 1;
            END_FOR;
            run := count;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let r: i32 = super::execute_wasm(&wasm, "run", ());
    assert_eq!(r, 4, "CONTINUE still increments the counter and re-checks the bound");
}

/// CONTINUE in a REPEAT must jump to the UNTIL check, not restart the body,
/// and not simply fall out of the IF: the statement AFTER the IF is the
/// distinguisher.
#[rstest]
fn continue_in_repeat_reaches_the_until_check(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION run : INT
        VAR count : INT; sum : INT; END_VAR
            REPEAT
                count := count + 1;
                IF count = 1 THEN CONTINUE; END_IF;
                sum := sum + count;
            UNTIL count >= 3 END_REPEAT;
            run := sum;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let r: i32 = super::execute_wasm(&wasm, "run", ());
    assert_eq!(r, 5, "iteration 1 skips the sum: 2 + 3");
}

/// EXIT leaves only the INNERMOST loop.
#[rstest]
fn exit_leaves_only_the_inner_loop(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION run : INT
        VAR i : INT; j : INT; count : INT; END_VAR
            FOR i := 1 TO 3 DO
                FOR j := 1 TO 10 DO
                    IF j = 2 THEN EXIT; END_IF;
                    count := count + 1;
                END_FOR;
            END_FOR;
            run := count;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let r: i32 = super::execute_wasm(&wasm, "run", ());
    assert_eq!(r, 3, "each outer iteration counts once before the inner EXIT");
}

/// The branch depth must survive DEEP nesting: an EXIT under an ELSIF arm
/// (two `if` labels), and one inside a CASE arm's IF (case block + arm if +
/// inner if = three labels).
#[rstest]
fn exit_survives_elsif_and_case_nesting(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION run : INT
        VAR i : INT; a : INT; b : INT; END_VAR
            WHILE i < 10 DO
                i := i + 1;
                IF i = 99 THEN
                    a := -1;
                ELSIF i = 4 THEN
                    EXIT;
                END_IF;
                a := a + 1;
            END_WHILE;

            WHILE b < 10 DO
                b := b + 1;
                CASE b OF
                    3:
                        IF TRUE THEN
                            EXIT;
                        END_IF;
                END_CASE;
            END_WHILE;
            run := i * 1000 + a * 100 + b;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let r: i32 = super::execute_wasm(&wasm, "run", ());
    assert_eq!(r, 4303, "first loop exits at i = 4 with a = 3; second at b = 3");
}

/// EXIT in a REPEAT leaves it; the body still runs AT LEAST once even when
/// the condition is true from the start.
#[rstest]
fn repeat_runs_at_least_once_and_exit_leaves_it(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION run : INT
        VAR n : INT; count : INT; END_VAR
            REPEAT
                count := count + 1;
            UNTIL TRUE END_REPEAT;

            REPEAT
                n := n + 1;
                IF n = 2 THEN EXIT; END_IF;
            UNTIL n >= 100 END_REPEAT;
            run := count * 100 + n;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let r: i32 = super::execute_wasm(&wasm, "run", ());
    assert_eq!(r, 102, "one mandatory iteration; EXIT at n = 2");
}

/// CONTINUE inside a nested loop belongs to the INNER loop: the outer
/// iteration count must not change.
#[rstest]
fn continue_targets_the_inner_loop(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION run : INT
        VAR i : INT; j : INT; count : INT; END_VAR
            FOR i := 1 TO 3 DO
                FOR j := 1 TO 4 DO
                    IF j = 2 THEN CONTINUE; END_IF;
                    count := count + 1;
                END_FOR;
            END_FOR;
            run := i * 100 + count;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let r: i32 = super::execute_wasm(&wasm, "run", ());
    assert_eq!(r, 409, "3 outer x 3 counted inner; the counter ends past the bound at 4");
}

/// CASE labels of every kind actually SELECT the right branch, and each
/// label's value is the one HIR evaluated.
///
/// IEC's `Case_List_Elem : Subrange | Constant_Expr`, and a constant
/// expression is anything that evaluates at compile time — so a named
/// CONSTANT and arithmetic over constants are labels too. Each of these used
/// to pass `rk check` and then abort `rk compile`, because MIR decided
/// constness by lowering the label and seeing whether a literal fell out.
#[rstest]
fn case_integer_labels_of_every_constant_form(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION pick : DINT
        VAR_INPUT x : DINT; END_VAR
        VAR_EXTERNAL CONSTANT K : DINT; END_VAR
            CASE x OF
                -1:      pick := 100;
                2..4:    pick := 200;
                K:       pick := 300;
                K + 2:   pick := 400;
                DINT#20: pick := 500;
            ELSE
                pick := 0;
            END_CASE;
        END_FUNCTION

        FUNCTION run : DINT
            (* K = 7, so K+2 = 9 *)
            run := pick(x := -1) + pick(x := 3) + pick(x := 7)
                 + pick(x := 9) + pick(x := 20) + pick(x := 99);
        END_FUNCTION

        CONFIGURATION Cfg
        VAR_GLOBAL CONSTANT K : DINT := 7; END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P WITH T : Dummy;
            END_RESOURCE
        END_CONFIGURATION

        PROGRAM Dummy
        END_PROGRAM
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "run", ());
    assert_eq!(
        result, 1500,
        "literal, subrange, CONSTANT, constant arithmetic and typed literal each match"
    );
}

/// An enum label matches on the variant's declared value, not its position.
#[rstest]
fn case_enum_labels_select_by_declared_value(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Mode : (Stop := 5, Run, Halt := 9); END_TYPE

        FUNCTION pick : DINT
        VAR_INPUT m : Mode; END_VAR
            CASE m OF
                Mode#Stop: pick := 1;
                Mode#Run:  pick := 20;
                Mode#Halt: pick := 300;
            ELSE
                pick := 0;
            END_CASE;
        END_FUNCTION

        FUNCTION run : DINT
            run := pick(m := Mode#Stop) + pick(m := Mode#Run) + pick(m := Mode#Halt);
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "run", ());
    assert_eq!(result, 321, "Stop=5, Run=6, Halt=9 each reach their own arm");
}

/// A STRING label compares as a STRING — the same byte comparison `=` uses.
/// There is no scalar to compare against, so the arm carries its own test;
/// before that existed the label had no MIR representation and aborted
/// lowering on source that checked clean.
#[rstest]
fn case_string_labels_compare_by_content(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION classify : DINT
        VAR_INPUT s : STRING; END_VAR
            CASE s OF
                'start': classify := 1;
                'stop':  classify := 20;
                'halt':  classify := 300;
            ELSE
                classify := 4000;
            END_CASE;
        END_FUNCTION

        FUNCTION run : DINT
            run := classify(s := 'start') + classify(s := 'stop')
                 + classify(s := 'halt') + classify(s := 'other')
                 + classify(s := '');
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "run", ());
    assert_eq!(
        result, 8321,
        "each string matches its own arm; a non-match and the empty string fall through"
    );
}

/// Several labels on one arm, mixing forms, and a subrange that must not
/// swallow neighbouring values.
#[rstest]
fn case_multiple_labels_per_arm(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION pick : DINT
        VAR_INPUT x : DINT; END_VAR
            CASE x OF
                1, 3, 10..12: pick := 7;
                2:            pick := 9;
            ELSE
                pick := 0;
            END_CASE;
        END_FUNCTION

        FUNCTION run : DINT
            run := pick(x := 1) * 1000000 + pick(x := 2) * 100000
                 + pick(x := 3) * 10000 + pick(x := 10) * 1000
                 + pick(x := 12) * 100 + pick(x := 13) * 10 + pick(x := 0);
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "run", ());
    assert_eq!(result, 7977700, "1,3 and 10..12 share an arm; 2 is its own; 13 and 0 fall through");
}

/// The control variable's value AFTER normal completion is implementation-
/// dependent in IEC; ours is the first value past the bound - the one that
/// failed the loop test (4 ascending, 0 descending BY -1).
#[rstest]
fn for_control_var_after_completion(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test : INT
        VAR i : INT; j : INT; END_VAR
            FOR i := 1 TO 3 DO
            END_FOR;
            FOR j := 10 TO 1 BY -1 DO
            END_FOR;
            test := i * 10 + j;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let r: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(r, 40, "i = 4 (past 3), j = 0 (past 1)");
}

/// A bound at the type's maximum terminates: the increment past 127 would
/// wrap a SINT, so the loop exits with the counter AT the bound instead of
/// past it: 8 iterations, then i = 127 adds 100. (The EXIT belt turns a
/// regression into a wrong count rather than a hung suite.)
#[rstest]
fn for_to_type_max_terminates(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test : INT
        VAR i : SINT; c : INT; END_VAR
            FOR i := 120 TO 127 DO
                c := c + 1;
                IF c > 300 THEN EXIT; END_IF;
            END_FOR;
            IF i = 127 THEN
                c := c + 100;
            END_IF;
            test := c;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let r: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(r, 108, "8 iterations, counter left AT the bound");
}

/// The full range of an unsigned type: 0 TO 255 over USINT runs 256 times.
#[rstest]
fn for_full_unsigned_range_terminates(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test : INT
        VAR i : USINT; c : INT; END_VAR
            FOR i := 0 TO 255 DO
                c := c + 1;
                IF c > 300 THEN EXIT; END_IF;
            END_FOR;
            test := c;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let r: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(r, 256, "every USINT value visited once");
}

/// A step that never lands ON the bound: 120, 124, then 124 + 4 would wrap.
/// The headroom check catches the overshoot an equality check cannot. The
/// counter's exit value is the last value VISITED (124) - "left at the
/// bound" in the sibling tests is the special case where the bound is hit.
#[rstest]
fn for_step_overshooting_type_max_terminates(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test : INT
        VAR i : SINT; c : INT; END_VAR
            FOR i := 120 TO 126 BY 4 DO
                c := c + 1;
                IF c > 300 THEN EXIT; END_IF;
            END_FOR;
            IF i = 124 THEN
                c := c + 100;
            END_IF;
            test := c;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let r: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(r, 102, "120 and 124 visited; the counter stays at 124");
}

/// Descending to the type's minimum: past -128 would wrap to 127.
#[rstest]
fn for_to_type_min_terminates(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test : INT
        VAR i : SINT; c : INT; END_VAR
            FOR i := -120 TO -128 BY -1 DO
                c := c + 1;
                IF c > 300 THEN EXIT; END_IF;
            END_FOR;
            IF i = -128 THEN
                c := c + 100;
            END_IF;
            test := c;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let r: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(r, 109, "9 iterations, counter left AT the bound");
}

/// The 64-bit lane: a bound at LINT's maximum.
#[rstest]
fn for_to_lint_max_terminates(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test : INT
        VAR i : LINT; c : INT; END_VAR
            FOR i := LINT#9223372036854775805 TO LINT#9223372036854775807 DO
                c := c + 1;
                IF c > 300 THEN EXIT; END_IF;
            END_FOR;
            test := c;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let r: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(r, 3, "the last three LINT values");
}

/// A folded step expression drives the loop: `BY 1 + 1` records 2, so
/// 1 TO 5 visits 1, 3, 5.
#[rstest]
fn for_step_folding_expression_runs(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test : INT
        VAR i : INT; c : INT; END_VAR
            FOR i := 1 TO 5 BY 1 + 1 DO
                c := c + 1;
                IF c > 300 THEN EXIT; END_IF;
            END_FOR;
            test := c;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let r: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(r, 3, "1, 3, 5");
}
