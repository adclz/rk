// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! `rk bindgen`: the ST declarations of the interfaces a WIT world imports,
//! as rk-bindgen writes them, for a library that calls a host's interfaces.
//! By default it reads the world of the workspace's `wit/` folder and
//! writes into its `generated/` folder.

use std::path::{Path, PathBuf};

use rk_bindgen::Reach;

use crate::cli::OutputFormat;
use crate::error::{CliError, CliResult};

/// What `rk bindgen` is asked for.
pub struct BindgenOptions {
    /// The WIT folder, `wit/` of the workspace when none is given.
    pub wit: Option<PathBuf>,
    /// Where the declarations go, `generated/` of the workspace when none
    /// is given.
    pub output: Option<PathBuf>,
    /// The world, when the folder's package has several.
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
    let wit = options.wit.unwrap_or_else(|| workspace.join("wit"));
    let folder = options
        .output
        .unwrap_or_else(|| workspace.join("generated"));
    let reach = match options.public {
        true => Reach::Everyone,
        false => Reach::Library,
    };
    let (resolve, world) =
        rk_bindgen::resolve_folder(&wit, options.world.as_deref()).map_err(CliError::Message)?;
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
