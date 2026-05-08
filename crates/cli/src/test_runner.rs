//! WASM Component test runner using wasmtime.
//!
//! Loads a WASM component, discovers tests from the manifest,
//! and runs each test in an isolated instance.
//! Uses WASI p2 for clocks and provides IEC-specific imports (math, assert).

use std::time::Instant;

use wasmtime::component::{Component, Linker};
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

/// Discover tests from the manifest file.
fn discover_tests(workspace: &std::path::Path) -> Vec<(String, String)> {
    let manifest_path = workspace.join("rk_build").join("test").join("manifest");
    let bytes = match std::fs::read(&manifest_path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("{}failed to read manifest: {}", "error: ".bold().red(), e);
            return vec![];
        }
    };
    match mir::test_manifest::TestManifest::from_msgpack(&bytes) {
        Ok(manifest) => manifest
            .tests
            .iter()
            .map(|t| (t.path.clone(), t.export.clone()))
            .collect(),
        Err(e) => {
            eprintln!("{}failed to parse manifest: {}", "error: ".bold().red(), e);
            vec![]
        }
    }
}

/// Build a component linker with WASI + IEC host imports.
fn build_linker(engine: &Engine) -> WasmResult<Linker<HostState>> {
    let mut linker = Linker::new(engine);

    // WASI p2 — provides clocks, filesystem, etc.
    wasmtime_wasi::p2::add_to_linker_sync(&mut linker)?;

    // Assert — the component advertises a proper `string` param; the canonical
    // ABI lowers (ptr, len) from guest memory into a host String for us.
    linker.root().func_wrap(
        "assert-fail",
        |_store: wasmtime::StoreContextMut<'_, HostState>,
         (msg,): (String,)|
         -> WasmResult<()> {
            if msg.is_empty() {
                Err(wasmtime::Error::msg("assertion failed"))
            } else {
                Err(wasmtime::Error::msg(format!(
                    "assertion failed: {}",
                    msg
                )))
            }
        },
    )?;

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
/// Reads the test manifest from `<workspace>/rk_build/test/manifest`.
pub fn run_tests(
    wasm_path: &std::path::Path,
    workspace: &std::path::Path,
    filter: Option<&str>,
) -> usize {
    let mut config = Config::new();
    config.wasm_component_model(true);
    let engine = Engine::new(&config).expect("Failed to create engine");

    let component = match Component::from_file(&engine, wasm_path) {
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

    let mut tests = discover_tests(workspace);

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
                Some(func) => match func.call(&mut store, &[], &mut []) {
                    Ok(()) => {
                        if let Err(e) = func.post_return(&mut store) {
                            TestOutcome::Fail(format!("post_return: {}", e))
                        } else {
                            TestOutcome::Pass
                        }
                    }
                    Err(e) => {
                        let mut reason = None;
                        let mut source: Option<&dyn std::error::Error> = Some(&*e);
                        while let Some(err) = source {
                            let msg = err.to_string();
                            if msg.starts_with("assertion failed") {
                                reason = Some(msg);
                                break;
                            }
                            source = err.source();
                        }
                        TestOutcome::Fail(reason.unwrap_or_else(|| {
                            e.to_string()
                                .lines()
                                .next()
                                .unwrap_or("unknown error")
                                .to_string()
                        }))
                    }
                },
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
                let display = if let Some(msg) = reason.strip_prefix("assertion failed: ") {
                    format!("assertion failed: {}", msg.bold())
                } else {
                    reason.to_string()
                };
                println!("        {} {} — {}", "FAIL".red(), f.name, display);
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
