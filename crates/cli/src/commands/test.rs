use crate::cli::OutputFormat;
use crate::compiler::build_core_with_format;
use crate::error::{CliError, CliResult};
use crate::workspace::init_db;

pub fn run_test(
    workspace: &std::path::Path,
    filter: Option<&str>,
    opt_level: Option<&str>,
    verbose: bool,
    format: OutputFormat,
) -> CliResult<()> {
    let db = init_db(workspace, verbose, true).ok_or(CliError::Failed)?;

    // Tests default to the unoptimized core but honour `-O`, so an optimized
    // build can be checked to compute the same answers.
    let (core_bytes, mir_module) =
        build_core_with_format(&db, workspace, verbose, format).map_err(|_| CliError::Failed)?;
    let core_bytes = crate::compiler::optimize_wasm(core_bytes, opt_level, verbose);
    let _ = &mir_module;

    // The artifact on disk is what the runtime is handed — and what a failing
    // run is reproducible from: `runtime --test rk_build/test/output.wasm`
    // repeats it exactly, with no compiler in the picture.
    let build_dir = workspace.join("rk_build").join("test");
    std::fs::create_dir_all(&build_dir).ok();
    let wasm_path = build_dir.join("output.wasm");
    std::fs::write(&wasm_path, &core_bytes)
        .map_err(|e| CliError::msg(format!("writing test binary: {e}")))?;

    let failures = crate::test_runner::run_tests(&wasm_path, filter, format);
    if failures > 0 {
        Err(CliError::Failed)
    } else {
        Ok(())
    }
}
