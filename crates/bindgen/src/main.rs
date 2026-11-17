// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! Writes the ST declarations of the interfaces a WIT world imports, one
//! file per interface, then the built-ins of their streams and futures. A
//! `.st` file of the folder that is no longer generated is removed.
//!
//! `cargo run -p rk-bindgen` writes `stdlib/generated/`, for the world Std
//! reaches WASI through ([`rk_bindgen::STD_WORLD`]); a folder given as an
//! argument is written instead. `--wit <dir>` reads the world of a WIT
//! folder, the packages it uses in its `deps/`, as WIT's tools lay them
//! out, and then takes the folder to write; `--world <name>` picks the
//! world when the package has several, and `--public` writes declarations
//! every program reaches, where Std's are INTERNAL.

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;

use rk_bindgen::Reach;

/// What the command line asks for.
#[derive(Debug, PartialEq)]
struct Args {
    /// The WIT folder to read, or `None` for Std's world.
    wit: Option<PathBuf>,
    /// The world of the folder's package, when it has several.
    world: Option<String>,
    reach: Reach,
    /// Where the files go.
    folder: PathBuf,
}

fn main() -> ExitCode {
    match parse(std::env::args_os().skip(1)).and_then(|args| write(&args)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("rk-bindgen: {message}");
            ExitCode::FAILURE
        }
    }
}

/// The arguments after the program's name.
fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Args, String> {
    let mut wit = None;
    let mut world = None;
    let mut reach = Reach::Library;
    let mut folder = None;
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--wit") => {
                wit = Some(PathBuf::from(
                    args.next().ok_or("`--wit` takes a WIT folder")?,
                ));
            }
            Some("--world") => {
                world = Some(
                    args.next()
                        .and_then(|name| name.into_string().ok())
                        .ok_or("`--world` takes the name of a world")?,
                );
            }
            Some("--public") => reach = Reach::Everyone,
            Some(option) if option.starts_with("--") => {
                return Err(format!("`{option}` is no option of rk-bindgen"));
            }
            _ if folder.is_none() => folder = Some(PathBuf::from(arg)),
            _ => return Err("one folder to write, at most".to_string()),
        }
    }
    if wit.is_none() && (world.is_some() || reach == Reach::Everyone) {
        return Err("`--world` and `--public` go with `--wit`: Std's world is its own".to_string());
    }
    let folder = match (folder, &wit) {
        (Some(folder), _) => folder,
        (None, None) => PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../stdlib/generated"),
        // Std's folder would lose its files, as stale ones.
        (None, Some(_)) => return Err("`--wit` takes the folder to write as well".to_string()),
    };
    Ok(Args {
        wit,
        world,
        reach,
        folder,
    })
}

fn write(args: &Args) -> Result<(), String> {
    let (resolve, world) = match &args.wit {
        Some(dir) => rk_bindgen::resolve_folder(dir, args.world.as_deref())?,
        None => rk_bindgen::resolve(rk_bindgen::STD_WORLD)?,
    };
    let files = rk_bindgen::generate_reaching(&resolve, world, args.reach)?;
    let folder = &args.folder;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(args: &[&str]) -> Result<Args, String> {
        parse(args.iter().map(OsString::from))
    }

    #[test]
    fn std_s_world_into_its_folder_or_another() {
        let std = parsed(&[]).unwrap();
        assert_eq!(
            (std.wit, std.world, std.reach),
            (None, None, Reach::Library)
        );
        assert!(std.folder.ends_with("stdlib/generated"), "{:?}", std.folder);
        assert_eq!(parsed(&["out"]).unwrap().folder, PathBuf::from("out"));
    }

    #[test]
    fn a_wit_folder_its_world_and_who_reaches_it() {
        assert_eq!(
            parsed(&["--wit", "wit", "--world", "plc", "--public", "out"]),
            Ok(Args {
                wit: Some(PathBuf::from("wit")),
                world: Some("plc".to_string()),
                reach: Reach::Everyone,
                folder: PathBuf::from("out"),
            })
        );
        assert_eq!(
            parsed(&["--wit", "wit", "out"]).unwrap().reach,
            Reach::Library
        );
    }

    #[test]
    fn what_the_command_line_refuses() {
        for (args, error) in [
            (
                &["--wit", "wit"][..],
                "`--wit` takes the folder to write as well",
            ),
            (&["--wit"], "`--wit` takes a WIT folder"),
            (
                &["--wit", "wit", "--world"],
                "`--world` takes the name of a world",
            ),
            (
                &["--world", "plc", "out"],
                "`--world` and `--public` go with `--wit`: Std's world is its own",
            ),
            (
                &["--public"],
                "`--world` and `--public` go with `--wit`: Std's world is its own",
            ),
            (&["--all", "out"], "`--all` is no option of rk-bindgen"),
            (&["one", "two"], "one folder to write, at most"),
        ] {
            assert_eq!(parsed(args), Err(error.to_string()), "{args:?}");
        }
    }
}
