//! Test helpers and modules for WASM codegen.

use auto_lsp::{
    default::db::{FileManager, file::File},
    lsp_types::Url,
};
use db::RootDatabase;
use hir::{check::diagnostics_for_file, hir_def::semantic_index::semantic_index};
use rstest::*;

use crate::debug::CodeGenConfig;

// Test modules - only execution tests, no validation-only tests
mod arrays;
mod control_flow;
mod debug;
mod execution;
mod function_blocks;
mod ref_to;
mod references;
mod structs;

#[fixture]
pub fn with_db() -> RootDatabase {
    RootDatabase::default()
}

pub fn add_source(db: &mut RootDatabase, source: &str) -> File {
    let url = Url::parse(&format!("file:///test{}.st", rand::random::<u32>())).unwrap();

    let file = File::from_string()
        .db(db)
        .parsers(ast::RK_PARSER.get("structured_text").unwrap())
        .url(&url)
        .source(source.to_string())
        .call()
        .unwrap();

    db.add_file(file).unwrap();
    file
}

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

/// Compile IEC source to WASM with custom code generation configuration.
///
/// Returns both the WASM bytes and debug information.
pub fn compile_to_wasm_with_config(
    db: &mut RootDatabase,
    source: &str,
    config: CodeGenConfig,
) -> (Vec<u8>, crate::debug::DebugInfo) {
    let file = add_source(db, source);
    let sem_idx = semantic_index(db, file);

    let mut codegen = crate::ModuleCodeGen::new_with_config(db, config);

    // Generate code for all POUs in the source
    for pou in sem_idx.global_pous.iter() {
        match pou {
            hir::hir_def::pous::pou::Pou::Function(func) => {
                codegen.generate_function(*func);
            }
            hir::hir_def::pous::pou::Pou::FunctionBlock(fb) => {
                codegen.generate_function_block(*fb);
            }
            hir::hir_def::pous::pou::Pou::Class(class) => {
                codegen.generate_class(*class);
            }
            _ => {}
        }
    }

    // Generate code for all PROGRAMs
    for program in sem_idx.programs.iter() {
        codegen.generate_program(*program);
    }

    // Build the module with all sections
    let mut module = wasm_encoder::Module::new();
    module.section(&codegen.type_section);
    module.section(&codegen.fn_section);

    // Add memory section
    let memory_size = codegen.memory_layout.total_size();
    let memory_min_pages = if memory_size > 0 {
        (memory_size + 65535) / 65536
    } else {
        1
    };

    let mut memory_section = wasm_encoder::MemorySection::new();
    memory_section.memory(wasm_encoder::MemoryType {
        minimum: memory_min_pages as u64,
        maximum: None,
        memory64: false,
        shared: false,
        page_size_log2: None,
    });
    module.section(&memory_section);

    // Add global section if it has debug globals
    if codegen.global_section.len() > 0 {
        module.section(&codegen.global_section);
    }

    // Export memory
    let mut export_section = codegen.export_section;
    export_section.export("memory", wasm_encoder::ExportKind::Memory, 0);

    // Export debug globals if present
    if let Some(idx) = codegen.debug_enabled_global {
        export_section.export("debug_enabled", wasm_encoder::ExportKind::Global, idx);
    }
    if let Some(idx) = codegen.debug_trap_id_global {
        export_section.export("debug_trap_id", wasm_encoder::ExportKind::Global, idx);
    }

    module.section(&export_section);
    module.section(&codegen.code_section);

    (module.finish(), codegen.debug_info)
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
                // Limit to first 10 errors
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

    let mut codegen = crate::ModuleCodeGen::new(db);

    // Generate code for all POUs in the source
    for pou in sem_idx.global_pous.iter() {
        match pou {
            hir::hir_def::pous::pou::Pou::Function(func) => {
                codegen.generate_function(*func);
            }
            hir::hir_def::pous::pou::Pou::FunctionBlock(fb) => {
                codegen.generate_function_block(*fb);
            }
            hir::hir_def::pous::pou::Pou::Class(class) => {
                codegen.generate_class(*class);
            }
            _ => {}
        }
    }

    // Generate code for all PROGRAMs
    for program in sem_idx.programs.iter() {
        codegen.generate_program(*program);
    }

    // Build and return the module with all sections
    let mut module = wasm_encoder::Module::new();
    module.section(&codegen.type_section);
    module.section(&codegen.fn_section);

    // Add memory section if we allocated any memory, or just add a minimal one
    let memory_size = codegen.memory_layout.total_size();
    let memory_min_pages = if memory_size > 0 {
        (memory_size + 65535) / 65536
    } else {
        1 // At least 1 page for pointer operations
    };

    let mut memory_section = wasm_encoder::MemorySection::new();
    memory_section.memory(wasm_encoder::MemoryType {
        minimum: memory_min_pages as u64,
        maximum: None,
        memory64: false,
        shared: false,
        page_size_log2: None,
    });
    module.section(&memory_section);

    // Export memory so tests can access it
    let mut export_section = codegen.export_section;
    export_section.export("memory", wasm_encoder::ExportKind::Memory, 0);
    module.section(&export_section);

    module.section(&codegen.code_section);

    module.finish()
}

/// Helper to validate WASM bytes using wasmtime.
///
/// Wasmtime's Module::new() performs full validation, so we don't need
/// wasmparser for validation anymore. This returns an error if the WASM
/// is invalid, or Ok(()) if it's valid.
pub fn validate_wasm(wasm_bytes: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    let engine = wasmtime::Engine::default();
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
pub fn execute_wasm<P, R>(wasm_bytes: &[u8], func_name: &str, params: P) -> R
where
    P: wasmtime::WasmParams,
    R: wasmtime::WasmResults,
{
    let engine = wasmtime::Engine::default();
    let module = wasmtime::Module::new(&engine, wasm_bytes).expect("Failed to create module");
    let mut store = wasmtime::Store::new(&engine, ());
    let instance =
        wasmtime::Instance::new(&mut store, &module, &[]).expect("Failed to instantiate");

    let func = instance
        .get_typed_func::<P, R>(&mut store, func_name)
        .unwrap_or_else(|_| panic!("Failed to get function '{}'", func_name));

    func.call(&mut store, params)
        .unwrap_or_else(|e| panic!("Failed to call function '{}': {}", func_name, e))
}
