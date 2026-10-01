//! Namespaced POUs as modules: the semantic index's namespace list is FLAT
//! (nested included), and every consumer that also recursed into children
//! visited nested namespaces twice. Here that doubled the EXPORTS — a module
//! wasm refused to load, from a compile that exited 0.

use crate::tests::codegen::{compile_to_wasm, with_db};
use rstest::*;

/// An FB declared in a NESTED namespace lowers once: the module validates
/// (a duplicate `$__body__` export fails at Module::new) and runs.
#[rstest]
fn a_nested_namespace_fb_exports_once_and_runs(mut with_db: db::RootDatabase) {
    let source = r#"
        NAMESPACE Outer
            NAMESPACE Inner
                FUNCTION_BLOCK Emitter
                    VAR_OUTPUT
                        o : INT;
                    END_VAR
                    o := 42;
                END_FUNCTION_BLOCK

                FUNCTION run : DINT
                    VAR
                        e : Emitter;
                    END_VAR
                    e();
                    run := e.o;
                END_FUNCTION
            END_NAMESPACE
        END_NAMESPACE
    "#;
    let result: i32 = super::run(&mut with_db, source, "Outer.Inner.run", ());
    assert_eq!(result, 42, "the nested FB lowers once and executes");
}

/// A nested {test} is discovered exactly once — the recursive walk found
/// every nested test twice, and each ran (and could fail) twice.
#[rstest]
fn a_nested_test_is_discovered_once(mut with_db: db::RootDatabase) {
    let source = r#"
        NAMESPACE T1
            NAMESPACE T2
                {test}
                FUNCTION test_nested
                END_FUNCTION
            END_NAMESPACE
        END_NAMESPACE
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let tests = crate::tests::codegen::discover_tests(&wasm);
    let hits: Vec<_> = tests
        .iter()
        .filter(|t| t.path.contains("test_nested"))
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "one declaration, one discovery: {:?}",
        tests.iter().map(|t| &t.path).collect::<Vec<_>>()
    );
}

/// The precedence ruling, pinned by VALUE: a file-scope declaration SHADOWS
/// a USING import (before the fix the import silently won — `USING
/// Std.Timers` hijacked a workspace TON), and the qualified path still
/// reaches the import.
#[rstest]
fn a_file_scope_declaration_shadows_a_using_import(mut with_db: db::RootDatabase) {
    let source = r#"
        NAMESPACE N
            FUNCTION_BLOCK X
                VAR_OUTPUT Q : INT; END_VAR
                Q := 1;
            END_FUNCTION_BLOCK
        END_NAMESPACE

        FUNCTION_BLOCK X
            VAR_OUTPUT Q : INT; END_VAR
            Q := 2;
        END_FUNCTION_BLOCK

        USING N;

        FUNCTION f : INT
            VAR a : X; b : N.X; END_VAR
            a();
            b();
            f := a.Q * 10 + b.Q;
        END_FUNCTION
    "#;
    let v: i32 = crate::tests::codegen::run(&mut with_db, source, "f", ());
    assert_eq!(
        v, 21,
        "unqualified = file scope (2), qualified = import (1)"
    );
}

/// A declaration outranks an import wherever the USING is written. A POU's
/// own `USING Lib` took over from its namespace's sibling, from a top-level
/// FUNCTION and from a top-level FB, where the same USING written one level
/// out did not.
#[rstest]
fn a_declaration_outranks_an_import_wherever_the_using_is_written(mut with_db: db::RootDatabase) {
    let source = r#"
        NAMESPACE Lib
            FUNCTION Shared : INT Shared := 1; END_FUNCTION
            FUNCTION Top : INT Top := 1; END_FUNCTION
            FUNCTION_BLOCK X
                VAR_OUTPUT Q : INT; END_VAR
                Q := 1;
            END_FUNCTION_BLOCK
        END_NAMESPACE

        FUNCTION Top : INT Top := 20; END_FUNCTION
        FUNCTION_BLOCK X
            VAR_OUTPUT Q : INT; END_VAR
            Q := 300;
        END_FUNCTION_BLOCK

        NAMESPACE App
            FUNCTION Shared : INT Shared := 4000; END_FUNCTION

            FUNCTION sum : INT
                USING Lib;
                VAR x : X; END_VAR
                x();
                sum := Shared() + Top() + x.Q;
            END_FUNCTION
        END_NAMESPACE

        FUNCTION total : INT
            total := App.sum();
        END_FUNCTION
    "#;
    let v: i32 = crate::tests::codegen::run(&mut with_db, source, "total", ());
    assert_eq!(
        v, 4320,
        "the sibling, the top-level FUNCTION and the top-level FB"
    );
}

/// A PRIVATE function of an imported namespace takes no part in the import
/// when another answers: `F` is `B.F`, the only one callable here, and `H`
/// the top-level one. The private `A.F` made the call ambiguous (E0205).
#[rstest]
fn a_private_function_gives_way_in_an_import(mut with_db: db::RootDatabase) {
    let source = r#"
        NAMESPACE A
            FUNCTION PRIVATE F : INT F := 1; END_FUNCTION
            FUNCTION PRIVATE H : INT H := 1; END_FUNCTION
        END_NAMESPACE
        NAMESPACE B
            FUNCTION F : INT F := 20; END_FUNCTION
        END_NAMESPACE
        FUNCTION H : INT H := 300; END_FUNCTION

        FUNCTION total : INT
            USING A;
            USING B;
            total := F() + H();
        END_FUNCTION
    "#;
    let v: i32 = crate::tests::codegen::run(&mut with_db, source, "total", ());
    assert_eq!(v, 320);
}

/// A relative namespace path binds to the NEAREST enclosing match: `Impl`
/// inside `Lib` (at any depth) is `Lib.Impl`; at global scope it is the
/// top-level `Impl`. Pinned by value, since both candidates resolve.
#[rstest]
fn a_relative_namespace_path_binds_to_the_nearest_match(mut with_db: db::RootDatabase) {
    let source = r#"
        NAMESPACE Impl
            FUNCTION hidden : INT
                hidden := 2;
            END_FUNCTION
        END_NAMESPACE
        NAMESPACE Lib
            NAMESPACE Impl
                FUNCTION hidden : INT
                    hidden := 1;
                END_FUNCTION
            END_NAMESPACE
            FUNCTION api : INT
                api := Impl.hidden();
            END_FUNCTION
            NAMESPACE Deep.Er
                FUNCTION api2 : INT
                    api2 := Impl.hidden() * 10;
                END_FUNCTION
            END_NAMESPACE
        END_NAMESPACE
        FUNCTION top : INT
            top := Impl.hidden();
        END_FUNCTION
        FUNCTION test : INT
            test := Lib.api() * 100 + Lib.Deep.Er.api2() + top();
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(
        result, 112,
        "1 (Lib.Impl) * 100 + 10 (still Lib.Impl two levels down) + 2 (top-level Impl)"
    );
}
