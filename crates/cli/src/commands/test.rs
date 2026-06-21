use crate::compiler::build_core;
use crate::error::{CliError, CliResult};
use crate::workspace::init_db;

pub fn run_test(
    workspace: &std::path::Path,
    no_stdlib: bool,
    filter: Option<&str>,
    _opt_level: Option<&str>,
    verbose: bool,
) -> CliResult<()> {
    let db = init_db(workspace, verbose, !no_stdlib).ok_or(CliError::Failed)?;

    // Test profile: core module → component (no optimization).
    let (core_bytes, mir_module) =
        build_core(&db, workspace, verbose).map_err(|_| CliError::Failed)?;
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

    let failures = crate::test_runner::run_tests(&wasm_path, filter);
    if failures > 0 {
        Err(CliError::Failed)
    } else {
        Ok(())
    }
}
