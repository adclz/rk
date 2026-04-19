use auto_lsp::default::db::BaseDatabase;
use db::{RootDatabase, WorkspaceDataBase};
use hir::hir_def::semantic_index::semantic_index;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use yansi::Paint;

use crate::diagnostics::report_diagnostics;

/// Check diagnostics and lower HIR → MIR → core WASM.
/// Returns `None` if there are errors or codegen fails.
pub fn build_core(
    db: &RootDatabase,
    workspace: &std::path::Path,
    _verbose: bool,
) -> Option<(Vec<u8>, mir::MirModule)> {
    let workspace_path =
        std::fs::canonicalize(workspace).unwrap_or_else(|_| workspace.to_path_buf());
    let config = ariadne::Config::new().with_color(true).with_tab_width(2);

    let caches = db
        .get_files()
        .iter()
        .map(|file| (file.url(db).as_str(), file.document(db).as_str()))
        .collect::<Vec<(&str, &str)>>();

    // Collect diagnostics in parallel across files
    let files = db.get_files();
    let per_file: Vec<_> = files
        .into_par_iter()
        .map_with(db.clone(), |db, file| {
            let file = *file;
            let diagnostics = hir::check::diagnostics_for_file(db, file).as_ref().clone();
            (file, diagnostics)
        })
        .collect();

    // Report sequentially
    let mut total_errors = 0;
    let mut total_warnings = 0;

    for (file, diagnostics) in &per_file {
        if !diagnostics.is_empty() {
            report_diagnostics(
                db,
                &workspace_path,
                config,
                file.url(db),
                &file.document(db).texter.text,
                diagnostics,
                &caches,
                &mut total_errors,
                &mut total_warnings,
            );
        }
    }

    if total_errors > 0 {
        eprintln!(
            "\n{}{} error(s) found, cannot compile.",
            "compilation failed: ".bold().red(),
            total_errors,
        );
        return None;
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
            eprintln!("{}{}", "codegen error: ".bold().red(), e);
            return None;
        }
    };

    let wasm_module = wasm_codegen::generate_wasm(db, &mir_module);
    Some((wasm_module.finish(), mir_module))
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
            eprintln!(
                "{}unknown optimization level '{}' (valid: 0-4, s, z)",
                "warning: ".bold().yellow(),
                level
            );
            return wasm_bytes;
        }
    };

    if verbose {
        println!("{}wasm-opt -O{}", "    Optimizing ".dim(), level);
    }

    let original_size = wasm_bytes.len();

    // wasm-opt requires file paths - use temp files
    let infile = std::env::temp_dir().join("rk_wasm_opt_in.wasm");
    let outfile = std::env::temp_dir().join("rk_wasm_opt_out.wasm");

    if let Err(e) = std::fs::write(&infile, &wasm_bytes) {
        eprintln!(
            "{}failed to write temp file: {}",
            "warning: ".bold().yellow(),
            e
        );
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
                println!(
                    "{}{} → {} bytes ({:.1}% reduction)",
                    "    Optimized ".dim(),
                    original_size,
                    optimized.len(),
                    pct
                );
            }
            optimized
        }
        Err(e) => {
            let _ = std::fs::remove_file(&infile);
            let _ = std::fs::remove_file(&outfile);
            eprintln!("{}wasm-opt failed: {}", "warning: ".bold().yellow(), e);
            wasm_bytes
        }
    }
}
