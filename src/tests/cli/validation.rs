// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! A build validates the module it emits, with the proposals rk's output may
//! use and no others. A fault of the code generator stops the build as an
//! internal compiler error instead of reaching a host.

use rk::workspace::init_db;

use super::{disable_stdlib, temp_workspace};

/// A module that raises, returns a STRING and copies an array uses
/// exceptions, multiple results and bulk memory: the build accepts it.
#[test]
fn a_build_validates_what_it_emits() {
    let source = "FUNCTION Pick : STRING
VAR_INPUT n : DINT; END_VAR
    IF n < 0 THEN
        __RAISE('negative');
    END_IF;
    Pick := 'ok';
END_FUNCTION

PROGRAM P
VAR a, b : ARRAY[0..9] OF DINT; s : STRING; END_VAR
    b := a;
    s := Pick(1);
END_PROGRAM
";
    let (_ws, root) = temp_workspace(&[("main.st", source)]);
    disable_stdlib();
    let db = init_db(&root, false, true).expect("init db");
    let (wasm, _) = rk::compiler::build_core_quiet(&db, &root, false).expect("it builds");
    wasm_codegen::validate(&wasm).expect("what the build returns validates");
}

/// A module using a proposal rk's output does not, a 64-bit memory, is
/// refused: one a code generator emitted by mistake would not load
/// everywhere.
#[test]
fn a_module_using_another_proposal_is_refused() {
    use wasm_encoder::{MemorySection, MemoryType, Module};
    let mut memories = MemorySection::new();
    memories.memory(MemoryType {
        minimum: 1,
        maximum: None,
        memory64: true,
        shared: false,
        page_size_log2: None,
    });
    let mut module = Module::new();
    module.section(&memories);
    let error = wasm_codegen::validate(&module.finish()).expect_err("memory64 is not allowed");
    assert!(error.to_string().contains("memory64"), "{error}");
}
