// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! `rk bindgen`: the ST declarations of the interfaces a library calls, as
//! rk-bindgen writes them, into its `generated/` folder. By default it
//! reads both of a library's folders: its component, the one `.wasm` of its
//! `component/`, whose binary carries its types, what it exports being what
//! the program imports (`rk_bindgen::component_exports`); and its `wit/`,
//! the host's packages as the host ships them, text or binary
//! (`rk_bindgen::resolve_folder`). `--wit` reads the WIT folder it names
//! instead. A program carries neither: it calls a host's interfaces through
//! a library.

use std::path::{Path, PathBuf};

use rk_bindgen::Reach;

use crate::cli::OutputFormat;
use crate::error::{CliError, CliResult};

/// What `rk bindgen` is asked for.
pub struct BindgenOptions {
    /// The WIT folder. When none is given, the library's component and its
    /// `wit/` folder.
    pub wit: Option<PathBuf>,
    /// Where the declarations go, `generated/` of the workspace when none
    /// is given.
    pub output: Option<PathBuf>,
    /// The world whose imports are written, when the package of the WIT
    /// folder declares several.
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
    let world = options.world.as_deref();
    let files = match options.wit {
        Some(wit) => {
            let (resolve, world) =
                rk_bindgen::resolve_folder(&wit, world).map_err(CliError::Message)?;
            rk_bindgen::generate_reaching(&resolve, world, reach).map_err(CliError::Message)?
        }
        None if is_library(workspace) => library_files(workspace, world, reach)?,
        None => {
            return Err(CliError::msg(
                "a program calls a host's interfaces through a library: run `rk bindgen` in \
                 the library's folder, or name a WIT folder with `--wit <dir>`",
            ));
        }
    };
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

/// What the library at `workspace` gives its programs: what its component
/// exports, and the interfaces its `wit/` declares, both when it carries
/// both. `world` picks a world of the package in `wit/`.
fn library_files(
    workspace: &Path,
    world: Option<&str>,
    reach: Reach,
) -> CliResult<Vec<rk_bindgen::File>> {
    let component = crate::component::carried_component(workspace).map_err(CliError::Message)?;
    let wit = workspace.join("wit");
    if component.is_none() && !wit.is_dir() {
        return Err(CliError::msg(
            "this library carries no component in `component/` and no WIT in `wit/`",
        ));
    }
    if world.is_some() && !wit.is_dir() {
        return Err(CliError::msg(
            "`--world` picks a world of a WIT folder: a component has its own",
        ));
    }
    let mut files = Vec::new();
    if let Some(component) = component {
        let bytes = std::fs::read(&component)
            .map_err(|e| CliError::msg(format!("reading {}: {e}", component.display())))?;
        let (resolve, exports) = rk_bindgen::component_exports(&bytes)
            .map_err(|e| CliError::msg(format!("{}: {e}", component.display())))?;
        files.extend(
            rk_bindgen::generate_reaching(&resolve, exports, reach).map_err(CliError::Message)?,
        );
    }
    if wit.is_dir() {
        let (resolve, world) =
            rk_bindgen::resolve_folder(&wit, world).map_err(CliError::Message)?;
        let declared =
            rk_bindgen::generate_reaching(&resolve, world, reach).map_err(CliError::Message)?;
        if let Some(twice) = declared.iter().find(|file| {
            files
                .iter()
                .any(|other: &rk_bindgen::File| other.name == file.name)
        }) {
            return Err(CliError::msg(format!(
                "`{}` is written from the component and from `wit/`: an interface is the \
                 component's or the host's",
                twice.name
            )));
        }
        files.extend(declared);
    }
    Ok(files)
}

/// Whether the `config.toml` of `workspace` says it is a library.
fn is_library(workspace: &Path) -> bool {
    std::fs::read_to_string(workspace.join(db::config_file::CONFIG_FILE))
        .ok()
        .and_then(|text| db::config_file::parse_config(&text).ok())
        .is_some_and(|config| config.project.kind == db::config_file::ProjectKind::Library)
}
