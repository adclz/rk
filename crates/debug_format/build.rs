//! Lists the WASI packages rk carries, from WASI's own repository: the git
//! submodule at `wit/wasi`, checked out at the release the standard library
//! imports. A package's name, version and files are what its files say;
//! only which proposals rk carries is written here.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// The WASI proposals rk carries, by their folder under `proposals/`.
const PROPOSALS: &[&str] = &["clocks"];

fn main() {
    let manifest = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("set by cargo"));
    let wasi = manifest.join("wit").join("wasi");
    let license = wasi.join("LICENSE.md");
    println!("cargo:rerun-if-changed={}", license.display());
    assert!(
        license.is_file(),
        "{} is empty: it is WASI's repository, a git submodule, which \
         `git submodule update --init` checks out",
        wasi.display()
    );

    let mut table = String::from("&[\n");
    for proposal in PROPOSALS {
        let dir = wasi.join("proposals").join(proposal).join("wit");
        println!("cargo:rerun-if-changed={}", dir.display());
        let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
            .unwrap_or_else(|e| panic!("reading {}: {e}", dir.display()))
            .map(|entry| entry.expect("a directory entry").path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "wit"))
            .collect();
        files.sort();
        let id = declared_package(&files);
        let name = id.split_once('@').map_or(id.as_str(), |(name, _)| name);
        writeln!(table, "    Package {{").unwrap();
        writeln!(table, "        id: {id:?},").unwrap();
        writeln!(table, "        dir: {:?},", name.replace(':', "-")).unwrap();
        writeln!(table, "        files: &[").unwrap();
        for file in &files {
            let file_name = file
                .file_name()
                .and_then(|name| name.to_str())
                .expect("a file name in UTF-8");
            writeln!(
                table,
                "            ({file_name:?}, include_str!({:?})),",
                text(file)
            )
            .unwrap();
        }
        writeln!(table, "        ],").unwrap();
        writeln!(
            table,
            "        license: Some(include_str!({:?})),",
            text(&license)
        )
        .unwrap();
        writeln!(table, "    }},").unwrap();
    }
    table.push(']');

    let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("set by cargo")).join("wasi.rs");
    std::fs::write(&out, table).unwrap_or_else(|e| panic!("writing {}: {e}", out.display()));
}

/// The package each of `files` declares, with its version, the same in
/// every file: `wasi:clocks@0.3.0`.
fn declared_package(files: &[PathBuf]) -> String {
    let mut package: Option<String> = None;
    for file in files {
        let source = std::fs::read_to_string(file)
            .unwrap_or_else(|e| panic!("reading {}: {e}", file.display()));
        let declared = source
            .lines()
            .find_map(|line| line.trim().strip_prefix("package ")?.strip_suffix(';'))
            .unwrap_or_else(|| panic!("{} declares no package", file.display()));
        match &package {
            None => package = Some(declared.to_string()),
            Some(first) => assert_eq!(
                first,
                declared,
                "{} declares another package than its folder's other files",
                file.display()
            ),
        }
    }
    package.expect("a proposal with WIT files")
}

/// `path` as `include_str!` takes it.
fn text(path: &Path) -> String {
    path.to_str().expect("a path in UTF-8").to_string()
}
