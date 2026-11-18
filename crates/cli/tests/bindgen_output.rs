// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! What `rk bindgen` writes: the ST declarations of the interfaces a
//! library's `wit/` folder declares, in its `generated/` folder.

use std::path::Path;
use std::process::Command;

fn rk(args: &[&str], ws: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_rk"))
        .arg("--workspace")
        .arg(ws)
        .args(args)
        .env("RK_STDLIB_PATH", "")
        .env("RK_HOME", ws.join("home"))
        .output()
        .expect("run rk")
}

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().expect("a folder")).unwrap();
    std::fs::write(path, text).unwrap();
}

/// A library that calls a host's drive: the drive's package is in its
/// `wit/`, as the host ships it, with no world written for it.
fn library() -> tempfile::TempDir {
    let library = tempfile::tempdir().unwrap();
    let root = library.path();
    write(
        &root.join("config.toml"),
        "[project]\nname = \"drives\"\nversion = \"0.1.0\"\n",
    );
    write(
        &root.join("wit/axis.wit"),
        "package acme:drive@1.0.0;\n\ninterface axis {\n  position: func() -> u32;\n}\n",
    );
    library
}

fn succeeded(out: &std::process::Output) -> String {
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// By default the interfaces of the workspace's `wit/` go into its
/// `generated/`, INTERNAL; `--public` writes them for every program, and
/// `--wit` and `--output` name other folders.
#[test]
fn rk_bindgen_writes_a_library_s_declarations() {
    let library = library();
    let root = library.path();
    let stdout = succeeded(&rk(&["bindgen"], root));
    assert!(stdout.contains("1 files in "), "{stdout}");
    let generated = std::fs::read_to_string(root.join("generated/acme-drive-axis.st"))
        .expect("the axis's declarations");
    assert!(
        generated.starts_with("/* Generated from the WIT of acme:drive@1.0.0. DO NOT EDIT! */"),
        "{generated}"
    );
    assert!(generated.contains("NAMESPACE INTERNAL Acme"), "{generated}");
    assert!(
        generated.contains("{extern 'acme:drive/axis@1.0.0' 'position'}"),
        "{generated}"
    );

    let elsewhere = root.join("bindings");
    let wit = root.join("wit");
    let args = [
        "bindgen",
        "--public",
        "--wit",
        wit.to_str().unwrap(),
        "-o",
        elsewhere.to_str().unwrap(),
    ];
    succeeded(&rk(&args, root));
    let public = std::fs::read_to_string(elsewhere.join("acme-drive-axis.st"))
        .expect("the public declarations");
    assert!(
        public.contains("NAMESPACE Acme") && !public.contains("INTERNAL"),
        "{public}"
    );
}

/// A `.st` file rk bindgen wrote before and no longer writes is removed;
/// a file of the library's own, in the same folder, is left as it is.
#[test]
fn only_what_rk_bindgen_wrote_is_removed() {
    let library = library();
    let root = library.path();
    let own = "PROGRAM Mine\nEND_PROGRAM\n";
    write(&root.join("generated/mine.st"), own);
    write(
        &root.join("generated/acme-old-gone.st"),
        "/* Generated from the WIT of acme:old@1.0.0. DO NOT EDIT! */\n",
    );
    let stdout = succeeded(&rk(&["bindgen", "--output-format", "json-lines"], root));
    let lines: Vec<serde_json::Value> = stdout
        .lines()
        .map(|line| serde_json::from_str(line).expect("a JSON line"))
        .collect();
    assert_eq!(
        lines,
        [
            serde_json::json!({ "type": "written", "file": "acme-drive-axis.st", "left_out": [] }),
            serde_json::json!({ "type": "removed", "file": "acme-old-gone.st" }),
        ]
    );
    assert_eq!(
        std::fs::read_to_string(root.join("generated/mine.st")).unwrap(),
        own
    );
    assert!(!root.join("generated/acme-old-gone.st").exists());
    assert!(root.join("generated/acme-drive-axis.st").is_file());
}

/// A workspace with no `wit/` folder is refused, the folder named.
#[test]
fn a_missing_wit_folder_is_refused() {
    let workspace = tempfile::tempdir().unwrap();
    let out = rk(&["bindgen"], workspace.path());
    assert!(!out.status.success(), "refused");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains(&format!(
            "reading the WIT in `{}`",
            workspace.path().join("wit").display()
        )),
        "{stderr}"
    );
    assert!(
        !workspace.path().join("generated").exists(),
        "nothing written"
    );
}
