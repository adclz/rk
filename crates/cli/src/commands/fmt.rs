use std::path::Path;

use formatter::TOPIARY_LANG;
use topiary_core::{Operation, formatter};
use yansi::Paint;

pub fn run_fmt(workspace: &Path, check: bool, verbose: bool) {
    let workspace = std::fs::canonicalize(workspace).unwrap_or_else(|e| {
        eprintln!("{}{}", "error: ".bold().red(), e);
        std::process::exit(1);
    });

    let st_files = collect_st_files(&workspace);

    if st_files.is_empty() {
        eprintln!("no .st files found in {}", workspace.display());
        std::process::exit(1);
    }

    if verbose {
        println!("found {} .st file(s)", st_files.len());
    }

    let mut formatted = 0;
    let mut unchanged = 0;
    let mut errored = 0;

    for path in &st_files {
        let source = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!(
                    "  {} {} — {}",
                    "error".bold().red(),
                    relative(path, &workspace),
                    e
                );
                errored += 1;
                continue;
            }
        };

        let mut output = Vec::new();
        let result = formatter(
            &mut source.as_bytes(),
            &mut output,
            &TOPIARY_LANG,
            Operation::Format {
                skip_idempotence: true,
                tolerate_parsing_errors: false,
            },
        );

        match result {
            Err(e) => {
                eprintln!(
                    "  {} {} — {}",
                    "error".bold().red(),
                    relative(path, &workspace),
                    e
                );
                errored += 1;
            }
            Ok(()) => {
                let output = match String::from_utf8(output) {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!(
                            "  {} {} — {}",
                            "error".bold().red(),
                            relative(path, &workspace),
                            e
                        );
                        errored += 1;
                        continue;
                    }
                };

                if output == source {
                    unchanged += 1;
                    if verbose {
                        println!("  {} {}", "unchanged".dim(), relative(path, &workspace));
                    }
                } else if check {
                    formatted += 1;
                    println!(
                        "  {} {}",
                        "unformatted".bold().yellow(),
                        relative(path, &workspace)
                    );
                } else {
                    if let Err(e) = std::fs::write(path, &output) {
                        eprintln!(
                            "  {} {} — {}",
                            "error".bold().red(),
                            relative(path, &workspace),
                            e
                        );
                        errored += 1;
                        continue;
                    }
                    formatted += 1;
                    println!(
                        "  {} {}",
                        "formatted".bold().green(),
                        relative(path, &workspace)
                    );
                }
            }
        }
    }

    // Summary
    println!();
    if check {
        println!(
            "{}{} file(s) checked: {} unformatted, {} unchanged, {} error(s).",
            "fmt check: ".bold().bright_green(),
            st_files.len(),
            formatted,
            unchanged,
            errored,
        );
        if formatted > 0 || errored > 0 {
            std::process::exit(1);
        }
    } else {
        println!(
            "{}{} file(s) checked: {} formatted, {} unchanged, {} error(s).",
            "fmt complete: ".bold().bright_green(),
            st_files.len(),
            formatted,
            unchanged,
            errored,
        );
        if errored > 0 {
            std::process::exit(1);
        }
    }
}

fn collect_st_files(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    collect_st_files_recursive(dir, &mut files);
    files.sort();
    files
}

fn collect_st_files_recursive(dir: &Path, files: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_st_files_recursive(&path, files);
        } else if path.extension().is_some_and(|ext| ext == "st") {
            files.push(path);
        }
    }
}

fn relative<'a>(path: &'a Path, base: &Path) -> &'a str {
    path.strip_prefix(base)
        .unwrap_or(path)
        .to_str()
        .unwrap_or("?")
}
