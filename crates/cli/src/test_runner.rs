//! WASM test runner using wasmtime.
//!
//! Runs each test in an isolated WASM instance for clean memory state.
//! Provides nextest-style output with per-test timing.

use std::time::Instant;

use wasmtime::*;
use yansi::Paint;

type WasmResult<T> = wasmtime::Result<T>;

/// Result of running a single test.
struct TestResult {
    name: String,
    outcome: TestOutcome,
    duration: std::time::Duration,
}

enum TestOutcome {
    Pass,
    Fail(String),
}

/// Host functions provided to the WASM module.
struct HostState;

/// Discover all test exports (functions starting with "test_") from a WASM module.
fn discover_tests(module: &Module) -> Vec<String> {
    module
        .exports()
        .filter(|e| e.name().starts_with("test_") && e.ty().func().is_some())
        .map(|e| e.name().to_string())
        .collect()
}

/// Build a linker with all host imports (math, assert, clocks).
fn build_linker(engine: &Engine, module: &Module) -> WasmResult<Linker<HostState>> {
    let mut linker = Linker::new(engine);

    // Scan module imports and register each one
    for import in module.imports() {
        let module_name = import.module();
        let field_name = import.name();

        match (module_name, field_name) {
            ("assert", "fail") => {
                linker.func_wrap("assert", "fail", || -> WasmResult<()> {
                    Err(wasmtime::Error::msg("assertion failed"))
                })?;
            }
            ("wasi:clocks/monotonic-clock", "now") => {
                // Monotonic clock returning nanoseconds as i64
                let epoch = Instant::now();
                linker.func_wrap("wasi:clocks/monotonic-clock", "now", move || -> i64 {
                    epoch.elapsed().as_nanos() as i64
                })?;
            }
            ("math", name) => {
                register_math_func(&mut linker, name, &import)?;
            }
            _ => {
                // Unknown import — skip (will fail at instantiation if actually called)
            }
        }
    }

    Ok(linker)
}

