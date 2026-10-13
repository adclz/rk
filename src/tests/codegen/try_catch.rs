// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! `__TRY … __CATCH … __FINALLY … __END_TRY`, run: what the body raises
//! lands in the handler with its message, the checks the compiler inserts
//! included, the cleanup runs on every way out, and the stack of recursive
//! calls is put back where the `__TRY` found it.
//!
//! What a test computes through [`call`] it computes again at each level of
//! the optimizer, where a wasm-opt is installed.

use crate::tests::codegen::{
    TestPlc, compile_to_mir_and_wasm, compile_to_wasm, compile_with_libraries, execute_wasm,
    run_tests, with_db,
};
use crate::tests::utils::{skip_without_wasm_opt, wasm_opt_installed};
use debug_format::{DebugInfo, VarValue};
use rstest::*;

/// What `name` of `wasm` returns for `params`: as built, and as each level
/// of the optimizer leaves the module, which has to return the same. The
/// jumps out of a `__TRY` with a `__FINALLY` are the control flow the
/// optimizer has the most to rewrite.
fn call<P>(wasm: &[u8], name: &str, params: P) -> i32
where
    P: wasmtime::WasmParams + Copy,
{
    let built = execute_wasm::<P, i32>(wasm, name, params);
    if !wasm_opt_installed() {
        skip_without_wasm_opt("the optimized module");
        return built;
    }
    for level in ["1", "3", "s", "z"] {
        let optimized = rk::compiler::optimize_wasm_release(wasm.to_vec(), level, false)
            .unwrap_or_else(|e| panic!("-O{level}: {e}"));
        assert_eq!(
            execute_wasm::<P, i32>(&optimized, name, params),
            built,
            "-O{level}"
        );
    }
    built
}

/// `run : DINT` of `source`, which raises nothing the host sees.
fn result(db: &mut db::RootDatabase, source: &str) -> i32 {
    call(&compile_to_wasm(db, source), "run", ())
}

/// The fault a call of `run` ends with: the exception's message when one
/// reaches the host, and what the machine says of a trap.
fn fault(db: &mut db::RootDatabase, source: &str) -> String {
    let wasm = compile_to_wasm(db, source);
    let engine = crate::tests::codegen::test_engine();
    let module = wasmtime::Module::new(&engine, &wasm).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());
    let (instance, memory) = super::instantiate_returning_memory(&mut store, &module);
    let f = instance
        .get_typed_func::<(), i32>(&mut store, "run")
        .unwrap();
    let err = f
        .call(&mut store, ())
        .expect_err("the call does not return");
    super::fault_message(&mut store, memory, err)
}

/// The stack the frames of recursive calls are pushed on, in bytes, and the
/// smallest and the largest frame a function of `wasm` pushes. The tests of
/// the stack size themselves from these: written against a stack of one
/// size, they would pass for nothing on a larger one.
fn stack_and_frames(wasm: &[u8]) -> (i32, i32, i32) {
    let info = DebugInfo::from_wasm(wasm);
    let (base, end) = info.stack().expect("a recursive function has a stack");
    let frames: Vec<u32> = info
        .frame_tables()
        .filter_map(|(_, table)| table.frame.as_ref().map(|frame| frame.size))
        .collect();
    let smallest = *frames.iter().min().expect("a recursive function");
    let largest = *frames.iter().max().expect("a recursive function");
    ((end - base) as i32, smallest as i32, largest as i32)
}

/// `Depth(n)` raises `bottom` under `n` calls of itself, and `Sum(n)` is
/// `0 + 1 + … + n`, by `n` calls of itself. Each call pushes a frame of the
/// same two kilobytes.
const DEPTH_AND_SUM: &str = r#"
        FUNCTION Depth : DINT
        VAR_INPUT n : DINT; END_VAR
        VAR trail : ARRAY[0..511] OF DINT; END_VAR
            trail[0] := n;
            IF n = 0 THEN
                __RAISE('bottom');
            END_IF;
            Depth := Depth(n - 1) + trail[0];
        END_FUNCTION

        FUNCTION Sum : DINT
        VAR_INPUT n : DINT; END_VAR
        VAR trail : ARRAY[0..511] OF DINT; END_VAR
            trail[0] := n;
            IF n > 0 THEN
                Sum := Sum(n - 1);
            END_IF;
            Sum := Sum + trail[0];
        END_FUNCTION
"#;

