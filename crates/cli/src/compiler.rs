use std::io::Write;

use auto_lsp::default::db::BaseDatabase;
use db::{RootDatabase, WorkspaceDataBase};
use hir::hir_def::semantic_index::semantic_index;

use crate::diagnostics::{DiagnosticReporter, collect_diagnostics};
use crate::ui;

/// Check diagnostics and lower HIR → MIR → core WASM. On success returns the
/// core wasm + MIR; on failure returns the rendered diagnostics (also echoed to
/// stderr) so callers like the debugger can forward them over the debugger transport.
pub fn build_core(
    db: &RootDatabase,
    workspace: &std::path::Path,
    _verbose: bool,
) -> Result<(Vec<u8>, mir::MirModule), String> {
    // Report into a buffer so the text can be both echoed to stderr (CLI
    // commands) and returned to the caller (the debugger forwards it over the debugger
    // transport — stdout there is the transport, and stderr isn't shown in VSCode).
    let per_file = collect_diagnostics(db, false);
    let reporter = DiagnosticReporter::new(db, workspace);
    let mut rendered: Vec<u8> = Vec::new();
    let (total_errors, _total_warnings) = reporter.report_files(&per_file, &mut rendered);

    // Echo to stderr for CLI usage; the debugger reads the returned string instead.
    let _ = std::io::stderr().write_all(&rendered);

    if total_errors > 0 {
        ui::failure(
            "compilation failed:",
            format!("{total_errors} error(s) found, cannot compile."),
        );
        let mut text = String::from_utf8_lossy(&rendered).into_owned();
        text.push_str(&format!(
            "\ncompilation failed: {total_errors} error(s) found, cannot compile.\n"
        ));
        return Err(text);
    }

    let sem_indices: Vec<_> = db
        .get_files()
        .iter()
        .chain(db.get_std_lib_files().iter())
        .map(|file| semantic_index(db, *file))
        .collect();

    let mir_module = match mir::lower::lower_module::lower_modules(db, &sem_indices) {
        Ok(m) => m,
        Err(e) => {
            ui::error(format!("codegen: {e}"));
            return Err(format!("codegen error: {e}"));
        }
    };

    let wasm_module = wasm_codegen::generate_wasm(db, &mir_module);
    Ok((wasm_module.finish(), mir_module))
}

/// Path of the debug **core** artifact: `rk compile --debug` writes it and
/// the debugger loads it.
pub fn debug_core_path(workspace: &std::path::Path) -> std::path::PathBuf {
    workspace.join("rk_build").join("debug").join("core.wasm")
}

/// Run wasm-opt on the WASM bytes if an optimization level is specified.
pub fn optimize_wasm(wasm_bytes: Vec<u8>, opt_level: Option<&str>, verbose: bool) -> Vec<u8> {
    let Some(level) = opt_level else {
        return wasm_bytes;
    };

    let opts = match level {
        "0" => wasm_opt::OptimizationOptions::new_opt_level_0(),
        "1" => wasm_opt::OptimizationOptions::new_opt_level_1(),
        "2" => wasm_opt::OptimizationOptions::new_opt_level_2(),
        "3" => wasm_opt::OptimizationOptions::new_opt_level_3(),
        "4" => wasm_opt::OptimizationOptions::new_opt_level_4(),
        "s" => wasm_opt::OptimizationOptions::new_optimize_for_size(),
        "z" => wasm_opt::OptimizationOptions::new_optimize_for_size_aggressively(),
        _ => {
            ui::warn(format!(
                "unknown optimization level '{level}' (valid: 0-4, s, z)"
            ));
            return wasm_bytes;
        }
    };

    if verbose {
        ui::detail(format!("    Optimizing wasm-opt -O{level}"));
    }

    let original_size = wasm_bytes.len();

    // wasm-opt requires file paths - use temp files
    let infile = std::env::temp_dir().join("rk_wasm_opt_in.wasm");
    let outfile = std::env::temp_dir().join("rk_wasm_opt_out.wasm");

    if let Err(e) = std::fs::write(&infile, &wasm_bytes) {
        ui::warn(format!("failed to write temp file: {e}"));
        return wasm_bytes;
    }

    match opts.run(&infile, &outfile) {
        Ok(()) => {
            let optimized = std::fs::read(&outfile).unwrap_or_else(|_| wasm_bytes.clone());
            let _ = std::fs::remove_file(&infile);
            let _ = std::fs::remove_file(&outfile);

            if verbose {
                let savings = original_size as f64 - optimized.len() as f64;
                let pct = (savings / original_size as f64) * 100.0;
                ui::detail(format!(
                    "    Optimized {} → {} bytes ({:.1}% reduction)",
                    original_size,
                    optimized.len(),
                    pct
                ));
            }
            optimized
        }
        Err(e) => {
            let _ = std::fs::remove_file(&infile);
            let _ = std::fs::remove_file(&outfile);
            ui::warn(format!("wasm-opt failed: {e}"));
            wasm_bytes
        }
    }
}
