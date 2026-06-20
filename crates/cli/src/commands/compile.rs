use std::path::PathBuf;
use yansi::Paint;

use crate::compiler::{build_core, debug_core_path, optimize_wasm};
use crate::workspace::init_db;

#[allow(clippy::too_many_arguments)]
pub fn run_compile(
    workspace: &std::path::Path,
    output: Option<&PathBuf>,
    no_stdlib: bool,
    opt_level: Option<&str>,
    debug: bool,
    watch: bool,
    verbose: bool,
) {
    if watch {
        crate::watcher::watch_and_run(workspace, || {
            compile_once(workspace, output, no_stdlib, opt_level, debug, verbose);
        });
    } else {
        compile_once(workspace, output, no_stdlib, opt_level, debug, verbose);
    }
}

fn compile_once(
    workspace: &std::path::Path,
    output: Option<&PathBuf>,
    no_stdlib: bool,
    opt_level: Option<&str>,
    debug: bool,
    verbose: bool,
) {
    let Some(db) = init_db(workspace, verbose, !no_stdlib) else {
        return;
    };

    let (core_bytes, mir_module) = match build_core(&db, workspace, verbose) {
        Ok(v) => v,
        Err(_) => return, // diagnostics already echoed to stderr
    };

    if debug {
        // Debug profile: write the bare core (unoptimized, `debug-*` sections
        // intact) that the debugger loads. No optimize/component — optimization
        // strips the debug sections and the DebugSession loads the core directly.
        let default_output = debug_core_path(workspace);
        let output = output.unwrap_or(&default_output);
        write_output(output, &core_bytes, "compiled debug core: ");
        return;
    }

    // Release profile: optimize core → wrap in component.
    // CLI flag takes precedence over config.toml.
    let config = db::config_file::get_config(&db);
    let config_opt = config
        .settings
        .as_ref()
        .and_then(|s| s.opt_level.as_deref());
    let effective_opt = opt_level.or(config_opt);

    let optimized = optimize_wasm(core_bytes, effective_opt, verbose);
    let component_bytes =
        match wasm_codegen::component::wrap_in_component(&db, &optimized, &mir_module) {
            Ok(bytes) => bytes,
            Err(e) => {
                eprintln!("{}{}", "component error: ".bold().red(), e);
                return;
            }
        };

    let default_output = workspace.join("rk_build").join("release").join("output.wasm");
    let output = output.unwrap_or(&default_output);
    write_output(output, &component_bytes, "compiled: ");
}

/// Write `bytes` to `output` (creating parent dirs), reporting on stdout with a
/// green `label` prefix. Errors go to stderr.
fn write_output(output: &std::path::Path, bytes: &[u8], label: &str) {
    if let Some(parent) = output.parent()
        && let Err(e) = std::fs::create_dir_all(parent)
    {
        eprintln!(
            "{}failed to create directory {}: {}",
            "error: ".bold().red(),
            parent.display(),
            e
        );
        return;
    }
    if let Err(e) = std::fs::write(output, bytes) {
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
        label.bold().bright_green(),
        output.display(),
        bytes.len(),
    );
}