/// What the body raises lands in the `__CATCH` with its message, the
/// statements after the raise are not run, and the code after the `__TRY`
/// is.
#[rstest]
fn a_raise_in_the_body_lands_in_the_catch(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION run : DINT
        VAR e : STRING; END_VAR
            run := 1;
            __TRY
                run := 2;
                __RAISE('boom');
                run := 3;
            __CATCH(e)
                IF e = 'boom' THEN
                    run := run * 10;
                END_IF;
            __END_TRY;
            run := run + 1;
        END_FUNCTION
    "#;
    assert_eq!(result(&mut with_db, source), 21);
}

/// A body that raises nothing runs through, and the `__CATCH` does not.
#[rstest]
fn a_body_that_does_not_raise_skips_the_catch(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION run : DINT
        VAR e : STRING := 'kept'; END_VAR
            __TRY
                run := 1;
            __CATCH(e)
                run := 2;
            __END_TRY;
            IF e = 'kept' THEN
                run := run + 10;
            END_IF;
        END_FUNCTION
    "#;
    assert_eq!(result(&mut with_db, source), 11);
}

/// The message is cut to the capacity of the STRING the `__CATCH` names,
/// as an assignment would cut it. A bare `__CATCH` drops it.
#[rstest]
fn the_message_is_cut_to_the_variable_s_capacity(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION run : DINT
        VAR short : STRING[4]; END_VAR
            __TRY
                __RAISE('boomerang');
            __CATCH(short)
                IF short = 'boom' THEN
                    run := 1;
                END_IF;
            __END_TRY;
            __TRY
                __RAISE('dropped');
            __CATCH
                run := run + 10;
            __END_TRY;
        END_FUNCTION
    "#;
    assert_eq!(result(&mut with_db, source), 11);
}

/// Two `__TRY` in the body of another: the inner `__CATCH` takes what its
/// body raises and the outer does not see it, and a `__RAISE` of the
/// message in a `__CATCH` raises it again, to the next `__CATCH` out. A
/// `__TRY` inside a `__CATCH` is `Guarded`'s, below.
#[rstest]
fn nested_tries_and_a_raise_from_the_catch(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION run : DINT
        VAR inner : STRING; outer : STRING; END_VAR
            __TRY
                __TRY
                    __RAISE('first');
                __CATCH(inner)
                    run := 1;
                __END_TRY;
                __TRY
                    __RAISE('second');
                __CATCH(inner)
                    __RAISE(inner);
                __END_TRY;
                run := run + 100;
            __CATCH(outer)
                IF outer = 'second' THEN
                    run := run + 10;
                END_IF;
            __END_TRY;
        END_FUNCTION
    "#;
    assert_eq!(result(&mut with_db, source), 11);
}

/// What no `__CATCH` takes reaches the host as before: a raise outside any
/// `__TRY`, and one a `__CATCH` raises again.
#[rstest]
#[case::outside_a_try("__RAISE('outside');", "outside")]
#[case::raised_again(
    "__TRY
        __RAISE('inner');
    __CATCH(e)
        __RAISE(e);
    __END_TRY;",
    "inner"
)]
fn what_is_not_caught_reaches_the_host(
    mut with_db: db::RootDatabase,
    #[case] statement: &str,
    #[case] message: &str,
) {
    let source = format!(
        r#"
        FUNCTION run : DINT
        VAR e : STRING; END_VAR
            {statement}
        END_FUNCTION
    "#
    );
    assert_eq!(fault(&mut with_db, &source), message);
}

/// An integer divided by zero is not an exception. The machine stops the
/// call where it is: no `__CATCH` takes it, and no `__FINALLY` runs.
#[rstest]
#[case::division("run := 10 / zero;")]
#[case::modulo("run := 10 MOD zero;")]
fn a_division_by_zero_is_not_caught(mut with_db: db::RootDatabase, #[case] statement: &str) {
    let source = format!(
        r#"
        FUNCTION run : DINT
        VAR
            zero : DINT;
            e : STRING;
        END_VAR
            __TRY
                {statement}
            __CATCH(e)
                run := 1;
            __FINALLY
                run := run + 10;
            __END_TRY;
        END_FUNCTION
    "#
    );
    let fault = fault(&mut with_db, &source);
    assert!(fault.contains("integer divide by zero"), "{fault}");
}

