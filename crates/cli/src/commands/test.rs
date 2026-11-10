// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use crate::cli::OutputFormat;
use crate::compiler::build_core_with_format;
use crate::error::{CliError, CliResult};
use crate::workspace::init_db;
use wasm_codegen::Profile;

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

    // Tests default to the unoptimized build but honour `-O`, so an optimized
    // build can be checked to compute the same answers.
    let (core_bytes, mir_module) =
        build_core_with_format(&db, workspace, verbose, format).map_err(|_| CliError::Failed)?;
    // An optimized build loses its line tables: the unoptimized one is kept
    // to say where a test failed.
    let unoptimized = opt_level.is_some().then(|| core_bytes.clone());
    let core_bytes = crate::compiler::optimize_wasm(core_bytes, opt_level, verbose);

    // The tests run on the component a host runs, wrapped as `rk compile`
    // wraps it.
    let project = db::config_file::get_config(&db).project.name.clone();
    let hosts = crate::component::host_functions(&db, &mir_module.extern_functions)
        .map_err(CliError::msg)?;
    let component = crate::component::wrap(&core_bytes, &project, &hosts).map_err(CliError::msg)?;

    // The artifact on disk is what ran, with its WIT beside it. `-O` is
    // neither profile (optimized, but carrying debug sections), so it keeps
    // its own folder.
    let wasm_path = match opt_level {
        None => crate::compiler::component_path(workspace, Profile::Debug, &project),
        Some(level) => crate::compiler::test_component_path(workspace, level, &project),
    };
    if let Some(dir) = wasm_path.parent() {
        std::fs::create_dir_all(dir).ok();
    }
    std::fs::write(&wasm_path, &component.bytes)
        .map_err(|e| CliError::msg(format!("writing the tested component: {e}")))?;
    crate::component::write_wit(&wasm_path, &component).map_err(CliError::msg)?;

    // The roots the manifest named each test's file against.
    let layout = db::workspace::Workspace::try_get(&db);
    let roots = crate::test_runner::SourceRoots {
        workspace: layout
            .and_then(|w| w.workspace_folder(&db))
            .map(|root| root.to_path_buf())
            .unwrap_or_else(|| workspace.to_path_buf()),
        libraries: layout
            .map(|w| w.libraries(&db))
            .into_iter()
            .flatten()
            .map(|library| {
                (
                    mir::lower::lower_module::library_label(library),
                    library.dir.clone(),
                )
            })
            .collect(),
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
