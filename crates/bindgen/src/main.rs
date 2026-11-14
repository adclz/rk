// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! Writes `stdlib/generated/`: the ST declarations of the WASI interfaces
//! `Std` reaches WASI through ([`rk_bindgen::STD_WORLD`]), one file per
//! interface, then the built-ins of their streams and futures. A file that
//! is no longer generated is removed. `cargo run -p rk-bindgen [folder]`
//! writes another folder instead.

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let folder = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../stdlib/generated")
        });
    match write(&folder) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("rk-bindgen: {message}");
            ExitCode::FAILURE
        }
    }
}

fn write(folder: &PathBuf) -> Result<(), String> {
    let (resolve, world) = rk_bindgen::resolve(rk_bindgen::STD_WORLD)?;
    let files = rk_bindgen::generate(&resolve, world)?;
    std::fs::create_dir_all(folder).map_err(|e| format!("making `{}`: {e}", folder.display()))?;
    let entries =
        std::fs::read_dir(folder).map_err(|e| format!("reading `{}`: {e}", folder.display()))?;
    for entry in entries.flatten() {
        let path = entry.path();
        let stale = path.extension().is_some_and(|extension| extension == "st")
            && !files.iter().any(|file| {
                path.file_name()
                    .is_some_and(|name| name == file.name.as_str())
            });
        if stale {
            std::fs::remove_file(&path)
                .map_err(|e| format!("removing `{}`: {e}", path.display()))?;
        }
    }
    for file in &files {
        let path = folder.join(&file.name);
        std::fs::write(&path, &file.text)
            .map_err(|e| format!("writing `{}`: {e}", path.display()))?;
        for skipped in &file.skipped {
            println!(
                "{}: left out {}: {}",
                file.name, skipped.item, skipped.reason
            );
        }
    }
    let shown = folder.canonicalize().unwrap_or_else(|_| folder.clone());
    println!("{} files in {}", files.len(), shown.display());
    Ok(())
}
