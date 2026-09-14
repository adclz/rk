use std::path::PathBuf;

use crate::cli::OutputFormat;
use crate::compiler::{build_core_profile, debug_core_path, optimize_wasm_release};
use crate::error::{CliError, CliResult};
use crate::ui;
use crate::workspace::init_db;

/// Options for `rk compile` (everything except the workspace + verbose, which are
/// common to all commands).
pub struct CompileOptions<'a> {
    pub output: Option<&'a PathBuf>,
    pub opt_level: Option<&'a str>,
    pub release: bool,
    pub format: OutputFormat,
}

pub fn run_compile(
    workspace: &std::path::Path,
    opts: CompileOptions<'_>,
    watch: bool,
    verbose: bool,
) -> CliResult<()> {
    if watch {
        crate::watcher::watch_and_run(workspace, || {
            let _ = compile_once(workspace, &opts, verbose);
        });
        Ok(())
    } else {
        compile_once(workspace, &opts, verbose)
    }
}

fn compile_once(
    workspace: &std::path::Path,
    opts: &CompileOptions<'_>,
    verbose: bool,
) -> CliResult<()> {
    let db = init_db(workspace, verbose, true).ok_or(CliError::Failed)?;

    let profile = if opts.release {
        wasm_codegen::Profile::Release
    } else {
        wasm_codegen::Profile::Debug
    };

    // `build_core` already echoed diagnostics to stderr; a build failure is a
    // silent non-zero exit.
    let (core_bytes, mir_module) =
        build_core_profile(&db, workspace, verbose, opts.format, profile)
            .map_err(|_| CliError::Failed)?;
    let _ = &mir_module;

    if !opts.release {
        // The default: the debug artifact, all sections intact, unoptimized —
        // what `rk test` runs and a debugger steps. Optimizing it would
        // re-encode the code and orphan the line table, so it never is.
        let default_output = debug_core_path(workspace);
        let output = opts.output.unwrap_or(&default_output);
        return write_output(output, &core_bytes, "compiled debug core:");
    }

    // Release: wasm-opt is mandatory and its failure is the build's. Level:
    // flag, else config.toml, else -O2.
    let config = db::config_file::get_config(&db);
    let config_opt = config
        .settings
        .as_ref()
        .and_then(|s| s.opt_level.as_deref());
    let level = opts.opt_level.or(config_opt).unwrap_or("2");

    let optimized =
        optimize_wasm_release(core_bytes, level, verbose).map_err(CliError::msg)?;

    let default_output = crate::compiler::artifact_path(workspace, profile);
    let output = opts.output.unwrap_or(&default_output);
    write_output(output, &optimized, "compiled release:")
}

/// Write `bytes` to `output` (creating parent dirs), reporting success on stdout.
fn write_output(output: &std::path::Path, bytes: &[u8], label: &str) -> CliResult<()> {
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| CliError::msg(format!("creating directory {}: {e}", parent.display())))?;
    }
    std::fs::write(output, bytes)
        .map_err(|e| CliError::msg(format!("writing {}: {e}", output.display())))?;
    ui::success(
        label,
        format!("{} ({} bytes)", output.display(), bytes.len()),
    );
    Ok(())
}
