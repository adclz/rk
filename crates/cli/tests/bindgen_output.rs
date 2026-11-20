// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! What `rk bindgen` writes: the ST declarations of the interfaces a
//! library's `wit/` folder declares, or a WIT folder `--wit` names, in its
//! `generated/` folder.

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

/// The host's drive package, with no world written for it.
const AXIS: &str = "package acme:drive@1.0.0;\n\ninterface axis {\n  position: func() -> u32;\n}\n";

/// A library that calls a host's drive: the drive's package is in its
/// `wit/`, as the host ships it.
fn library() -> tempfile::TempDir {
    let library = tempfile::tempdir().unwrap();
    let root = library.path();
    write(
        &root.join("config.toml"),
        "[project]\nname = \"drives\"\nversion = \"0.1.0\"\ntype = \"library\"\n",
    );
    write(&root.join("wit/axis.wit"), AXIS);
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

/// By default the interfaces of the library's `wit/` go into its
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
    let vendor = tempfile::tempdir().unwrap();
    let wit = vendor.path().join("wit");
    write(&wit.join("axis.wit"), AXIS);
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

/// `rk bindgen --help` says what the command reads: in a library, its
/// component and its `wit/`, and otherwise the WIT folder `--wit` names.
/// The help is wrapped at the terminal's width, so words are compared, not
/// lines.
#[test]
fn rk_bindgen_s_help_says_what_it_reads() {
    let words = |text: &str| text.split_whitespace().collect::<Vec<_>>().join(" ");
    let workspace = tempfile::tempdir().unwrap();
    let help = words(&succeeded(&rk(&["bindgen", "--help"], workspace.path())));
    for said in [
        "Write the ST declarations of a library's component and of its host's interfaces",
        "In a library, by default: what the component in `component/` exports, and every \
         interface of the package in `wit/`",
        "A program names a WIT folder with `--wit`.",
        "A WIT folder to read instead of the library's component and `wit/`",
        "each a folder, a `.wit` file or a binary `.wasm` package",
    ] {
        assert!(help.contains(&words(said)), "{said}\n\n{help}");
    }
}

/// A program calls a host's interfaces through a library: `rk bindgen` in a
/// program names a WIT folder, and one that is not there is refused, the
/// folder named.
#[test]
fn a_program_names_a_wit_folder() {
    let workspace = tempfile::tempdir().unwrap();
    let out = rk(&["bindgen"], workspace.path());
    assert!(!out.status.success(), "refused");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains(
            "a program calls a host's interfaces through a library: run `rk bindgen` in the \
             library's folder, or name a WIT folder with `--wit <dir>`"
        ),
        "{stderr}"
    );
    let missing = workspace.path().join("wit");
    let out = rk(
        &["bindgen", "--wit", missing.to_str().unwrap()],
        workspace.path(),
    );
    assert!(!out.status.success(), "refused");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains(&format!("reading the WIT in `{}`", missing.display())),
        "{stderr}"
    );
    assert!(
        !workspace.path().join("generated").exists(),
        "nothing written"
    );
}