/// The checks the compiler inserts raise what a `__CATCH` takes: a
/// subscript out of bounds, a value out of a subrange, a null reference.
#[rstest]
#[case::subscript("a[i] := 1;", "array index out of bounds")]
#[case::subrange("s := i;", "value out of subrange bounds")]
#[case::null_reference("run := Deref(p);", "dereference of a null reference")]
fn a_check_the_compiler_inserts_is_caught(
    mut with_db: db::RootDatabase,
    #[case] statement: &str,
    #[case] message: &str,
) {
    let source = format!(
        r#"
        FUNCTION Deref : DINT
        VAR_INPUT p : REF_TO DINT; END_VAR
            Deref := p^;
        END_FUNCTION

        FUNCTION run : DINT
        VAR
            a : ARRAY[0..3] OF DINT;
            s : INT (0..10);
            p : REF_TO DINT;
            i : INT := 99;
            e : STRING;
        END_VAR
            __TRY
                {statement}
                run := 1;
            __CATCH(e)
                IF e = '{message}' THEN
                    run := 2;
                END_IF;
            __END_TRY;
        END_FUNCTION
    "#
    );
    assert_eq!(result(&mut with_db, &source), 2);
}

/// A failed assertion of the standard library is a raise like any other.
#[rstest]
fn a_failed_assertion_is_caught(mut with_db: db::RootDatabase) {
    let source = r#"
        USING Std.Unit;

        FUNCTION run : DINT
        VAR e : STRING; END_VAR
            __TRY
                ASSERT(FALSE, 'the sky is green');
                run := 1;
            __CATCH(e)
                IF e = 'assertion failed: the sky is green' THEN
                    run := 2;
                END_IF;
            __END_TRY;
        END_FUNCTION
    "#;
    // `Unit.st` builds its message with `CONCAT`, from `Strings.st`.
    let wasm = compile_with_libraries(
        &mut with_db,
        &[
            include_str!("../../../stdlib/Strings.st"),
            include_str!("../../../stdlib/Unit.st"),
        ],
        source,
    );
    assert_eq!(call(&wasm, "run", ()), 2);

    // A library is lowered into the module with the workspace: what it
    // raises and what the workspace catches are one tag, which the module
    // defines.
    let mut defined = 0;
    for payload in wasmparser::Parser::new(0).parse_all(&wasm) {
        match payload.expect("a module") {
            wasmparser::Payload::TagSection(tags) => defined += tags.count(),
            wasmparser::Payload::ImportSection(imports) => {
                for import in imports {
                    if let wasmparser::Imports::Single(_, import) = import.expect("an import") {
                        assert!(
                            !matches!(import.ty, wasmparser::TypeRef::Tag(_)),
                            "a tag is imported: {}.{}",
                            import.module,
                            import.name
                        );
                    }
                }
            }
            _ => {}
        }
    }
    assert_eq!(defined, 1);
}

/// An `EXIT`, a `CONTINUE` and a `RETURN` leave a `__TRY` body as they
/// leave any block, from under the labels the `__TRY` adds.
#[rstest]
fn a_jump_leaves_the_body_of_a_try(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION run : DINT
        VAR i : DINT; e : STRING; END_VAR
            FOR i := 1 TO 10 DO
                __TRY
                    IF i = 2 THEN
                        CONTINUE;
                    END_IF;
                    IF i = 4 THEN
                        EXIT;
                    END_IF;
                    run := run + i;
                __CATCH(e)
                    run := -1;
                __END_TRY;
            END_FOR;
            run := run * 100;
            __TRY
                run := run + i;
                RETURN;
            __CATCH(e)
                run := -1;
            __END_TRY;
            run := -2;
        END_FUNCTION
    "#;
    // 1 + 3, then the counter 4 is left where the loop was left.
    assert_eq!(result(&mut with_db, source), 404);
}

/// An exception leaves the frames of the recursive calls it unwound on
/// the stack, and the `__CATCH` puts the stack pointer back where the
/// `__TRY` found it. The rounds are counted from the stack itself: left
/// where they are, the frames of nine calls a round take several times
/// what it holds.
#[rstest]
fn the_stack_of_recursive_calls_is_put_back(mut with_db: db::RootDatabase) {
    let source = format!(
        r#"{DEPTH_AND_SUM}
        FUNCTION run : DINT
        VAR_INPUT rounds : DINT; END_VAR
        VAR i : DINT; e : STRING; END_VAR
            FOR i := 1 TO rounds DO
                __TRY
                    run := Depth(8);
                __CATCH(e)
                    IF e = 'bottom' THEN
                        run := run + 1;
                    END_IF;
                __END_TRY;
            END_FOR;
            IF Sum(8) <> 36 THEN
                run := -1;
            END_IF;
        END_FUNCTION
    "#
    );
    let wasm = compile_to_wasm(&mut with_db, &source);
    let (stack, frame, _) = stack_and_frames(&wasm);
    let rounds = 4 * (stack / (9 * frame) + 1);
    assert!(rounds * 9 * frame > 4 * stack);
    assert_eq!(call(&wasm, "run", (rounds,)), rounds);
}

