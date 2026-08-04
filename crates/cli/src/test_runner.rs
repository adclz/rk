//! WASM Component test runner using wasmtime.
//!
//! Loads a WASM component, discovers tests from the manifest,
//! and runs each test in an isolated instance.
//! Uses WASI p2 for clocks and provides IEC-specific imports (math, assert).

use std::time::Instant;

use wasmtime::component::{Component, Linker, Val};
use wasmtime::{Config, Engine, Store};
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder, WasiView};
use yansi::Paint;

use crate::cli::OutputFormat;
use crate::ui;

type WasmResult<T> = wasmtime::Result<T>;

struct TestResult {
    name: String,
    /// Declaration site from the manifest (file may be empty, line 0 = unknown).
    file: String,
    line: u32,
    outcome: TestOutcome,
}

/// ` (file:line)` when the declaration site is known, empty otherwise.
fn loc_suffix(file: &str, line: u32) -> String {
    if file.is_empty() {
        String::new()
    } else {
        format!(" ({file}:{line})")
    }
}

enum TestOutcome {
    Pass,
    Fail(String),
}

/// NDJSON record for one test in [`OutputFormat::JsonLines`]. Field order is
/// part of the wire shape; additions go at the END.
#[derive(serde::Serialize)]
struct TestRecord<'a> {
    r#type: &'static str,
    name: &'a str,
    status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<&'a str>,
    duration_us: u128,
    #[serde(skip_serializing_if = "Option::is_none")]
    file: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    line: Option<u32>,
}

/// Terminal NDJSON record: totals for the whole run.
#[derive(serde::Serialize)]
struct TestSummary {
    r#type: &'static str,
    total: usize,
    passed: usize,
    failed: usize,
}

/// Host state with WASI context.
struct HostState {
    wasi: WasiCtx,
    table: wasmtime::component::ResourceTable,
}

impl WasiView for HostState {
    fn ctx(&mut self) -> wasmtime_wasi::WasiCtxView<'_> {
        wasmtime_wasi::WasiCtxView {
            ctx: &mut self.wasi,
            table: &mut self.table,
        }
    }
}

/// Discover tests from the `rk.test-manifest` custom section embedded in the
/// component binary. Entries carry the test's declaration site
/// (workspace-relative `file` + 1-based `line`) alongside path/export.
fn discover_tests(component_bytes: &[u8]) -> Vec<mir::test_manifest::TestEntry> {
    match read_manifest_section(component_bytes) {
        Some(manifest) => manifest.tests,
        None => {
            ui::error(format!(
                "no test manifest found in component (missing `{}` custom section)",
                mir::test_manifest::TEST_MANIFEST_SECTION
            ));
            vec![]
        }
    }
}

/// Find the `rk.test-manifest` custom section in the component binary and
/// decode the MessagePack [`mir::test_manifest::TestManifest`] it carries.
fn read_manifest_section(bytes: &[u8]) -> Option<mir::test_manifest::TestManifest> {
    for payload in wasmparser::Parser::new(0).parse_all(bytes) {
        if let Ok(wasmparser::Payload::CustomSection(reader)) = payload
            && reader.name() == mir::test_manifest::TEST_MANIFEST_SECTION
        {
            return mir::test_manifest::TestManifest::from_msgpack(reader.data()).ok();
        }
    }
    None
}

/// Build a component linker with WASI imports.
///
/// `{test}` functions are codegen-wrapped in a wasm-level `try_table`
/// that catches `$rk_exception` and surfaces the message as a typed
/// `result<unit, string>` Err - no host import is needed for assertion
/// capture; the canonical-ABI lift handles the payload.
fn build_linker(engine: &Engine) -> WasmResult<Linker<HostState>> {
    let mut linker = Linker::new(engine);

    // WASI p2 - provides clocks, filesystem, etc.
    wasmtime_wasi::p2::add_to_linker_sync(&mut linker)?;

    Ok(linker)
}

fn fmt_duration(d: std::time::Duration) -> String {
    let us = d.as_micros();
    if us < 1000 {
        format!("{}µs", us)
    } else if us < 1_000_000 {
        format!("{:.1}ms", us as f64 / 1000.0)
    } else {
        format!("{:.2}s", us as f64 / 1_000_000.0)
    }
}

