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
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "Outer.Inner.run", ());
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
    let tests = runtime::test::discover(&wasm);
    let hits: Vec<_> = tests.iter().filter(|t| t.path.contains("test_nested")).collect();
    assert_eq!(
        hits.len(),
        1,
        "one declaration, one discovery: {:?}",
        tests.iter().map(|t| &t.path).collect::<Vec<_>>()
    );
}
