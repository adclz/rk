use std::path::Path;

use yansi::Paint;

use crate::cli::OutputFormat;
use crate::error::{CliError, CliResult};
use crate::ui;

/// NDJSON record for one file in [`OutputFormat::JsonLines`]. Field order is
/// part of the wire shape; additions go at the END.
#[derive(serde::Serialize)]
struct FmtRecord<'a> {
    r#type: &'static str,
    file: &'a str,
    status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    line: Option<u32>,
}

/// Terminal NDJSON record: totals for the whole run.
#[derive(serde::Serialize)]
struct FmtSummary {
    r#type: &'static str,
    checked: usize,
    changed: usize,
    unchanged: usize,
    errors: usize,
}

/// One per-file result line per format: the styled human line, a stable
/// `<status>: <file>` line, or one [`FmtRecord`].
fn emit_file(
    format: OutputFormat,
    status: &'static str,
    rel: &str,
    reason: Option<String>,
    line: Option<u32>,
) {
    // `line` (1-based) is the FIRST line where formatting differs — only set
    // for `unformatted`, where "where?" is actionable.
    let loc = line.map(|l| format!(":{l}")).unwrap_or_default();
    match format {
        OutputFormat::Full => match status {
            "unchanged" => println!("  {} {}", "unchanged".dim(), rel),
            "unformatted" => println!("  {} {}{}", "unformatted".bold().yellow(), rel, loc),
            "formatted" => println!("  {} {}", "formatted".bold().green(), rel),
            _ => eprintln!(
                "  {} {}: {}",
                "error".bold().red(),
                rel,
                reason.unwrap_or_default()
            ),
        },
        OutputFormat::Concise => match status {
            "error" => eprintln!("error: {rel}: {}", reason.unwrap_or_default()),
            _ => println!("{status}: {rel}{loc}"),
        },
        OutputFormat::JsonLines => {
            let record = FmtRecord {
                r#type: "fmt",
                file: rel,
                status,
                reason,
                line,
            };
            if let Ok(json) = serde_json::to_string(&record) {
                println!("{json}");
            }
        }
    }
}

/// 1-based line of the first difference between the source and its formatted
/// output (the two are known to differ when this is called).
fn first_diff_line(source: &str, output: &str) -> u32 {
    let mut line = 1;
    let mut a = source.lines();
    let mut b = output.lines();
    loop {
        match (a.next(), b.next()) {
            (Some(x), Some(y)) if x == y => line += 1,
            _ => return line,
        }
    }
}

pub fn run_fmt(workspace: &Path, check: bool, verbose: bool, format: OutputFormat) -> CliResult<()> {
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
        let rel = relative(path, &workspace);
        let source = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                emit_file(format, "error", rel, Some(e.to_string()), None);
                errored += 1;
                continue;
            }
        };

        // The formatter crate owns the refusal of unparseable input, so the
        // CLI and the language server cannot drift on what is left untouched.
        match formatter::format_source(&source) {
            Err(e) => {
                emit_file(format, "error", rel, Some(e.to_string()), None);
                errored += 1;
            }
            Ok(output) => {
                if output == source {
                    unchanged += 1;
                    if verbose {
                        emit_file(format, "unchanged", rel, None, None);
                    }
                } else if check {
                    formatted += 1;
                    emit_file(format, "unformatted", rel, None, Some(first_diff_line(&source, &output)));
                } else if let Err(e) = std::fs::write(path, &output) {
                    emit_file(format, "error", rel, Some(e.to_string()), None);
                    errored += 1;
                } else {
                    formatted += 1;
                    emit_file(format, "formatted", rel, None, None);
                }
            }
        }
    }

    if format == OutputFormat::JsonLines {
        // Stdout is a pure NDJSON stream: the totals ride in the terminal
        // summary record, not a prose line.
        let record = FmtSummary {
            r#type: "summary",
            checked: st_files.len(),
            changed: formatted,
            unchanged,
            errors: errored,
        };
        if let Ok(json) = serde_json::to_string(&record) {
            println!("{json}");
        }
    } else {
        println!();
        let (label, verb) = if check {
            ("fmt check:", "unformatted")
        } else {
            ("fmt complete:", "formatted")
        };
        ui::success(
            label,
            format!(
                "{} file(s) checked: {} {}, {} unchanged, {} error(s).",
                st_files.len(),
                formatted,
                verb,
                unchanged,
                errored,
            ),
        );
    }

    let failed = if check {
        formatted > 0 || errored > 0
    } else {
        errored > 0
    };
    if failed { Err(CliError::Failed) } else { Ok(()) }
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
        } else if db::loader::is_st_file(&path) {
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