/// The stack pointer a landing puts back is the one of its own call: every
/// level of a recursion goes through the same `__TRY`, and the level that
/// catches finds its frame as it left it, with the stack above it free.
/// `Level` raises once the deeper levels have returned, and `Climb` at the
/// bottom, where the two levels under the one that takes it pass it on.
/// The handler then calls as deep as the stack allows above its own frame:
/// put back at the start of that frame, the pointer lets those calls write
/// over `trail`, and left where the exception found it, it leaves them no
/// room.
#[rstest]
#[case::raised_at_the_level_that_catches("Level")]
#[case::raised_under_it("Climb")]
fn a_level_of_a_recursion_catches_on_its_own_frame(
    mut with_db: db::RootDatabase,
    #[case] function: &str,
) {
    let source = format!(
        r#"{DEPTH_AND_SUM}
        FUNCTION Level : DINT
        VAR_INPUT n : DINT; deep : DINT; END_VAR
        VAR trail : ARRAY[0..511] OF DINT; e : STRING; END_VAR
            trail[0] := n;
            __TRY
                IF n > 0 THEN
                    Level := Level(n - 1, deep);
                END_IF;
                IF n = 2 THEN
                    __RAISE('after the inner tries');
                END_IF;
            __CATCH(e)
                Level := Sum(deep);
            __END_TRY;
            Level := Level + trail[0];
        END_FUNCTION

        FUNCTION Climb : DINT
        VAR_INPUT n : DINT; deep : DINT; END_VAR
        VAR trail : ARRAY[0..511] OF DINT; e : STRING; END_VAR
            trail[0] := n;
            __TRY
                IF n = 0 THEN
                    __RAISE('bottom');
                END_IF;
                Climb := Climb(n - 1, deep);
            __CATCH(e)
                IF n < 2 THEN
                    __RAISE(e);
                END_IF;
                Climb := Sum(deep);
            __END_TRY;
            Climb := Climb + trail[0];
        END_FUNCTION
    "#
    );
    let wasm = compile_to_wasm(&mut with_db, &source);
    // Level 2 takes it, under the frames of levels 4 and 3: `Sum(8)`, then
    // 2, 3 and 4 on the way up.
    assert_eq!(call(&wasm, function, (4, 8)), 36 + 2 + 3 + 4);

    // As deep as fits above the three frames in use, which is too deep
    // above the five there are when the exception starts from the bottom.
    let (stack, sum, level) = stack_and_frames(&wasm);
    let calls = (stack - 3 * level) / sum;
    assert!(3 * level + calls * sum <= stack && 5 * level + calls * sum > stack);
    let deep = calls - 1;
    assert_eq!(
        call(&wasm, function, (4, deep)),
        deep * (deep + 1) / 2 + 2 + 3 + 4
    );
}

/// A `__CATCH` in a PROGRAM takes what the body of a FUNCTION_BLOCK it
/// calls raises, and the scan goes on.
#[rstest]
fn a_catch_in_a_program_takes_what_a_block_raises(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Counter
        VAR_OUTPUT n : DINT; END_VAR
            n := n + 1;
            IF n = 2 THEN
                __RAISE('two');
            END_IF;
        END_FUNCTION_BLOCK

        PROGRAM Main
        VAR
            c : Counter;
            scans : DINT;
            failures : DINT;
            last : STRING;
        END_VAR
            __TRY
                c();
            __CATCH(last)
                failures := failures + 1;
            __END_TRY;
            scans := scans + 1;
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : Main;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let mut plc = TestPlc::load(&wasm).expect("load");
    plc.run(3).expect("three scans, the second raising");
    let info = DebugInfo::from_wasm(&wasm);
    assert_eq!(plc.read_var(&info, "P1.scans"), Some(VarValue::I32(3)));
    assert_eq!(plc.read_var(&info, "P1.failures"), Some(VarValue::I32(1)));
    assert_eq!(plc.read_var(&info, "P1.c.n"), Some(VarValue::I32(3)));
    assert_eq!(
        plc.read_var(&info, "P1.last"),
        Some(VarValue::String("two".to_string()))
    );
}

