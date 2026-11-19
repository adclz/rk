// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! `rk bindgen`: the ST declarations of the interfaces a library calls, as
//! rk-bindgen writes them, into its `generated/` folder. By default it
//! reads the library's component, the one `.wasm` of its `component/`
//! folder, whose binary carries its types: what the component exports is
//! what the program imports (`rk_bindgen::component_exports`). With no
//! component it reads the workspace's `wit/` folder, a host's package as
//! the host ships it (`rk_bindgen::resolve_folder`).

use std::path::{Path, PathBuf};

use rk_bindgen::Reach;

use crate::cli::OutputFormat;
use crate::error::{CliError, CliResult};

/// What `rk bindgen` is asked for.
pub struct BindgenOptions {
    /// The WIT folder. When none is given, the workspace's component, or
    /// its `wit/` folder when it carries none.
    pub wit: Option<PathBuf>,
    /// Where the declarations go, `generated/` of the workspace when none
    /// is given.
    pub output: Option<PathBuf>,
    /// The world whose imports are written, when the folder's package
    /// declares several.
    pub world: Option<String>,
    /// Declarations every program reaches, where by default only the
    /// library that holds them does.
    pub public: bool,
}

pub fn run_bindgen(
    workspace: &Path,
    options: BindgenOptions,
    format: OutputFormat,
) -> CliResult<()> {
    let folder = options
        .output
        .unwrap_or_else(|| workspace.join("generated"));
    let reach = match options.public {
        true => Reach::Everyone,
        false => Reach::Library,
    };
    // A WIT folder given is what is read; with none, the component first.
    let component = match options.wit {
        None => component_of(workspace)?,
        Some(_) => None,
    };
    let (resolve, world) = match (options.wit, component) {
        (None, Some(component)) => {
            if options.world.is_some() {
                return Err(CliError::msg(
                    "`--world` picks a world of a WIT folder: a component has its own",
                ));
            }
            let bytes = std::fs::read(&component)
                .map_err(|e| CliError::msg(format!("reading {}: {e}", component.display())))?;
            rk_bindgen::component_exports(&bytes)
                .map_err(|e| CliError::msg(format!("{}: {e}", component.display())))?
        }
        (wit, _) => {
            let wit = wit.unwrap_or_else(|| workspace.join("wit"));
            rk_bindgen::resolve_folder(&wit, options.world.as_deref()).map_err(CliError::Message)?
        }
    };
    let files = rk_bindgen::generate_reaching(&resolve, world, reach).map_err(CliError::Message)?;
    let removed = rk_bindgen::write(&folder, &files).map_err(CliError::Message)?;

    if format == OutputFormat::JsonLines {
        for file in &files {
            let left_out: Vec<serde_json::Value> = file
                .skipped
                .iter()
                .map(
                    |skipped| serde_json::json!({ "item": skipped.item, "reason": skipped.reason }),
                )
                .collect();
            println!(
                "{}",
                serde_json::json!({ "type": "written", "file": file.name, "left_out": left_out })
            );
        }
        for name in &removed {
            println!("{}", serde_json::json!({ "type": "removed", "file": name }));
        }
        return Ok(());
    }
    for file in &files {
        for skipped in &file.skipped {
            println!(
                "{}: left out {}: {}",
                file.name, skipped.item, skipped.reason
            );
        }
    }
    for name in &removed {
        println!("{name}: removed, no longer generated");
    }
    let shown = folder.canonicalize().unwrap_or_else(|_| folder.clone());
    println!("{} files in {}", files.len(), shown.display());
    Ok(())
}

/// The component `workspace` carries: the one `.wasm` of its `component/`
/// folder, or `None` with no such folder. A folder that holds no `.wasm`, or
/// more than one, is refused: a library carries one component.
fn component_of(workspace: &Path) -> CliResult<Option<PathBuf>> {
    let folder = workspace.join("component");
    if !folder.is_dir() {
        return Ok(None);
    }
    let mut found: Vec<PathBuf> = std::fs::read_dir(&folder)
        .map_err(|e| CliError::msg(format!("reading {}: {e}", folder.display())))?
        .filter_map(|entry| Some(entry.ok()?.path()))
        .filter(|path| path.is_file() && path.extension().is_some_and(|ext| ext == "wasm"))
        .collect();
    found.sort();
    match found.len() {
        1 => Ok(found.pop()),
        0 => Err(CliError::msg(format!(
            "{} holds no `.wasm`: a library carries one component, its `.wasm` and its LICENSE",
            folder.display()
        ))),
        n => Err(CliError::msg(format!(
            "{} holds {n} `.wasm` files: a library carries one component",
            folder.display()
        ))),
    }
}
