// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! `__TRY … __CATCH … __END_TRY`, run: what the body raises lands in the
//! handler with its message, the checks the compiler inserts included, and
//! the stack of recursive calls is put back where the `__TRY` found it.

use crate::tests::codegen::{
    TestPlc, compile_to_mir_and_wasm, compile_to_wasm, compile_with_libraries, execute_wasm, run,
    run_tests, with_db,
};
use debug_format::{DebugInfo, VarValue};
use rstest::*;

/// `run : DINT` of `source`, which raises nothing the host sees.
fn result(db: &mut db::RootDatabase, source: &str) -> i32 {
    run(db, source, "run", ())
}

/// The fault a call of `run` ends with: the exception's message, when one
/// reaches the host.
fn fault(db: &mut db::RootDatabase, source: &str) -> String {
    let wasm = compile_to_wasm(db, source);
    let engine = crate::tests::codegen::test_engine();
    let module = wasmtime::Module::new(&engine, &wasm).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());
    let (instance, memory) = super::instantiate_returning_memory(&mut store, &module);
    let f = instance
        .get_typed_func::<(), i32>(&mut store, "run")
        .unwrap();
    let err = f.call(&mut store, ()).expect_err("the call raises");
    super::fault_message(&mut store, memory, err)
}

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

/// A `__TRY` inside a `__CATCH` and inside a body: the inner `__CATCH`
/// takes what its body raises and the outer does not see it, and a
/// `__RAISE` of the message in a `__CATCH` raises it again, to the next
/// `__CATCH` out.
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
fn what_is_not_caught_reaches_the_host(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION run : DINT
        VAR e : STRING; END_VAR
            __TRY
                __RAISE('inner');
            __CATCH(e)
                __RAISE(e);
            __END_TRY;
        END_FUNCTION
    "#;
    assert_eq!(fault(&mut with_db, source), "inner");
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
    assert_eq!(execute_wasm::<(), i32>(&wasm, "run", ()), 2);
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
/// `__TRY` found it: caught forty times, the frames of eight calls each
/// would have taken more than the stack holds.
#[rstest]
fn the_stack_of_recursive_calls_is_put_back(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION Depth : DINT
        VAR_INPUT n : DINT; END_VAR
        VAR trail : ARRAY[0..511] OF DINT; END_VAR
            trail[n] := n;
            IF n = 0 THEN
                __RAISE('bottom');
            END_IF;
            Depth := Depth(n - 1) + trail[n];
        END_FUNCTION

        FUNCTION Sum : DINT
        VAR_INPUT n : DINT; END_VAR
        VAR trail : ARRAY[0..511] OF DINT; END_VAR
            trail[n] := n;
            IF n = 0 THEN
                Sum := 0;
            ELSE
                Sum := Sum(n - 1) + trail[n];
            END_IF;
        END_FUNCTION

        FUNCTION run : DINT
        VAR i : DINT; caught : DINT; e : STRING; END_VAR
            FOR i := 1 TO 40 DO
                __TRY
                    run := Depth(8);
                __CATCH(e)
                    IF e = 'bottom' THEN
                        caught := caught + 1;
                    END_IF;
                __END_TRY;
            END_FOR;
            run := caught * 1000 + Sum(8);
        END_FUNCTION
    "#;
    assert_eq!(result(&mut with_db, source), 40_036);
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

/// The optimizer takes a module with a `__TRY`, and the module computes
/// the same. Skipped where no wasm-opt is installed, as the release tests
/// are.
#[rstest]
fn an_optimized_build_catches_the_same(mut with_db: db::RootDatabase) {
    if rk::wasm_opt::find(false).is_none() {
        eprintln!("SKIP: no wasm-opt available");
        return;
    }
    let source = r#"
        FUNCTION Depth : DINT
        VAR_INPUT n : DINT; END_VAR
        VAR trail : ARRAY[0..7] OF DINT; END_VAR
            trail[n] := n;
            IF n = 0 THEN
                __RAISE('bottom');
            END_IF;
            Depth := Depth(n - 1) + trail[n];
        END_FUNCTION

        FUNCTION run : DINT
        VAR e : STRING[6]; END_VAR
            __TRY
                run := Depth(4);
            __CATCH(e)
                IF e = 'bottom' THEN
                    run := 7;
                END_IF;
            __END_TRY;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    for level in ["1", "3", "s", "z"] {
        let optimized = rk::compiler::optimize_wasm_release(wasm.clone(), level, false)
            .unwrap_or_else(|e| panic!("-O{level}: {e}"));
        assert_eq!(
            execute_wasm::<(), i32>(&optimized, "run", ()),
            7,
            "-O{level}"
        );
    }
}
