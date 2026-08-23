//! Test helpers and modules for WASM codegen.

use db::RootDatabase;
use hir::{check::diagnostics_for_file, hir_def::semantic_index::semantic_index};

// Test modules - only execution tests, no validation-only tests
mod aggregate_returns;
mod arrays;
mod bit_access;
mod classes;
mod control_flow;
mod debug_functions;
mod debug_lines;
mod debug_stacktrace;
mod debug_symbols;
mod e2e;
mod empty_bodies;
mod enums;
mod exceptions_spike;
mod execution;
mod expt;
mod fb_dispatch;
mod function_blocks;
mod function_inputs;
mod function_outputs;
mod globals;
mod modulo;
mod imports;
mod initializers;
mod inout;
mod instance_initializers;
mod mir_smoke;
mod namespaces;
mod overloads;
mod profile_swap;
mod ref_to;
mod references;
mod schedule;
mod static_strings;
mod string_audit;
mod structs;
mod traps;
mod unary_ops;

pub use crate::tests::utils::with_db;

pub use crate::tests::utils::add_source;

/// Helper function to compile IEC source code to WASM bytes.
///
/// This function:
/// 1. Adds the source to the database
/// 2. Runs semantic indexing
/// 3. Finds all functions and generates WASM code
/// 4. Returns the compiled WASM module as bytes
///
/// Note: WASM validation is not performed here - use wasmtime's Module::new()
/// or wasmparser::validate() on the returned bytes to validate.
///
/// Use `compile_to_wasm_checked` to also validate for diagnostics before compiling.
pub fn compile_to_wasm(db: &mut RootDatabase, source: &str) -> Vec<u8> {
    compile_to_wasm_impl(db, source, false)
}

/// Same as `compile_to_wasm` but panics if the source has any diagnostics.
///
/// Use this when you want to ensure the source is error-free before compiling.
pub fn compile_to_wasm_checked(db: &mut RootDatabase, source: &str) -> Vec<u8> {
    compile_to_wasm_impl(db, source, true)
}

/// Lower IEC source to MIR and WASM in a single pass, returning both. Use this
/// when a test needs to inspect the MIR layout (e.g. the retain band bounds or
/// a variable's storage) and run the emitted module against it. Lowering only
/// once avoids registering the same source twice (which would duplicate POUs).
pub fn compile_to_mir_and_wasm(db: &mut RootDatabase, source: &str) -> (mir::MirModule, Vec<u8>) {
    let file = add_source(db, source);
    let sem_idx = semantic_index(db, file);
    let mir_module =
        mir::lower::lower_module::lower_module(db, sem_idx).expect("MIR lowering failed");
    let wasm = wasm_codegen::generate_wasm(db, &mir_module).finish();
    (mir_module, wasm)
}

fn compile_to_wasm_impl(db: &mut RootDatabase, source: &str, check_diagnostics: bool) -> Vec<u8> {
    let file = add_source(db, source);
    let sem_idx = semantic_index(db, file);

    // Optionally check for diagnostics before compiling
    if check_diagnostics {
        let diagnostics = diagnostics_for_file(db, file);
        if !diagnostics.is_empty() {
            let mut error_msg = format!(
                "Source has {} diagnostic(s), cannot compile:\n",
                diagnostics.len()
            );
            for diag in diagnostics.iter().take(10) {
                let inner = &diag.diagnostic;
                error_msg.push_str(&format!("  [{:?}] {}\n", inner.severity, inner.message));
            }
            if diagnostics.len() > 10 {
                error_msg.push_str(&format!(
                    "  ... and {} more diagnostics\n",
                    diagnostics.len() - 10
                ));
            }
            panic!("{}", error_msg);
        }
    }

    // MIR pipeline: HIR → MIR → WASM
    let mir_module =
        mir::lower::lower_module::lower_module(db, sem_idx).expect("MIR lowering failed");

    let wasm_module = wasm_codegen::generate_wasm(db, &mir_module);
    wasm_module.finish()
}

/// Helper to validate WASM bytes using wasmtime.
///
/// Wasmtime's Module::new() performs full validation, so we don't need
/// wasmparser for validation anymore. This returns an error if the WASM
/// is invalid, or Ok(()) if it's valid.
pub fn validate_wasm(wasm_bytes: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    let engine = test_engine();
    let _ = wasmtime::Module::new(&engine, wasm_bytes)?;
    Ok(())
}