/// A `{test}` that catches its own failure passes. One that raises again
/// fails with what it raised last.
#[rstest]
fn a_test_that_catches_its_failure_passes(mut with_db: db::RootDatabase) {
    let source = r#"
        {test}
        FUNCTION test_caught
        VAR e : STRING; END_VAR
            __TRY
                __RAISE('handled');
            __CATCH(e)
                e := '';
            __END_TRY;
        END_FUNCTION

        {test}
        FUNCTION test_raised_again
        VAR e : STRING; END_VAR
            __TRY
                __RAISE('first');
            __CATCH(e)
                __RAISE('second');
            __END_TRY;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let records = run_tests(&wasm, None).expect("the tests run");
    let outcome: Vec<(&str, bool, Option<&str>)> = records
        .iter()
        .map(|r| (r.name.as_str(), r.passed(), r.reason.as_deref()))
        .collect();
    assert_eq!(
        outcome,
        [
            ("test_caught", true, None),
            ("test_raised_again", false, Some("second")),
        ]
    );
}

/// A `__FINALLY` runs on every way out of the body and the handler, last:
/// after a body that ran through, after a handler that took what the body
/// raised, and after a body with no `__CATCH`. The trace is read in the
/// order it was written.
#[rstest]
fn a_finally_runs_last_on_every_way_out(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION run : DINT
        VAR e : STRING; END_VAR
            __TRY
                run := run * 10 + 1;
            __CATCH(e)
                run := run * 10 + 2;
            __FINALLY
                run := run * 10 + 3;
            __END_TRY;
            __TRY
                __RAISE('x');
            __CATCH(e)
                run := run * 10 + 4;
            __FINALLY
                run := run * 10 + 5;
            __END_TRY;
            __TRY
                run := run * 10 + 6;
            __FINALLY
                run := run * 10 + 7;
            __END_TRY;
        END_FUNCTION
    "#;
    assert_eq!(result(&mut with_db, source), 134_567);
}

/// With no `__CATCH`, the cleanup runs and the exception goes on, with its
/// message. So does what the handler raises, after the cleanup.
#[rstest]
fn a_finally_runs_before_the_exception_goes_on(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Guarded
        VAR_OUTPUT cleaned : DINT; END_VAR
        VAR e : STRING; END_VAR
            __TRY
                __TRY
                    __RAISE('from the body');
                __FINALLY
                    cleaned := cleaned + 1;
                __END_TRY;
            __CATCH(e)
                __TRY
                    __RAISE(e);
                __CATCH(e)
                    __RAISE('from the handler');
                __FINALLY
                    cleaned := cleaned + 10;
                __END_TRY;
            __END_TRY;
        END_FUNCTION_BLOCK

        PROGRAM Main
        VAR
            g : Guarded;
            seen : STRING;
        END_VAR
            __TRY
                g();
            __CATCH(seen)
                seen := CONCAT('caught ', seen);
            __END_TRY;
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : Main;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let wasm = compile_with_libraries(
        &mut with_db,
        &[include_str!("../../../stdlib/Strings.st")],
        &format!("USING Std.Strings;\n{source}"),
    );
    let mut plc = TestPlc::load(&wasm).expect("load");
    plc.run(1).expect("the scan catches what the block raises");
    let info = DebugInfo::from_wasm(&wasm);
    assert_eq!(plc.read_var(&info, "P1.g.cleaned"), Some(VarValue::I32(11)));
    assert_eq!(
        plc.read_var(&info, "P1.seen"),
        Some(VarValue::String("caught from the handler".to_string()))
    );
}

