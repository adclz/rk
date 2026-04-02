use std::path::PathBuf;
use yansi::Paint;

use crate::compiler::{build_core, optimize_wasm};
use crate::workspace::init_db;

pub fn run_compile(
    workspace: &std::path::Path,
    output: Option<&PathBuf>,
    no_stdlib: bool,
    opt_level: Option<&str>,
    watch: bool,
    verbose: bool,
) {
    if watch {
        crate::watcher::watch_and_run(workspace, || {
            compile_once(workspace, output, no_stdlib, opt_level, verbose);
        });
    } else {
        compile_once(workspace, output, no_stdlib, opt_level, verbose);
    }
}

fn compile_once(
    workspace: &std::path::Path,
    output: Option<&PathBuf>,
    no_stdlib: bool,
    opt_level: Option<&str>,
    verbose: bool,
) {
    let Some(db) = init_db(workspace, verbose, !no_stdlib) else {
        return;
    };

    // CLI flag takes precedence over config.toml
    let config = db::config_file::get_config(&db);
    let config_opt = config
        .settings
        .as_ref()
        .and_then(|s| s.opt_level.as_deref());
    let effective_opt = opt_level.or(config_opt);

    let Some((core_bytes, mir_module)) = build_core(&db, workspace, verbose) else {
        return;
    };

    // Release profile: optimize core → wrap in component
    let optimized = optimize_wasm(core_bytes, effective_opt, verbose);
    let component_bytes =
        match wasm_codegen::component::wrap_in_component(&db, &optimized, &mir_module) {
            Ok(bytes) => bytes,
            Err(e) => {
                eprintln!("{}{}", "component error: ".bold().red(), e);
                return;
            }
        };

    // Default output: <workspace>/rk_build/release/output.wasm
    let build_dir = workspace.join("rk_build").join("release");
    let default_output = build_dir.join("output.wasm");
    let output = output.unwrap_or(&default_output);

    // Create output directory if needed
    if let Some(parent) = output.parent()
        && let Err(e) = std::fs::create_dir_all(parent) {
            eprintln!(
                "{}failed to create directory {}: {}",
                "error: ".bold().red(),
                parent.display(),
                e
            );
            return;
        }

    if let Err(e) = std::fs::write(output, &component_bytes) {
        eprintln!(
            "{}failed to write {}: {}",
            "error: ".bold().red(),
            output.display(),
            e
        );
        return;
    }

    println!(
        "{}{} ({} bytes)",
        "compiled: ".bold().bright_green(),
        output.display(),
        component_bytes.len(),
    );
}
