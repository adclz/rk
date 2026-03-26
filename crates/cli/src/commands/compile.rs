use std::path::PathBuf;
use yansi::Paint;

use crate::compiler::{build_core, optimize_wasm};
use crate::workspace::init_db;

pub fn run_compile(
    workspace: &std::path::Path,
    output: Option<&PathBuf>,
    no_stdlib: bool,
    opt_level: Option<&str>,
    verbose: bool,
) {
    let Some(db) = init_db(workspace, verbose, !no_stdlib) else {
        std::process::exit(1);
    };

    let (core_bytes, mir_module) = build_core(&db, workspace, verbose);

    // Release profile: optimize core → wrap in component
    let optimized = optimize_wasm(core_bytes, opt_level, verbose);
    let component_bytes = wasm_codegen::component::wrap_in_component(&db, &optimized, &mir_module)
        .unwrap_or_else(|e| {
            eprintln!("{}{}", "component error: ".bold().red(), e);
            std::process::exit(1);
        });

    // Default output: <workspace>/rk_build/release/output.wasm
    let build_dir = workspace.join("rk_build").join("release");
    let default_output = build_dir.join("output.wasm");
    let output = output.unwrap_or(&default_output);

    // Create output directory if needed
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent).unwrap_or_else(|e| {
            eprintln!(
                "{}failed to create directory {}: {}",
                "error: ".bold().red(),
                parent.display(),
                e
            );
            std::process::exit(1);
        });
    }

    std::fs::write(output, &component_bytes).unwrap_or_else(|e| {
        eprintln!(
            "{}failed to write {}: {}",
            "error: ".bold().red(),
            output.display(),
            e
        );
        std::process::exit(1);
    });

    println!(
        "{}{} ({} bytes)",
        "compiled: ".bold().bright_green(),
        output.display(),
        component_bytes.len(),
    );
}
