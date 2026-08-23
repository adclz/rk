use crate::cli::OutputFormat;
use crate::compiler::build_core_with_format;
use crate::error::{CliError, CliResult};
use crate::workspace::init_db;

pub fn run_test(
    workspace: &std::path::Path,
    filter: Option<&str>,
    opt_level: Option<&str>,
    timeout: Option<&str>,
    verbose: bool,
    format: OutputFormat,
) -> CliResult<()> {
    let timeout = timeout.map(crate::duration::parse).transpose()?;
    let db = init_db(workspace, verbose, true).ok_or(CliError::Failed)?;

    // Tests default to the unoptimized core but honour `-O`, so an optimized
    // build can be checked to compute the same answers.
    let (core_bytes, mir_module) =
        build_core_with_format(&db, workspace, verbose, format).map_err(|_| CliError::Failed)?;
    let core_bytes = crate::compiler::optimize_wasm(core_bytes, opt_level, verbose);
    let _ = &mir_module;

    // The artifact on disk is what the runtime is handed — and what a failing
    // run is reproducible from: `runtime --test <that path>` repeats it
    // exactly, with no compiler in the picture.
    // `-O` makes this neither profile — an optimized build that still carries
    // debug sections — so it keeps its own path rather than overwriting the
    // debug artifact with something that is not one. That mongrel is a known
    // defect in its own right (its line table no longer describes its code);
    // giving it a separate file names it rather than hiding it.
    let wasm_path = match opt_level {
        None => crate::compiler::debug_core_path(workspace),
        Some(level) => workspace
            .join("rk_build")
            .join(format!("test-O{level}"))
            .join("core.wasm"),
    };
    if let Some(dir) = wasm_path.parent() {
        std::fs::create_dir_all(dir).ok();
    }
    std::fs::write(&wasm_path, &core_bytes)
        .map_err(|e| CliError::msg(format!("writing test binary: {e}")))?;

    let failures = crate::test_runner::run_tests(&wasm_path, filter, timeout, format);
    if failures > 0 {
        Err(CliError::Failed)
    } else {
        Ok(())
    }
}
