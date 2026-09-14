//! Reporting for `rk test`.
//!
//! Execution is [`crate::test_host`]'s: a module's `{test}` functions run on
//! a wasmtime host inside this process, and what is here is presentation —
//! three output shapes, one of which (`json-lines`) other programs parse. The
//! records are the ones a test run reports across a process boundary
//! ([`debug_format::test_report`]), so a tool that runs the same module
//! elsewhere renders its results through the same functions.
//!
//! Results are rendered as they happen: a suite is streamed, so a failure
//! appears when it occurs rather than when the run ends.

use std::path::Path;
use std::time::Duration;

use debug_format::test_report::{ReportLine, Status, Summary, TestRecord};
use yansi::Paint;

use crate::cli::OutputFormat;
use crate::ui;

/// `file:line` suffix for a test, empty when the manifest has no position.
fn loc_suffix(record: &TestRecord) -> String {
    match (&record.file, record.line) {
        (Some(file), Some(line)) if !file.is_empty() && line > 0 => format!(" ({file}:{line})"),
        _ => String::new(),
    }
}

fn fmt_duration(d: Duration) -> String {
    let us = d.as_micros();
    if us < 1_000 {
        format!("{us}µs")
    } else if us < 1_000_000 {
        format!("{:.1}ms", us as f64 / 1_000.0)
    } else {
        format!("{:.2}s", us as f64 / 1_000_000.0)
    }
}

/// Run a compiled module's tests and report them. Returns the failure count.
pub fn run_tests(
    wasm_path: &Path,
    filter: Option<&str>,
    timeout: Option<Duration>,
    format: OutputFormat,
) -> usize {
    let wasm = match std::fs::read(wasm_path) {
        Ok(bytes) => bytes,
        Err(e) => {
            ui::error(format!("reading {}: {e}", wasm_path.display()));
            return 1;
        }
    };
    announce(wasm_path, format);

    let total_start = std::time::Instant::now();
    let mut failures = Vec::new();
    let mut passed = 0;
    let run = crate::test_host::run_each(&wasm, filter, timeout, |record| {
        if record.status == Status::Fail {
            failures.push(record.clone());
        } else {
            passed += 1;
        }
        report_one(record, format);
    });
    let total_elapsed = total_start.elapsed();

    match run {
        Ok(_) => summarize(
            Summary {
                total: passed + failures.len(),
                passed,
                failed: failures.len(),
            },
            &failures,
            total_elapsed,
            format,
        ),
        Err(e) => {
            ui::error(format!("the test run did not finish: {e:#}"));
            1
        }
    }
}

/// Say what is about to run, in the human format only.
pub fn announce(wasm_path: &Path, format: OutputFormat) {
    if format == OutputFormat::Full {
        println!("{}  {}", "    Running".dim(), wasm_path.display());
    }
}

/// The recap once every test has been reported. Returns the failure count;
/// a run with no tests counts as one failure, not as a green build.
pub fn summarize(
    summary: Summary,
    failures: &[TestRecord],
    total_elapsed: Duration,
    format: OutputFormat,
) -> usize {
    if summary.total == 0 {
        if format == OutputFormat::JsonLines {
            print_json(&ReportLine::Summary(summary));
        } else {
            println!("No test functions found");
        }
        return 1;
    }

    match format {
        OutputFormat::Full => {
            let rule = "────────────────────────────────────────────────────────────";
            println!("{}", rule.dim());
            if !failures.is_empty() {
                println!("     {}:", "Failures".bold().red());
                for f in failures {
                    println!(
                        "        {} {}{}: {}",
                        "FAIL".red(),
                        f.name,
                        loc_suffix(f).dim(),
                        f.reason.as_deref().unwrap_or_default().bold()
                    );
                }
                println!("{}", rule.dim());
            }
            let passed = if summary.failed == 0 {
                format!("{} passed", summary.passed).green().to_string()
            } else {
                format!("{} passed", summary.passed)
            };
            let failed = if summary.failed == 0 {
                format!("{} failed", summary.failed).green().to_string()
            } else {
                format!("{} failed", summary.failed).red().to_string()
            };
            println!(
                "    {} {} {} tests run: {}, {}",
                "Summary".dim(),
                format!("[{:>7}]", fmt_duration(total_elapsed)).dim(),
                summary.total,
                passed,
                failed,
            );
        }
        // Failure reasons were already inline on the FAIL lines.
        OutputFormat::Concise => {
            println!(
                "summary: {} run, {} passed, {} failed",
                summary.total, summary.passed, summary.failed
            );
        }
        OutputFormat::JsonLines => print_json(&ReportLine::Summary(summary)),
    }

    summary.failed
}

/// Print one NDJSON record to stdout.
pub fn print_json<T: serde::Serialize>(record: &T) {
    if let Ok(json) = serde_json::to_string(record) {
        println!("{json}");
    }
}

/// Render one finished test.
pub fn report_one(r: &TestRecord, format: OutputFormat) {
    let name = &r.name;
    match format {
        OutputFormat::Full => {
            let tag = if r.passed() {
                "PASS".green().to_string()
            } else {
                "FAIL".red().to_string()
            };
            println!(
                "        {} {} {}",
                tag,
                format!(
                    "[{:>7}]",
                    fmt_duration(Duration::from_micros(r.duration_us))
                )
                .dim(),
                name,
            );
        }
        // Reason inline, no durations/decoration — line-stable for agents.
        OutputFormat::Concise => match &r.reason {
            None => println!("PASS {name}"),
            Some(reason) => println!(
                "FAIL {name}{}: {}",
                loc_suffix(r),
                reason.replace('\n', " ")
            ),
        },
        // The report's own shape, forwarded unchanged.
        OutputFormat::JsonLines => print_json(&ReportLine::Test(r.clone())),
    }
}
