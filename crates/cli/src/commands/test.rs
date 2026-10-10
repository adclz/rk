// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

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
    // An optimized build loses its line tables: the unoptimized one is kept
    // to say where a test failed.
    let unoptimized = opt_level.is_some().then(|| core_bytes.clone());
    let core_bytes = crate::compiler::optimize_wasm(core_bytes, opt_level, verbose);
    let _ = &mir_module;

    // The artifact on disk is what ran. `-O` is neither profile (optimized,
    // but carrying debug sections), so it keeps its own path.
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

    // The roots the manifest named each test's file against.
    let layout = db::workspace::Workspace::try_get(&db);
    let roots = crate::test_runner::SourceRoots {
        workspace: layout
            .and_then(|w| w.workspace_folder(&db))
            .map(|root| root.to_path_buf())
            .unwrap_or_else(|| workspace.to_path_buf()),
        library: layout
            .and_then(|w| w.library_path(&db))
            .map(|lib| lib.to_path_buf()),
    };
    let failures = crate::test_runner::run_tests(
        &wasm_path,
        unoptimized.as_deref(),
        &roots,
        filter,
        timeout,
        format,
    );
    if failures > 0 {
        Err(CliError::Failed)
    } else {
        Ok(())
    }
}