/// Run tests from a compiled WASM component.
/// Reads the test manifest from the component's `rk.test-manifest` custom
/// section — there is no sidecar file.
///
/// Output per format (results go to STDOUT): `full` keeps the human
/// Running/PASS-FAIL/Failures/Summary block; `concise` prints one stable
/// `PASS <name>` / `FAIL <name>: <reason>` line per test (no durations — they
/// would defeat matching) and a final `summary:` line; `json-lines` prints one
/// [`TestRecord`] per test and a terminal [`TestSummary`].
pub fn run_tests(wasm_path: &std::path::Path, filter: Option<&str>, format: OutputFormat) -> usize {
    let mut config = Config::new();
    config.wasm_component_model(true);
    config.wasm_exceptions(true);

    let engine = Engine::new(&config).expect("Failed to create engine");

    let bytes = match std::fs::read(wasm_path) {
        Ok(b) => b,
        Err(e) => {
            ui::error(format!("failed to read component: {e}"));
            return 1;
        }
    };

    let component = match Component::from_binary(&engine, &bytes) {
        Ok(c) => c,
        Err(e) => {
            ui::error(format!("wasm: {e}"));
            let mut src = e.source();
            while let Some(cause) = src {
                eprintln!("  caused by: {}", cause);
                src = cause.source();
            }
            return 1;
        }
    };

    let mut tests = discover_tests(&bytes);

    if let Some(f) = filter {
        let f_lower = f.to_lowercase();
        tests.retain(|t| t.path.to_lowercase().contains(&f_lower));
    }

    if tests.is_empty() {
        // Keep stdout pure NDJSON in json-lines mode — the empty run is
        // expressed by the summary record. (Still exit non-zero: an empty test
        // run is a failure, not a green build.)
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

    let linker = match build_linker(&engine) {
        Ok(l) => l,
        Err(e) => {
            ui::error(format!("linker: {e}"));
            return 1;
        }
    };

    let total = tests.len();
    let mut results: Vec<TestResult> = Vec::with_capacity(total);
    let total_start = Instant::now();

    if format == OutputFormat::Full {
        println!("{}  {} test(s)", "    Running".dim(), total);
    }

    for entry in &tests {
        let (display_name, export_name) = (&entry.path, &entry.export);
        let mut store = Store::new(
            &engine,
            HostState {
                wasi: WasiCtxBuilder::new().build(),
                table: wasmtime::component::ResourceTable::new(),
            },
        );
        let start = Instant::now();

        // Convert export name to kebab-case (component uses kebab names)
        let kebab_name = wasm_codegen::component::to_kebab_case(export_name);

        let outcome = match linker.instantiate(&mut store, &component) {
            Err(e) => TestOutcome::Fail(format!("instantiation failed: {}", e)),
            Ok(instance) => match instance.get_func(&mut store, &kebab_name) {
                None => TestOutcome::Fail(format!("export '{}' not found", kebab_name)),
                Some(func) => {
                    // `{test}` functions are typed at the component
                    // boundary as `() -> result<unit, string>`. The
                    // canonical-ABI lift reads the 12-byte result area
                    // we wrote in the core function, hands us a typed
                    // `Val::Result`:
                    //   - Ok variant  → test passed
                    //   - Err variant (carrying a string) → assertion
                    //     message; surface it as the failure reason.
                    let mut results = [Val::Bool(false)]; // placeholder
                    match func.call(&mut store, &[], &mut results) {
                        Ok(()) => match &results[0] {
                            Val::Result(Ok(_)) => TestOutcome::Pass,
                            Val::Result(Err(payload)) => {
                                let msg = match payload.as_deref() {
                                    Some(Val::String(s)) if s.is_empty() => {
                                        "<no error messsage provided>".to_string()
                                    }
                                    Some(Val::String(s)) => s.to_owned(),
                                    _ => "<no error messsage provided>".to_string(),
                                };
                                TestOutcome::Fail(msg)
                            }
                            other => TestOutcome::Fail(format!(
                                "unexpected test return shape: {:?}",
                                other
                            )),
                        },
                        // A trap or other non-typed failure - `__RAISE`
                        // is caught inside the test wrapper, so reaching
                        // here means an actual wasm trap (e.g. divide
                        // by zero, OOB) or a wasmtime-level error.
                        //
                        // Wasmtime's `Error::to_string` returns just the
                        // top-level "error while executing at wasm
                        // backtrace:" prefix; the actual trap kind
                        // (`integer divide by zero`, `out of bounds
                        // memory access`, etc.) lives on the
                        // downcastable `wasmtime::Trap` carried in the
                        // source chain. Try that first; fall back to
                        // the first line otherwise.
                        Err(e) => {
                            let msg = if let Some(trap) = e.downcast_ref::<wasmtime::Trap>() {
                                trap.to_string()
                            } else {
                                let mut src: Option<&dyn std::error::Error> = Some(e.as_ref());
                                let mut last = String::new();
                                while let Some(c) = src {
                                    last = c.to_string();
                                    src = c.source();
                                }
                                if last.is_empty() {
                                    e.to_string()
                                        .lines()
                                        .next()
                                        .unwrap_or("unknown error")
                                        .to_string()
                                } else {
                                    last
                                }
                            };
                            TestOutcome::Fail(msg)
                        }
                    }
                }
            },
        };

        let duration = start.elapsed();

        match format {
            OutputFormat::Full => match &outcome {
                TestOutcome::Pass => {
                    println!(
                        "        {} {} {}",
                        "PASS".green(),
                        format!("[{:>7}]", fmt_duration(duration)).dim(),
                        display_name,
                    );
                }
                TestOutcome::Fail(_) => {
                    println!(
                        "        {} {} {}",
                        "FAIL".red(),
                        format!("[{:>7}]", fmt_duration(duration)).dim(),
                        display_name,
                    );
                }
            },
            // Reason inline, no durations/decoration — line-stable for agents.
            OutputFormat::Concise => match &outcome {
                TestOutcome::Pass => println!("PASS {display_name}"),
                TestOutcome::Fail(reason) => {
                    println!(
                        "FAIL {display_name}{}: {}",
                        loc_suffix(&entry.file, entry.line),
                        reason.replace('\n', " ")
                    )
                }
            },
            OutputFormat::JsonLines => {
                let (status, reason) = match &outcome {
                    TestOutcome::Pass => ("pass", None),
                    TestOutcome::Fail(reason) => ("fail", Some(reason.as_str())),
                };
                print_json(&TestRecord {
                    r#type: "test",
                    name: display_name,
                    status,
                    reason,
                    duration_us: duration.as_micros(),
                    file: (!entry.file.is_empty()).then_some(entry.file.as_str()),
                    line: (entry.line > 0).then_some(entry.line),
                });
            }
        }

        results.push(TestResult {
            name: display_name.clone(),
            file: entry.file.clone(),
            line: entry.line,
            outcome,
        });
    }

    let total_elapsed = total_start.elapsed();
    let passed = results
        .iter()
        .filter(|r| matches!(r.outcome, TestOutcome::Pass))
        .count();
    let failed = results
        .iter()
        .filter(|r| matches!(r.outcome, TestOutcome::Fail(_)))
        .count();

    match format {
        OutputFormat::Full => {
            let failures: Vec<_> = results
                .iter()
                .filter(|r| matches!(r.outcome, TestOutcome::Fail(_)))
                .collect();

            println!(
                "{}",
                "────────────────────────────────────────────────────────────".dim()
            );

            if !failures.is_empty() {
                println!("     {}:", "Failures".bold().red());
                for f in &failures {
                    if let TestOutcome::Fail(reason) = &f.outcome {
                        println!(
                            "        {} {}{}: {}",
                            "FAIL".red(),
                            f.name,
                            loc_suffix(&f.file, f.line).dim(),
                            reason.bold()
                        );
                    }
                }
                println!(
                    "{}",
                    "────────────────────────────────────────────────────────────".dim()
                );
            }

            let status = if failed == 0 {
                format!("{} passed", passed).green().to_string()
            } else {
                format!("{} passed", passed).to_string()
            };
            let fail_status = if failed == 0 {
                format!("{} failed", failed).green().to_string()
            } else {
                format!("{} failed", failed).red().to_string()
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

/// Print one NDJSON record to stdout.
fn print_json<T: serde::Serialize>(record: &T) {
    if let Ok(json) = serde_json::to_string(record) {
        println!("{json}");
    }
}
