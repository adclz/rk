// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! What `rk clean` deletes: the workspace's build folder in the cache, and
//! nothing else.

use std::path::{Path, PathBuf};
use std::process::Command;

fn rk(args: &[&str], ws: &Path, cache: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_rk"))
        .arg("--workspace")
        .arg(ws)
        .args(args)
        .env("RK_STDLIB_PATH", "")
        .env("RK_CACHE_DIR", cache)
        .output()
        .expect("run rk")
}

/// A workspace named `name` under `root`, with one test.
fn workspace(root: &Path, name: &str) -> PathBuf {
    let ws = root.join(name);
    std::fs::create_dir_all(&ws).unwrap();
    std::fs::write(
        ws.join("config.toml"),
        format!("[project]\nname = \"{name}\"\nversion = \"0.1.0\"\n"),
    )
    .unwrap();
    std::fs::write(
        ws.join("main.st"),
        "{test}\nFUNCTION test_passes\nEND_FUNCTION\n",
    )
    .unwrap();
    ws
}

fn succeeded(out: std::process::Output) -> String {
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// `rk clean` deletes what was built for the workspace, and says how much.
/// The workspace, another workspace's builds and the libraries the cache
/// holds stay. Run again, it finds nothing built.
#[test]
fn rk_clean_deletes_the_workspace_s_build_folder() {
    let root = tempfile::tempdir().unwrap();
    let cache = root.path().join("cache");
    let library = cache.join("p").join("library");
    std::fs::create_dir_all(&library).unwrap();
    let plant = workspace(root.path(), "plant");
    let press = workspace(root.path(), "press");
    for ws in [&plant, &press] {
        succeeded(rk(&["test"], ws, &cache));
    }
    // The component `rk test` ran, and the file that names the workspace.
    let built = db::cache::build_dir(&cache, &plant);
    let component = built.join("test").join("plant.wasm");
    let bytes = std::fs::metadata(&component).unwrap().len()
        + std::fs::metadata(built.join("workspace")).unwrap().len();

    let stdout = succeeded(rk(&["clean"], &plant, &cache));
    assert!(
        stdout.contains(&format!(
            "cleaned: {} (2 files, {bytes} bytes)",
            built.display()
        )),
        "{stdout}"
    );
    assert!(!built.exists());
    let mut held: Vec<String> = std::fs::read_dir(&plant)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    held.sort();
    assert_eq!(held, ["config.toml", "main.st"]);
    assert!(
        db::cache::build_dir(&cache, &press)
            .join("test")
            .join("press.wasm")
            .is_file(),
        "another workspace's builds stay"
    );
    assert!(library.is_dir(), "the libraries stay");

    let stdout = succeeded(rk(&["clean"], &plant, &cache));
    assert!(
        stdout.contains("cleaned: nothing was built for this workspace"),
        "{stdout}"
    );
}
