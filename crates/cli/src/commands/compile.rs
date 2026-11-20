// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use std::path::PathBuf;

use crate::cli::OutputFormat;
use crate::compiler::{build_core_profile, component_path, optimize_wasm_release};
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
    // A library is code a program takes in: checked and tested in its own
    // folder, built into each program that lists it, and never alone.
    if db::workspace::Workspace::get(&db).is_library(&db) {
        return Err(CliError::msg(
            "this workspace is a library (`type = \"library\"`): `rk check` and `rk test` take it, and a program that lists it in its `[dependencies]` builds it",
        ));
    }

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
    let config = db::config_file::get_config(&db);

    let (core_bytes, label) = if !opts.release {
        // The default: the debug artifact, all sections intact, never optimized
        // (that would orphan the line table).
        (core_bytes, "compiled debug:")
    } else {
        // Release: wasm-opt is mandatory and its failure is the build's.
        // Level: flag, else config.toml, else -O2.
        let config_opt = config
            .settings
            .as_ref()
            .and_then(|s| s.opt_level.as_deref());
        let level = opts.opt_level.or(config_opt).unwrap_or("2");
        let optimized = optimize_wasm_release(core_bytes, level, verbose).map_err(CliError::msg)?;
        (optimized, "compiled release:")
    };

    // Last, since wasm-opt reads core modules only: the module wrapped into a
    // component, which carries the types of what it imports and exports.
    let interfaces = crate::component::interfaces(&db, &mir_module).map_err(CliError::msg)?;
    let component = crate::component::wrap(&core_bytes, &config.project.name, &interfaces)
        .map_err(CliError::msg)?;
    let default_output = component_path(workspace, profile, &config.project.name);
    let output = opts.output.unwrap_or(&default_output);
    write_output(output, &component.bytes, label)?;
    write_beside(output, &component.beside)
}

/// The libraries' components the program imports from, in `components/`
/// beside it, each named after its library: the runtime runs them beside
/// the program and links each import to the one that exports it. The
/// folder holds those of the last build only, so that none it no longer
/// imports from is run.
pub(crate) fn write_beside(
    output: &std::path::Path,
    beside: &[crate::component::Carried],
) -> CliResult<()> {
    let folder = output
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .join("components");
    let kept: Vec<std::path::PathBuf> = beside
        .iter()
        .map(|carried| folder.join(format!("{}.wasm", carried.library)))
        .collect();
    if folder.is_dir() {
        for entry in std::fs::read_dir(&folder)
            .map_err(|e| CliError::msg(format!("reading {}: {e}", folder.display())))?
        {
            let path = entry
                .map_err(|e| CliError::msg(format!("reading {}: {e}", folder.display())))?
                .path();
            if path.extension().is_some_and(|ext| ext == "wasm") && !kept.contains(&path) {
                std::fs::remove_file(&path)
                    .map_err(|e| CliError::msg(format!("removing {}: {e}", path.display())))?;
            }
        }
        if beside.is_empty() && std::fs::read_dir(&folder).is_ok_and(|mut d| d.next().is_none()) {
            let _ = std::fs::remove_dir(&folder);
        }
    }
    for (carried, to) in beside.iter().zip(&kept) {
        std::fs::create_dir_all(&folder)
            .map_err(|e| CliError::msg(format!("creating directory {}: {e}", folder.display())))?;
        std::fs::copy(&carried.path, to).map_err(|e| {
            CliError::msg(format!(
                "copying {} to {}: {e}",
                carried.path.display(),
                to.display()
            ))
        })?;
        ui::detail(format!("    beside it: {}", to.display()));
    }
    Ok(())
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