/// Helper to execute a WASM function and return its result.
///
/// This is a generic helper that works with any function signature that
/// implements wasmtime's WasmParams and WasmResults traits.
///
/// # Examples
///
/// ```ignore
/// // No parameters, returns i32
/// let result: i32 = execute_wasm(&wasm_bytes, "my_func", ());
///
/// // Two i32 parameters, returns i32
/// let result: i32 = execute_wasm(&wasm_bytes, "add", (5, 3));
///
/// // One i32 parameter, returns f32
/// let result: f32 = execute_wasm(&wasm_bytes, "to_float", 42);
/// ```
/// Instantiate a core module produced by `generate_wasm`, providing the
/// `env.memory` import that the module now requires (see `WasmGen::new`).
///
/// Use in tests that build their own engine/store rather than going through
/// `execute_wasm` / `execute_wasm_with_imports`.
/// An engine configured the way the real runtime's is: exception handling on.
/// Any module whose lowering pulled in `rk.idx_check` (every runtime array
/// subscript), `RAISE`, or an assertion carries the `$rk_exception` tag, and
/// wasmtime's DEFAULT config refuses to even parse it.
pub fn test_engine() -> wasmtime::Engine {
    let mut config = wasmtime::Config::new();
    config.wasm_exceptions(true);
    wasmtime::Engine::new(&config).expect("engine with exceptions")
}

pub fn instantiate_with_memory(
    store: &mut wasmtime::Store<()>,
    module: &wasmtime::Module,
) -> wasmtime::Instance {
    instantiate_returning_memory(store, module).0
}

/// As [`instantiate_with_memory`], keeping the memory handle — a fault test
/// needs it to read the `$rk_exception` payload out of linear memory.
pub fn instantiate_returning_memory(
    store: &mut wasmtime::Store<()>,
    module: &wasmtime::Module,
) -> (wasmtime::Instance, wasmtime::Memory) {
    let memory =
        wasmtime::Memory::new(&mut *store, wasmtime::MemoryType::new(1, None)).expect("memory");
    let instance = wasmtime::Instance::new(store, module, &[memory.into()])
        .expect("Failed to instantiate with memory");
    (instance, memory)
}

/// What a failed call says to whoever reads the fault: the `$rk_exception`
/// payload when one is pending (the same decode the runtime does, so a test
/// asserts the message a user would see), else the trap's own words. Asserting
/// only `is_err()` lets a named fault silently degrade into "thrown Wasm
/// exception".
pub fn fault_message(
    store: &mut wasmtime::Store<()>,
    memory: wasmtime::Memory,
    err: wasmtime::Error,
) -> String {
    let Some(exn) = store.take_pending_exception() else {
        return format!("{err:?}");
    };
    let (Ok(wasmtime::Val::I32(ptr)), Ok(wasmtime::Val::I32(len))) =
        (exn.field(&mut *store, 0), exn.field(&mut *store, 1))
    else {
        return format!("{err:?}");
    };
    let data = memory.data(&*store);
    data.get(ptr as u32 as usize..(ptr as u32 as usize).saturating_add(len as u32 as usize))
        .map(|b| String::from_utf8_lossy(b).into_owned())
        .unwrap_or_else(|| "<exception payload out of bounds>".to_string())
}

pub fn execute_wasm<P, R>(wasm_bytes: &[u8], func_name: &str, params: P) -> R
where
    P: wasmtime::WasmParams,
    R: wasmtime::WasmResults,
{
    execute_wasm_with_imports(wasm_bytes, func_name, params, |_| {})
}

/// Helper to execute a WASM function that requires imports (extern pragmas).
///
/// Uses a wasmtime Linker to provide the import functions before instantiation.
/// The `define_imports` closure receives a `&mut Linker<()>` to register host functions.
pub fn execute_wasm_with_imports<P, R, F>(
    wasm_bytes: &[u8],
    func_name: &str,
    params: P,
    define_imports: F,
) -> R
where
    P: wasmtime::WasmParams,
    R: wasmtime::WasmResults,
    F: FnOnce(&mut wasmtime::Linker<()>),
{
    let engine = test_engine();
    let module = wasmtime::Module::new(&engine, wasm_bytes).expect("Failed to create module");
    let mut store = wasmtime::Store::new(&engine, ());
    let mut linker = wasmtime::Linker::new(&engine);

    // The core module imports its memory from `env`. Provide a host-owned memory
    // that the module can load/store into during the test.
    let memory =
        wasmtime::Memory::new(&mut store, wasmtime::MemoryType::new(1, None)).expect("memory");
    linker
        .define(&store, "env", "memory", memory)
        .expect("define env.memory");

    define_imports(&mut linker);

    let instance = linker
        .instantiate(&mut store, &module)
        .expect("Failed to instantiate with imports");

    let func = instance
        .get_typed_func::<P, R>(&mut store, func_name)
        .unwrap_or_else(|_| panic!("Failed to get function '{}'", func_name));

    func.call(&mut store, params)
        .unwrap_or_else(|e| panic!("Failed to call function '{}': {}", func_name, e))
}
