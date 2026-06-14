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

type WasmResult<T> = wasmtime::Result<T>;

struct TestResult {
    name: String,
    outcome: TestOutcome,
    duration: std::time::Duration,
}

enum TestOutcome {
    Pass,
    Fail(String),
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
/// component binary.
fn discover_tests(component_bytes: &[u8]) -> Vec<(String, String)> {
    match read_manifest_section(component_bytes) {
        Some(manifest) => manifest
            .tests
            .iter()
            .map(|t| (t.path.clone(), t.export.clone()))
            .collect(),
        None => {
            eprintln!(
                "{}no test manifest found in component (missing `{}` custom section)",
                "error: ".bold().red(),
                mir::test_manifest::TEST_MANIFEST_SECTION
            );
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
pub fn run_tests(wasm_path: &std::path::Path, filter: Option<&str>) -> usize {
    let mut config = Config::new();
    config.wasm_component_model(true);
    config.wasm_exceptions(true);

    let engine = Engine::new(&config).expect("Failed to create engine");

    let bytes = match std::fs::read(wasm_path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("{}failed to read component: {}", "error: ".bold().red(), e);
            return 1;
        }
    };

    let component = match Component::from_binary(&engine, &bytes) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{}{}", "wasm error: ".bold().red(), e);
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
        tests.retain(|(path, _)| path.to_lowercase().contains(&f_lower));
    }

    if tests.is_empty() {
        println!("No test functions found");
        return 1;
    }

    let linker = match build_linker(&engine) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("{}{}", "linker error: ".bold().red(), e);
            return 1;
        }
    };

    let total = tests.len();
    let mut results: Vec<TestResult> = Vec::with_capacity(total);
    let total_start = Instant::now();

    println!("{}  {} test(s)", "    Running".dim(), total);

    for (display_name, export_name) in &tests {
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
                        Ok(()) => {
                            let outcome = match &results[0] {
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
                            };
                            if let Err(e) = func.post_return(&mut store) {
                                TestOutcome::Fail(format!("post_return: {}", e))
                            } else {
                                outcome
                            }
                        }
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

        match &outcome {
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
        }

        results.push(TestResult {
            name: display_name.clone(),
            outcome,
            duration,
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
                println!("        {} {}: {}", "FAIL".red(), f.name, reason.bold());
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

    failed
}
