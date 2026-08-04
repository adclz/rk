use crate::cli::OutputFormat;
use crate::compiler::build_core_with_format;
use crate::error::{CliError, CliResult};
use crate::workspace::init_db;

pub fn run_test(
    workspace: &std::path::Path,
    no_stdlib: bool,
    filter: Option<&str>,
    opt_level: Option<&str>,
    verbose: bool,
    format: OutputFormat,
) -> CliResult<()> {
    let db = init_db(workspace, verbose, !no_stdlib, true).ok_or(CliError::Failed)?;

    // Tests default to the unoptimized core but honour `-O`, so an optimized
    // build can be checked to compute the same answers.
    let (core_bytes, mir_module) =
        build_core_with_format(&db, workspace, verbose, format).map_err(|_| CliError::Failed)?;
    let core_bytes = crate::compiler::optimize_wasm(core_bytes, opt_level, verbose);
    let component_bytes = wasm_codegen::component::wrap_in_component(&db, &core_bytes, &mir_module)
        .map_err(|e| CliError::msg(format!("component: {e}")))?;

    // Write the component to rk_build/test/. The test manifest is embedded as a
    // custom section inside the component (see `wrap_in_component`), so there is
    // no sidecar file to write.
    let build_dir = workspace.join("rk_build").join("test");
    std::fs::create_dir_all(&build_dir).ok();
    let wasm_path = build_dir.join("output.wasm");
    std::fs::write(&wasm_path, &component_bytes)
        .map_err(|e| CliError::msg(format!("writing test binary: {e}")))?;

    let failures = crate::test_runner::run_tests(&wasm_path, filter, format);
    if failures > 0 {
        Err(CliError::Failed)
    } else {
        Ok(())
    }
}
