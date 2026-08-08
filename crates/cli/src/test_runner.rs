//! Reporting for `rk test`.
//!
//! Execution belongs to the runtime ([`runtime::test`]) — the same loader a
//! plant runs, so a test cannot pass against a differently-linked copy of the
//! module. What is left here is presentation: three output shapes, one of
//! which (`json-lines`) other programs parse.

use std::time::Duration;

use runtime::test::{Outcome, TestResult};
use yansi::Paint;

use crate::cli::OutputFormat;
use crate::ui;

/// `file:line` suffix for a test, empty when the manifest has no position.
fn loc_suffix(file: &str, line: u32) -> String {
    if file.is_empty() || line == 0 {
        String::new()
    } else {
        format!(" ({file}:{line})")
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

/// The reason a test did not pass, as one line.
fn reason_of(outcome: &Outcome) -> Option<String> {
    match outcome {
        Outcome::Pass => None,
        Outcome::Fail(msg) => Some(msg.clone()),
        // A trap is not an assertion: say so, and say where when the module's
        // debug info can place it, rather than reporting a bare wasm phrase.
        Outcome::Trap { reason, at } => Some(match at {
            Some(at) => format!("trapped at {at}: {reason}"),
            None => format!("trapped: {reason}"),
        }),
    }
}

#[derive(serde::Serialize)]
struct TestRecord<'a> {
    r#type: &'a str,
    name: &'a str,
    status: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<&'a str>,
    duration_us: u128,
    #[serde(skip_serializing_if = "Option::is_none")]
    file: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    line: Option<u32>,
}

#[derive(serde::Serialize)]
struct TestSummary<'a> {
    r#type: &'a str,
    total: usize,
    passed: usize,
    failed: usize,
}

/// Print one NDJSON record to stdout.
fn print_json<T: serde::Serialize>(record: &T) {
    if let Ok(json) = serde_json::to_string(record) {
        println!("{json}");
    }
}

/// Run a compiled module's tests and report them. Returns the failure count.
pub fn run_tests(wasm: &[u8], filter: Option<&str>, format: OutputFormat) -> usize {
    let tests = runtime::test::discover(wasm);
    let selected = match filter {
        Some(f) => {
            let f = f.to_lowercase();
            tests
                .iter()
                .filter(|t| t.path.to_lowercase().contains(&f))
                .count()
        }
        None => tests.len(),
    };

    if selected == 0 {
        // Keep stdout pure NDJSON in json-lines mode — the empty run is
        // expressed by the summary record. (Still non-zero: an empty test run
        // is a failure, not a green build.)
        if format == OutputFormat::JsonLines {
            print_json(&TestSummary {
                r#type: "summary",
                total: 0,
                passed: 0,
                failed: 0,
            });
        } else {
            println!("No test functions found");
        }
        return 1;
    }

    if format == OutputFormat::Full {
        println!("{}  {} test(s)", "    Running".dim(), selected);
    }

    let total_start = std::time::Instant::now();
    let results = match runtime::test::run(wasm, filter) {
        Ok(r) => r,
        Err(e) => {
            ui::error(format!("running tests: {e:#}"));
            return 1;
        }
    };
    let total_elapsed = total_start.elapsed();

    for r in &results {
        report_one(r, format);
    }

    let total = results.len();
    let passed = results.iter().filter(|r| r.passed()).count();
    let failed = total - passed;

    match format {
        OutputFormat::Full => {
            let rule = "────────────────────────────────────────────────────────────";
            println!("{}", rule.dim());
            let failures: Vec<&TestResult> = results.iter().filter(|r| !r.passed()).collect();
            if !failures.is_empty() {
                println!("     {}:", "Failures".bold().red());
                for f in &failures {
                    println!(
                        "        {} {}{}: {}",
                        "FAIL".red(),
                        f.entry.path,
                        loc_suffix(&f.entry.file, f.entry.line).dim(),
                        reason_of(&f.outcome).unwrap_or_default().bold()
                    );
                }
                println!("{}", rule.dim());
            }
            let status = if failed == 0 {
                format!("{passed} passed").green().to_string()
            } else {
                format!("{passed} passed")
            };
            let fail_status = if failed == 0 {
                format!("{failed} failed").green().to_string()
            } else {
                format!("{failed} failed").red().to_string()
            };
            println!(
                "    {} {} {} tests run: {}, {}",
                "Summary".dim(),
                format!("[{:>7}]", fmt_duration(total_elapsed)).dim(),
                total,
                status,
                fail_status,
            );
        }
        // Failure reasons were already inline on the FAIL lines.
        OutputFormat::Concise => {
            println!("summary: {total} run, {passed} passed, {failed} failed");
        }
        OutputFormat::JsonLines => {
            print_json(&TestSummary {
                r#type: "summary",
                total,
                passed,
                failed,
            });
        }
    }

    failed
}

fn report_one(r: &TestResult, format: OutputFormat) {
    let name = &r.entry.path;
    let reason = reason_of(&r.outcome);
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
                format!("[{:>7}]", fmt_duration(r.duration)).dim(),
                name,
            );
        }
        // Reason inline, no durations/decoration — line-stable for agents.
        OutputFormat::Concise => match &reason {
            None => println!("PASS {name}"),
            Some(reason) => println!(
                "FAIL {name}{}: {}",
                loc_suffix(&r.entry.file, r.entry.line),
                reason.replace('\n', " ")
            ),
        },
        OutputFormat::JsonLines => {
            print_json(&TestRecord {
                r#type: "test",
                name,
                status: if r.passed() { "pass" } else { "fail" },
                reason: reason.as_deref(),
                duration_us: r.duration.as_micros(),
                file: (!r.entry.file.is_empty()).then_some(r.entry.file.as_str()),
                line: (r.entry.line > 0).then_some(r.entry.line),
            });
        }
    }
}
