// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! Lists the files of `stdlib/` for `src/stdlib.rs`, which builds them into
//! the compiler: every file under it but the hidden ones and the build
//! folder, by its path in the library.

use std::path::{Path, PathBuf};

fn main() {
    let stdlib = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../stdlib");
    let stdlib = stdlib.canonicalize().unwrap_or(stdlib);
    let mut files = Vec::new();
    collect(&stdlib, &stdlib, &mut files);
    files.sort();

    // A file added or removed changes the folder's date; a file changed is
    // seen through its `include_bytes!`.
    println!("cargo:rerun-if-changed={}", stdlib.display());
    let mut code = String::from("&[\n");
    for (path, file) in &files {
        code.push_str(&format!(
            "    ({path:?}, include_bytes!({:?})),\n",
            file.display().to_string()
        ));
    }
    code.push_str("]\n");
    let out = PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("stdlib_files.rs");
    std::fs::write(out, code).unwrap();
}

fn collect(root: &Path, dir: &Path, files: &mut Vec<(String, PathBuf)>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') || name == "rk_build" {
            continue;
        }
        let path = entry.path();
        if path.is_dir() {
            println!("cargo:rerun-if-changed={}", path.display());
            collect(root, &path, files);
        } else {
            let relative = path.strip_prefix(root).unwrap();
            let relative = relative.to_string_lossy().replace('\\', "/");
            files.push((relative, path));
        }
    }
}
