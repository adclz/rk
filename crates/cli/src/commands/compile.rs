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
    // component, with the WIT that types it.
    let component =
        crate::component::wrap(&core_bytes, &config.project.name).map_err(CliError::msg)?;
    let default_output = component_path(workspace, profile, &config.project.name);
    let output = opts.output.unwrap_or(&default_output);
    write_output(output, &component.bytes, label)?;
    write_wit(output, &component)
}

/// The component's WIT folder, `wit/` beside `output`: `world.wit`, and
/// each package it names under `deps/`. A package's folder is written
/// whole, so a file an earlier build left in it goes; nothing else in the
/// folder is touched.
fn write_wit(output: &std::path::Path, component: &crate::component::Component) -> CliResult<()> {
    let dir = output
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .join("wit");
    let packages: std::collections::BTreeSet<_> = component
        .wit
        .iter()
        .filter_map(|(path, _)| std::path::Path::new(path).parent())
        .collect();
    for package in packages {
        let package = dir.join(package);
        match std::fs::remove_dir_all(&package) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                return Err(CliError::msg(format!(
                    "removing {}: {e}",
                    package.display()
                )));
            }
            _ => {}
        }
    }
    let files = std::iter::once(("world.wit".to_string(), component.world.as_str())).chain(
        component
            .wit
            .iter()
            .map(|(path, text)| (path.clone(), *text)),
    );
    for (path, text) in files {
        let path = dir.join(path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                CliError::msg(format!("creating directory {}: {e}", parent.display()))
            })?;
        }
        std::fs::write(&path, text)
            .map_err(|e| CliError::msg(format!("writing {}: {e}", path.display())))?;
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
