use std::path::Path;

use formatter::TOPIARY_LANG;
use topiary_core::{Operation, formatter};
use yansi::Paint;

use crate::error::{CliError, CliResult};
use crate::ui;

pub fn run_fmt(workspace: &Path, check: bool, verbose: bool) -> CliResult<()> {
    let workspace = std::fs::canonicalize(workspace).map_err(CliError::msg)?;

    let st_files = collect_st_files(&workspace);
    if st_files.is_empty() {
        return Err(CliError::msg(format!(
            "no .st files found in {}",
            workspace.display()
        )));
    }

    if verbose {
        ui::detail(format!("found {} .st file(s)", st_files.len()));
    }

    let mut formatted = 0;
    let mut unchanged = 0;
    let mut errored = 0;

    for path in &st_files {
        let source = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                report_file_error(path, &workspace, &e);
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
                report_file_error(path, &workspace, &e);
                errored += 1;
            }
            Ok(()) => {
                let output = match String::from_utf8(output) {
                    Ok(s) => s,
                    Err(e) => {
                        report_file_error(path, &workspace, &e);
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
                } else if let Err(e) = std::fs::write(path, &output) {
                    report_file_error(path, &workspace, &e);
                    errored += 1;
                } else {
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

    println!();
    if check {
        ui::success(
            "fmt check:",
            format!(
                "{} file(s) checked: {} unformatted, {} unchanged, {} error(s).",
                st_files.len(),
                formatted,
                unchanged,
                errored,
            ),
        );
        if formatted > 0 || errored > 0 {
            return Err(CliError::Failed);
        }
    } else {
        ui::success(
            "fmt complete:",
            format!(
                "{} file(s) checked: {} formatted, {} unchanged, {} error(s).",
                st_files.len(),
                formatted,
                unchanged,
                errored,
            ),
        );
        if errored > 0 {
            return Err(CliError::Failed);
        }
    }
    Ok(())
}

/// A per-file `  error <path> — <reason>` line on stderr.
fn report_file_error(path: &Path, base: &Path, err: &dyn std::fmt::Display) {
    eprintln!("  {} {} — {}", "error".bold().red(), relative(path, base), err);
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