/// Register a math host function based on the import's type signature.
fn register_math_func(
    linker: &mut Linker<HostState>,
    name: &str,
    import: &ImportType,
) -> WasmResult<()> {
    let import_ty = import.ty();
    let func_ty = import_ty.func().unwrap();
    let params: Vec<_> = func_ty.params().collect();
    let results: Vec<_> = func_ty.results().collect();

    let is_two_param = params.len() == 2;
    let is_i32 = params.first().map_or(false, |p| p.matches(&ValType::I32));
    let is_i64 = params.first().map_or(false, |p| p.matches(&ValType::I64));
    let is_f32 = params.first().map_or(false, |p| p.matches(&ValType::F32));
    let is_f64 = params.first().map_or(false, |p| p.matches(&ValType::F64));

    let op = name.split('.').next().unwrap_or(name);

    // Integer abs
    if op == "abs" && is_i32 {
        linker.func_wrap("math", name, |v: i32| -> i32 { v.abs() })?;
        return Ok(());
    }
    if op == "abs" && is_i64 {
        linker.func_wrap("math", name, |v: i64| -> i64 { v.abs() })?;
        return Ok(());
    }

    match (op, is_f64, is_two_param) {
        // Single-param f32 operations
        ("abs", false, false) if is_f32 => {
            linker.func_wrap("math", name, |v: f32| -> f32 { v.abs() })?;
        }
        ("sqrt", false, false) => {
            linker.func_wrap("math", name, |v: f32| -> f32 { v.sqrt() })?;
        }
        ("ln", false, false) => {
            linker.func_wrap("math", name, |v: f32| -> f32 { v.ln() })?;
        }
        ("log", false, false) => {
            linker.func_wrap("math", name, |v: f32| -> f32 { v.log10() })?;
        }
        ("exp", false, false) => {
            linker.func_wrap("math", name, |v: f32| -> f32 { v.exp() })?;
        }
        ("sin", false, false) => {
            linker.func_wrap("math", name, |v: f32| -> f32 { v.sin() })?;
        }
        ("cos", false, false) => {
            linker.func_wrap("math", name, |v: f32| -> f32 { v.cos() })?;
        }
        ("tan", false, false) => {
            linker.func_wrap("math", name, |v: f32| -> f32 { v.tan() })?;
        }
        ("asin", false, false) => {
            linker.func_wrap("math", name, |v: f32| -> f32 { v.asin() })?;
        }
        ("acos", false, false) => {
            linker.func_wrap("math", name, |v: f32| -> f32 { v.acos() })?;
        }
        ("atan", false, false) => {
            linker.func_wrap("math", name, |v: f32| -> f32 { v.atan() })?;
        }
        // Single-param f64 operations
        ("abs", true, false) => {
            linker.func_wrap("math", name, |v: f64| -> f64 { v.abs() })?;
        }
        ("sqrt", true, false) => {
            linker.func_wrap("math", name, |v: f64| -> f64 { v.sqrt() })?;
        }
        ("ln", true, false) => {
            linker.func_wrap("math", name, |v: f64| -> f64 { v.ln() })?;
        }
        ("log", true, false) => {
            linker.func_wrap("math", name, |v: f64| -> f64 { v.log10() })?;
        }
        ("exp", true, false) => {
            linker.func_wrap("math", name, |v: f64| -> f64 { v.exp() })?;
        }
        ("sin", true, false) => {
            linker.func_wrap("math", name, |v: f64| -> f64 { v.sin() })?;
        }
        ("cos", true, false) => {
            linker.func_wrap("math", name, |v: f64| -> f64 { v.cos() })?;
        }
        ("tan", true, false) => {
            linker.func_wrap("math", name, |v: f64| -> f64 { v.tan() })?;
        }
        ("asin", true, false) => {
            linker.func_wrap("math", name, |v: f64| -> f64 { v.asin() })?;
        }
        ("acos", true, false) => {
            linker.func_wrap("math", name, |v: f64| -> f64 { v.acos() })?;
        }
        ("atan", true, false) => {
            linker.func_wrap("math", name, |v: f64| -> f64 { v.atan() })?;
        }
        // Two-param operations
        ("atan2", false, true) => {
            linker.func_wrap("math", name, |y: f32, x: f32| -> f32 { y.atan2(x) })?;
        }
        ("atan2", true, true) => {
            linker.func_wrap("math", name, |y: f64, x: f64| -> f64 { y.atan2(x) })?;
        }
        ("expt", false, true) => {
            linker.func_wrap("math", name, |b: f32, e: f32| -> f32 { b.powf(e) })?;
        }
        ("expt", true, true) => {
            linker.func_wrap("math", name, |b: f64, e: f64| -> f64 { b.powf(e) })?;
        }
        _ => {}
    }

    Ok(())
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

/// Run tests from a compiled WASM module.
/// Returns the number of failures.
pub fn run_tests(wasm_bytes: &[u8], filter: Option<&str>) -> usize {
    let engine = Engine::default();
    let module = match Module::new(&engine, wasm_bytes) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("{}{}", "wasm error: ".bold().red(), e);
            return 1;
        }
    };

    let mut test_names = discover_tests(&module);

    if let Some(f) = filter {
        test_names.retain(|n| n.contains(f));
    }

    if test_names.is_empty() {
        println!("No test functions found");
        return 1;
    }

    let linker = match build_linker(&engine, &module) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("{}{}", "linker error: ".bold().red(), e);
            return 1;
        }
    };

    let total = test_names.len();
    let mut results: Vec<TestResult> = Vec::with_capacity(total);
    let total_start = Instant::now();

    println!("{}  {} test(s)", "    Running".dim(), total);

    for name in &test_names {
        // Fresh store per test — full memory isolation
        let mut store = Store::new(&engine, HostState);
        let start = Instant::now();

        let outcome = match linker.instantiate(&mut store, &module) {
            Err(e) => TestOutcome::Fail(format!("instantiation failed: {}", e)),
            Ok(instance) => match instance.get_typed_func::<(), ()>(&mut store, name) {
                Err(e) => TestOutcome::Fail(format!("export error: {}", e)),
                Ok(func) => match func.call(&mut store, ()) {
                    Ok(()) => TestOutcome::Pass,
                    Err(e) => {
                        let msg = e.to_string();
                        if msg.contains("assertion") {
                            TestOutcome::Fail("assertion failed".into())
                        } else {
                            let first_line = msg.lines().next().unwrap_or(&msg);
                            TestOutcome::Fail(first_line.to_string())
                        }
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
                    name,
                );
            }
            TestOutcome::Fail(_) => {
                println!(
                    "        {} {} {}",
                    "FAIL".red(),
                    format!("[{:>7}]", fmt_duration(duration)).dim(),
                    name,
                );
            }
        }

        results.push(TestResult {
            name: name.clone(),
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

    // Separator
    println!(
        "{}",
        "────────────────────────────────────────────────────────────".dim()
    );

    // List failures
    if !failures.is_empty() {
        println!("     {}:", "Failures".bold().red());
        for f in &failures {
            if let TestOutcome::Fail(reason) = &f.outcome {
                println!("        {} {} — {}", "FAIL".red(), f.name, reason);
            }
        }
        println!(
            "{}",
            "────────────────────────────────────────────────────────────".dim()
        );
    }

    // Summary
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
