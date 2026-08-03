use std::path::PathBuf;

use crate::cli::OutputFormat;
use crate::compiler::{build_core_with_format, debug_core_path, optimize_wasm};
use crate::error::{CliError, CliResult};
use crate::ui;
use crate::workspace::init_db;

/// Options for `rk compile` (everything except the workspace + verbose, which are
/// common to all commands).
pub struct CompileOptions<'a> {
    pub output: Option<&'a PathBuf>,
    pub no_stdlib: bool,
    pub opt_level: Option<&'a str>,
    pub debug: bool,
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
    let db = init_db(workspace, verbose, !opts.no_stdlib).ok_or(CliError::Failed)?;

    // `build_core` already echoed diagnostics to stderr; a build failure is a
    // silent non-zero exit.
    let (core_bytes, mir_module) = build_core_with_format(&db, workspace, verbose, opts.format)
        .map_err(|_| CliError::Failed)?;

    if opts.debug {
        // Debug profile: write the bare core (unoptimized, `debug-*` sections
        // intact) that the debugger loads. No optimize/component — optimization
        // strips the debug sections and the DebugSession loads the core directly.
        let default_output = debug_core_path(workspace);
        let output = opts.output.unwrap_or(&default_output);
        return write_output(output, &core_bytes, "compiled debug core:");
    }

    // Release profile: optimize core → wrap in component. CLI flag takes
    // precedence over config.toml.
    let config = db::config_file::get_config(&db);
    let config_opt = config
        .settings
        .as_ref()
        .and_then(|s| s.opt_level.as_deref());
    let effective_opt = opts.opt_level.or(config_opt);

    let optimized = optimize_wasm(core_bytes, effective_opt, verbose);
    let component_bytes = wasm_codegen::component::wrap_in_component(&db, &optimized, &mir_module)
        .map_err(|e| CliError::msg(format!("component: {e}")))?;

    let default_output = workspace
        .join("rk_build")
        .join("release")
        .join("output.wasm");
    let output = opts.output.unwrap_or(&default_output);
    write_output(output, &component_bytes, "compiled:")
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