/// A `RETURN`, an `EXIT` and a `CONTINUE` out of the body or the handler
/// run the cleanup first, and then jump. A loop inside the body keeps its
/// own jumps, which run no cleanup. The result a `RETURN` returns is what
/// the cleanup leaves in it.
#[rstest]
fn a_jump_out_of_a_try_with_a_finally_runs_the_cleanup_first(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION Loop : DINT
        VAR i : DINT; j : DINT; e : STRING; END_VAR
            FOR i := 1 TO 10 DO
                __TRY
                    FOR j := 1 TO 3 DO
                        IF j = 2 THEN
                            EXIT;
                        END_IF;
                        Loop := Loop + 1;
                    END_FOR;
                    IF i = 2 THEN
                        CONTINUE;
                    END_IF;
                    IF i = 4 THEN
                        __RAISE('four');
                    END_IF;
                    Loop := Loop + 100;
                __CATCH(e)
                    EXIT;
                __FINALLY
                    Loop := Loop + 1000;
                __END_TRY;
            END_FOR;
        END_FUNCTION

        FUNCTION Early : DINT
        VAR e : STRING; END_VAR
            __TRY
                Early := 1;
                RETURN;
            __CATCH(e)
                Early := -1;
            __FINALLY
                Early := Early + 10;
            __END_TRY;
            Early := -2;
        END_FUNCTION

        FUNCTION FromHandler : DINT
        VAR e : STRING; END_VAR
            __TRY
                __RAISE('x');
            __CATCH(e)
                FromHandler := 1;
                RETURN;
            __FINALLY
                FromHandler := FromHandler + 10;
            __END_TRY;
            FromHandler := -2;
        END_FUNCTION

        FUNCTION run : DINT
            run := Loop() * 1000 + Early() * 10 + FromHandler();
        END_FUNCTION
    "#;
    // Loop: i = 1 runs through (1 + 100 + 1000), i = 2 continues after the
    // inner loop (1 + 1000), i = 3 runs through, i = 4 raises, the handler
    // exits, the cleanup runs (1 + 1000): 1101 + 1001 + 1101 + 1001.
    assert_eq!(result(&mut with_db, source), 4_204_000 + 110 + 11);
}

/// Two `__FINALLY`s nested: a `RETURN` out of the inner body runs the inner
/// cleanup, then the outer, then returns. An exception out of the inner
/// `__TRY` runs its cleanup, then lands in the outer `__CATCH`.
#[rstest]
fn nested_finallys_run_inner_first(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION Returned : DINT
            __TRY
                __TRY
                    Returned := 1;
                    RETURN;
                __FINALLY
                    Returned := Returned * 10 + 2;
                __END_TRY;
                Returned := -1;
            __FINALLY
                Returned := Returned * 10 + 3;
            __END_TRY;
            Returned := -2;
        END_FUNCTION

        FUNCTION Raised : DINT
        VAR e : STRING; END_VAR
            __TRY
                __TRY
                    __RAISE('deep');
                __FINALLY
                    Raised := Raised * 10 + 1;
                __END_TRY;
                Raised := -1;
            __CATCH(e)
                IF e = 'deep' THEN
                    Raised := Raised * 10 + 2;
                END_IF;
            __FINALLY
                Raised := Raised * 10 + 3;
            __END_TRY;
        END_FUNCTION

        FUNCTION run : DINT
            run := Returned() * 1000 + Raised();
        END_FUNCTION
    "#;
    assert_eq!(result(&mut with_db, source), 123_123);
}

/// The frames an exception unwound are left on the stack until a landing
/// puts the stack pointer back: a `__FINALLY` with no `__CATCH` does it
/// too, before the exception goes on to the `__CATCH` of a caller. The
/// cleanup is where it shows, so the cleanup calls as deep as the stack
/// holds: above the nine frames `Depth` left it would overflow, and that
/// would be the exception the caller takes.
#[rstest]
fn a_finally_puts_the_stack_back_before_the_exception_goes_on(mut with_db: db::RootDatabase) {
    let source = format!(
        r#"{DEPTH_AND_SUM}
        FUNCTION Cleaned : DINT
        VAR_INPUT deep : DINT; END_VAR
        VAR_IN_OUT count : DINT; END_VAR
            __TRY
                Cleaned := Depth(8);
            __FINALLY
                Cleaned := Sum(deep);
                count := count + 1;
            __END_TRY;
        END_FUNCTION

        FUNCTION run : DINT
        VAR_INPUT deep : DINT; END_VAR
        VAR i : DINT; done : DINT; caught : DINT; e : STRING; END_VAR
            FOR i := 1 TO 3 DO
                __TRY
                    run := Cleaned(deep, done);
                __CATCH(e)
                    IF e = 'bottom' THEN
                        caught := caught + 1;
                    END_IF;
                __END_TRY;
            END_FOR;
            run := caught * 10 + done;
        END_FUNCTION
    "#
    );
    let wasm = compile_to_wasm(&mut with_db, &source);
    let (stack, frame, largest) = stack_and_frames(&wasm);
    let calls = stack / largest - 1;
    assert!(calls * largest <= stack && (9 + calls) * frame > stack);
    assert_eq!(call(&wasm, "run", (calls - 1,)), 33);
}

