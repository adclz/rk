//! Reporting for `rk test`.
//!
//! Execution belongs to the runtime, and it happens in a *separate process*:
//! `rk test` builds a module and hands it to `runtime --test`, which runs it
//! and dies. The results come back as newline-delimited JSON
//! ([`wire::report`]), and what is left here is presentation — three output
//! shapes, one of which (`json-lines`) other programs parse.
//!
//! Spawning rather than linking is the whole point. A test must run against the
//! module a plant would run, under the same loader; if the compiler ran it
//! in-process, "the runtime" would be a library the compiler happened to
//! contain, and the two could drift without anything noticing. Now the same
//! binary that runs a controller runs the tests.
//!
//! It also means the results arrive as they happen: a suite is streamed, so a
//! failure appears when it occurs rather than when the run ends.

use std::io::BufRead;
use std::path::Path;
use std::time::Duration;

use wire::report::{ReportLine, Status, Summary, TestRecord};
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
pub fn run_tests(wasm_path: &Path, filter: Option<&str>, format: OutputFormat) -> usize {
    let binary = crate::spawn::runtime_binary();
    let mut command = std::process::Command::new(&binary);
    command.arg("--test").arg(wasm_path);
    if let Some(filter) = filter {
        command.arg("--filter").arg(filter);
    }
    command.stdout(std::process::Stdio::piped());

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(e) => {
            // The runtime is a separate binary now, so its absence is a real
            // failure mode a user can hit — name it and say where we looked.
            ui::error(crate::spawn::missing_hint(&binary, &e));
            return 1;
        }
    };

    let Some(stdout) = child.stdout.take() else {
        ui::error("the runtime produced no output");
        return 1;
    };

    if format == OutputFormat::Full {
        println!("{}  {}", "    Running".dim(), wasm_path.display());
    }

    // Render each line as it arrives: the runtime flushes per test, so a long
    // suite reports progress instead of going quiet.
    let total_start = std::time::Instant::now();
    let (summary, failures) = read_report(std::io::BufReader::new(stdout), format);
    let total_elapsed = total_start.elapsed();

    let status = child.wait();
    let Some(summary) = summary else {
        // No summary means the run did not finish: a crash, a kill, a module
        // the runtime refused. Do not report that as zero failures.
        let detail = match status {
            Ok(s) if !s.success() => format!(" (runtime exited with {s})"),
            Ok(_) => String::new(),
            Err(e) => format!(" ({e})"),
        };
        ui::error(format!("the test run did not finish{detail}"));
        return 1;
    };

    if summary.total == 0 {
        if format == OutputFormat::JsonLines {
            print_json(&ReportLine::Summary(summary));
        } else {
            println!("No test functions found");
        }
        // Still non-zero: an empty test run is a failure, not a green build.
        return 1;
    }

    match format {
        OutputFormat::Full => {
            let rule = "────────────────────────────────────────────────────────────";
            println!("{}", rule.dim());
            if !failures.is_empty() {
                println!("     {}:", "Failures".bold().red());
                for f in &failures {
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
fn print_json<T: serde::Serialize>(record: &T) {
    if let Ok(json) = serde_json::to_string(record) {
        println!("{json}");
    }
}

/// Consume the runtime's report stream, rendering each test as it arrives.
///
/// Returns the summary — `None` when the stream ended without one, which means
/// the run did not finish and must NOT be reported as zero failures — and the
/// failing records, for the recap block.
fn read_report(
    reader: impl BufRead,
    format: OutputFormat,
) -> (Option<Summary>, Vec<TestRecord>) {
    let mut summary = None;
    let mut failures = Vec::new();
    for line in reader.lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<ReportLine>(&line) {
            Ok(ReportLine::Test(record)) => {
                if record.status == Status::Fail {
                    failures.push(record.clone());
                }
                report_one(&record, format);
            }
            Ok(ReportLine::Summary(s)) => summary = Some(s),
            // Anything else on stdout is the runtime talking out of turn.
            // Pass it through rather than swallowing it — it is likely the
            // explanation for a run that is about to look inexplicable.
            Err(_) => {
                if format != OutputFormat::JsonLines {
                    ui::detail(format!("    {line}"));
                }
            }
        }
    }
    (summary, failures)
}

fn report_one(r: &TestRecord, format: OutputFormat) {
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
                format!("[{:>7}]", fmt_duration(Duration::from_micros(r.duration_us))).dim(),
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
        // Already the wire's own shape: forward it unchanged, so what a program
        // parses here is exactly what the runtime said.
        OutputFormat::JsonLines => print_json(&ReportLine::Test(r.clone())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A run that dies partway leaves tests but no summary. That absence is
    /// the signal the caller turns into "the run did not finish" — so it must
    /// come back as `None`, never as a summary inferred from the lines seen.
    #[test]
    fn a_truncated_stream_has_no_summary() {
        let stream = concat!(
            r#"{"type":"test","name":"t_one","status":"pass","duration_us":5}"#,
            "\n",
            r#"{"type":"test","name":"t_two","status":"fail","reason":"boom","duration_us":7}"#,
            "\n",
            // killed here: no summary line
        );
        let (summary, failures) = read_report(stream.as_bytes(), OutputFormat::Concise);
        assert_eq!(summary, None, "no summary may be invented for a dead run");
        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0].name, "t_two");
    }

    /// The happy path: tests stream, the summary arrives last, and only the
    /// failing records are kept for the recap.
    #[test]
    fn a_complete_stream_yields_its_summary_and_failures() {
        let stream = concat!(
            r#"{"type":"test","name":"t_one","status":"pass","duration_us":5}"#,
            "\n",
            r#"{"type":"test","name":"t_two","status":"fail","reason":"boom","duration_us":7}"#,
            "\n",
            r#"{"type":"summary","total":2,"passed":1,"failed":1}"#,
            "\n",
        );
        let (summary, failures) = read_report(stream.as_bytes(), OutputFormat::Concise);
        assert_eq!(
            summary,
            Some(Summary {
                total: 2,
                passed: 1,
                failed: 1
            })
        );
        assert_eq!(failures.len(), 1);
    }

    /// A line that is not a report record — a stray print, a panic message —
    /// must not end the stream: everything after it still counts.
    #[test]
    fn a_stray_line_does_not_end_the_stream() {
        let stream = concat!(
            "something wrote to stdout\n",
            r#"{"type":"test","name":"t_one","status":"pass","duration_us":5}"#,
            "\n",
            r#"{"type":"summary","total":1,"passed":1,"failed":0}"#,
            "\n",
        );
        let (summary, failures) = read_report(stream.as_bytes(), OutputFormat::Concise);
        assert_eq!(summary.map(|s| s.total), Some(1));
        assert!(failures.is_empty());
    }
}


