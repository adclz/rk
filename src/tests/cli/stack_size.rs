// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! `stack_size` in `config.toml` sets the size of the stack recursive calls
//! push their frames on. A stack that cannot hold the largest frame, or that
//! does not fit in the 4 GiB a module can address, stops the build with an
//! error naming the setting, before codegen lays out a module that would
//! fail at run time.

use rk::workspace::init_db;

use super::{disable_stdlib, temp_workspace};

/// A recursive function with a frame of 80000 bytes.
const DEEP: &str = "FUNCTION deep : DINT
VAR_INPUT n : DINT; END_VAR
VAR cells : ARRAY[0..19999] OF DINT; END_VAR
    cells[0] := n;
    IF n > 0 THEN deep := deep(n - 1); ELSE deep := cells[0]; END_IF;
END_FUNCTION
";

/// Build a workspace holding [`DEEP`], with `stack_size` set to `size`.
fn build(size: u64) -> Result<(Vec<u8>, mir::MirModule), String> {
    let config =
        format!("[project]\nname = \"T\"\nversion = \"0.0\"\n\n[settings]\nstack_size = {size}\n");
    let (_ws, root) = temp_workspace(&[("config.toml", &config), ("main.st", DEEP)]);
    disable_stdlib();
    let db = init_db(&root, false, true).expect("init db");
    rk::compiler::build_core_quiet(&db, &root, false)
}

/// The pages of memory the module asks its host for.
fn memory_pages(wasm: &[u8]) -> u64 {
    let engine = crate::tests::codegen::test_engine();
    let module = wasmtime::Module::new(&engine, wasm).expect("a valid module");
    module
        .imports()
        .find_map(|import| match import.ty() {
            wasmtime::ExternType::Memory(memory) => Some(memory.minimum()),
            _ => None,
        })
        .expect("the module imports its memory")
}

#[test]
fn a_stack_size_sets_the_memory_the_module_asks_for() {
    let (wasm, _) = build(1 << 20).expect("a 1 MiB stack fits");
    // 1 MiB is 16 pages of 64 KiB, after the memory before the stack.
    assert!(memory_pages(&wasm) > 16, "{} pages", memory_pages(&wasm));
}

#[test]
fn a_stack_smaller_than_a_frame_is_refused() {
    let error = build(16384).expect_err("the frame does not fit");
    assert!(
        error.contains(
            "`stack_size` in config.toml is 16384 bytes, smaller than the 80000-byte frame of 'deep'"
        ),
        "{error}"
    );
}

#[test]
fn a_stack_past_the_memory_is_refused() {
    let error = build(4 << 30).expect_err("the stack passes the last address");
    assert!(
        error.contains(
            "`stack_size` in config.toml is 4294967296 bytes, and the memory before the stack"
        ),
        "{error}"
    );
    assert!(
        error.contains("a module addresses at most 4 GiB"),
        "{error}"
    );
}