/// A `__RAISE` in a `__FINALLY` is what leaves the `__TRY`: it takes the
/// place of the exception the cleanup ran for, and the cleanup that raised
/// is not run again for it. Nor is it the `__CATCH` of its own `__TRY`
/// that takes it.
#[rstest]
fn a_raise_in_a_finally_leaves_in_place_of_the_first(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION Replaced : DINT
        VAR e : STRING; cleaned : DINT; END_VAR
            __TRY
                __TRY
                    __RAISE('first');
                __FINALLY
                    cleaned := cleaned + 1;
                    __RAISE('second');
                __END_TRY;
            __CATCH(e)
                IF e = 'second' THEN
                    Replaced := 10;
                END_IF;
            __END_TRY;
            Replaced := Replaced + cleaned;
        END_FUNCTION

        FUNCTION Own : DINT
        VAR e : STRING; END_VAR
            __TRY
                __TRY
                    Own := 1;
                __CATCH(e)
                    Own := -1;
                __FINALLY
                    Own := Own + 10;
                    __RAISE('from the cleanup');
                __END_TRY;
            __CATCH(e)
                IF e = 'from the cleanup' THEN
                    Own := Own + 100;
                END_IF;
            __END_TRY;
        END_FUNCTION

        FUNCTION run : DINT
            run := Replaced() * 1000 + Own();
        END_FUNCTION
    "#;
    assert_eq!(result(&mut with_db, source), 11_111);
}

/// The message goes where an assignment of a STRING would put it: a field,
/// an element picked at run time and cut to its own capacity, the output
/// of an instance, and the variable of a caller behind a VAR_IN_OUT.
#[rstest]
fn the_message_goes_where_an_assignment_would(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Log : STRUCT last : STRING[8]; END_STRUCT; END_TYPE

        FUNCTION_BLOCK Keeper
        VAR_OUTPUT last : STRING; END_VAR
        END_FUNCTION_BLOCK

        FUNCTION Fill : DINT
        VAR_IN_OUT into : STRING; END_VAR
            __TRY
                __RAISE('through');
            __CATCH(into)
                Fill := 1;
            __END_TRY;
        END_FUNCTION

        FUNCTION run : DINT
        VAR
            log : Log;
            lines : ARRAY[0..2] OF STRING[4];
            keeper : Keeper;
            passed : STRING;
            i : DINT := 2;
        END_VAR
            __TRY
                __RAISE('a field');
            __CATCH(log.last)
                i := 2;
            __END_TRY;
            __TRY
                __RAISE('element');
            __CATCH(lines[i])
                i := 2;
            __END_TRY;
            __TRY
                __RAISE('an output');
            __CATCH(keeper.last)
                i := 2;
            __END_TRY;
            Fill(passed);
            IF log.last = 'a field' THEN
                run := run + 1;
            END_IF;
            IF lines[2] = 'elem' AND lines[1] = '' THEN
                run := run + 10;
            END_IF;
            IF keeper.last = 'an output' THEN
                run := run + 100;
            END_IF;
            IF passed = 'through' THEN
                run := run + 1000;
            END_IF;
        END_FUNCTION
    "#;
    assert_eq!(result(&mut with_db, source), 1111);
}

/// An FB instance picked at run time, called inside a `__TRY`: the
/// receiver's scratch is allocated for the call there too.
#[rstest]
fn a_block_picked_at_run_time_is_called_inside_a_try(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Adder
        VAR_INPUT amount : DINT; END_VAR
        VAR_OUTPUT total : DINT; END_VAR
            total := total + amount;
        END_FUNCTION_BLOCK

        FUNCTION run : DINT
        VAR adders : ARRAY[0..1] OF Adder; i : DINT; e : STRING; END_VAR
            FOR i := 0 TO 1 DO
                __TRY
                    adders[i].amount := i + 1;
                    adders[i]();
                __CATCH(e)
                    run := -1;
                __FINALLY
                    run := run + adders[i].total;
                __END_TRY;
            END_FOR;
        END_FUNCTION
    "#;
    assert_eq!(result(&mut with_db, source), 3);
}
