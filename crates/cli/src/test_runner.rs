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

/// Discover tests from the MIR test manifest.
/// Returns (display_path, wasm_export_name) pairs.
fn discover_tests(manifest: &mir::test_manifest::TestManifest) -> Vec<(String, String)> {
    manifest
        .tests
        .iter()
        .map(|t| (t.path.clone(), t.export.clone()))
        .collect()
}

/// Build a linker with all host imports pre-registered.
/// Every function is a direct `func_wrap` with concrete types - zero runtime dispatch.
fn build_linker(engine: &Engine, _module: &Module) -> WasmResult<Linker<HostState>> {
    let mut linker = Linker::new(engine);
    linker.allow_shadowing(true);

    // Assert - takes (ptr: i32, len: i32) for the message string
    linker.func_wrap(
        "assert",
        "fail",
        |mut caller: Caller<'_, HostState>, ptr: i32, len: i32| -> WasmResult<()> {
            let msg = if len > 0 {
                caller
                    .get_export("memory")
                    .and_then(|e| e.into_memory())
                    .map(|mem| {
                        let data = mem.data(&caller);
                        let start = ptr as usize;
                        let end = (start + len as usize).min(data.len());
                        String::from_utf8_lossy(&data[start..end]).to_string()
                    })
                    .unwrap_or_default()
            } else {
                String::new()
            };
            if msg.is_empty() {
                Err(wasmtime::Error::msg("assertion failed"))
            } else {
                Err(wasmtime::Error::msg(format!("assertion failed: {}", msg)))
            }
        },
    )?;

    // WASI clocks
    let epoch = Instant::now();
    linker.func_wrap("wasi:clocks/monotonic-clock", "now", move || -> i64 {
        epoch.elapsed().as_nanos() as i64
    })?;

    // Math - all monomorphized variants, O(1) direct calls
    register_all_math(&mut linker)?;

    Ok(linker)
}

/// Register a math host function dynamically based on the import's type signature.
///
/// Extracts the operation name (e.g. "abs" from "abs.INT") and uses the WASM
/// function type to determine parameter types. All math operations dispatch to
/// Rust's built-in methods on f32/f64/i32/i64.
/// Register ALL math host functions upfront with concrete typed `func_wrap`.
/// Each variant is a direct function pointer - zero runtime dispatch overhead.
fn register_all_math(linker: &mut Linker<HostState>) -> WasmResult<()> {
    macro_rules! math1_i32 {
        ($name:literal, $op:expr) => {
            linker.func_wrap("math", $name, |v: i32| -> i32 { $op(v) })?;
        };
    }
    macro_rules! math1_i64 {
        ($name:literal, $op:expr) => {
            linker.func_wrap("math", $name, |v: i64| -> i64 { $op(v) })?;
        };
    }
    macro_rules! math1_f32 {
        ($name:literal, $method:ident) => {
            linker.func_wrap("math", $name, |v: f32| -> f32 { v.$method() })?;
        };
    }
    macro_rules! math1_f64 {
        ($name:literal, $method:ident) => {
            linker.func_wrap("math", $name, |v: f64| -> f64 { v.$method() })?;
        };
    }
    macro_rules! math2_f32 {
        ($name:literal, $method:ident) => {
            linker.func_wrap("math", $name, |a: f32, b: f32| -> f32 { a.$method(b) })?;
        };
    }
    macro_rules! math2_f64 {
        ($name:literal, $method:ident) => {
            linker.func_wrap("math", $name, |a: f64, b: f64| -> f64 { a.$method(b) })?;
        };
    }

    // ABS - signed integers
    math1_i32!("abs.SINT", i32::abs);
    math1_i32!("abs.INT", i32::abs);
    math1_i32!("abs.DINT", i32::abs);
    math1_i64!("abs.LINT", i64::abs);
    // ABS - unsigned (identity)
    linker.func_wrap("math", "abs.USINT", |v: i32| -> i32 { v })?;
    linker.func_wrap("math", "abs.UINT", |v: i32| -> i32 { v })?;
    linker.func_wrap("math", "abs.UDINT", |v: i32| -> i32 { v })?;
    linker.func_wrap("math", "abs.ULINT", |v: i64| -> i64 { v })?;
    // ABS - float
    math1_f32!("abs.REAL", abs);
    math1_f64!("abs.LREAL", abs);

    // SQRT
    math1_f32!("sqrt.REAL", sqrt);
    math1_f64!("sqrt.LREAL", sqrt);

    // LN
    math1_f32!("ln.REAL", ln);
    math1_f64!("ln.LREAL", ln);

    // LOG
    math1_f32!("log.REAL", log10);
    math1_f64!("log.LREAL", log10);

    // EXP
    math1_f32!("exp.REAL", exp);
    math1_f64!("exp.LREAL", exp);

    // Trigonometry
    math1_f32!("sin.REAL", sin);
    math1_f64!("sin.LREAL", sin);
    math1_f32!("cos.REAL", cos);
    math1_f64!("cos.LREAL", cos);
    math1_f32!("tan.REAL", tan);
    math1_f64!("tan.LREAL", tan);
    math1_f32!("asin.REAL", asin);
    math1_f64!("asin.LREAL", asin);
    math1_f32!("acos.REAL", acos);
    math1_f64!("acos.LREAL", acos);
    math1_f32!("atan.REAL", atan);
    math1_f64!("atan.LREAL", atan);

    // Two-param
    math2_f32!("atan2.REAL", atan2);
    math2_f64!("atan2.LREAL", atan2);
    math2_f32!("expt.REAL", powf);
    math2_f64!("expt.LREAL", powf);

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
/// Reads the test manifest from `<workspace>/rk_build/manifest`.
/// Returns the number of failures.
pub fn run_tests(
    wasm_path: &std::path::Path,
    workspace: &std::path::Path,
    filter: Option<&str>,
) -> usize {
    let manifest_path = workspace.join("rk_build").join("test").join("manifest");
    let manifest = match std::fs::read(&manifest_path) {
        Ok(bytes) => match mir::test_manifest::TestManifest::from_msgpack(&bytes) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("{}failed to parse manifest: {}", "error: ".bold().red(), e);
                return 1;
            }
        },
        Err(e) => {
            eprintln!("{}failed to read manifest at {}: {}", "error: ".bold().red(), manifest_path.display(), e);
            return 1;
        }
    };
    let engine = Engine::default();
    let module = match Module::from_file(&engine, wasm_path) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("{}{}", "wasm error: ".bold().red(), e);
            return 1;
        }
    };

    let mut tests = discover_tests(&manifest);

    if let Some(f) = filter {
        let f_lower = f.to_lowercase();
        tests.retain(|(path, _)| path.to_lowercase().contains(&f_lower));
    }

    if tests.is_empty() {
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

    let total = tests.len();
    let mut results: Vec<TestResult> = Vec::with_capacity(total);
    let total_start = Instant::now();

    println!("{}  {} test(s)", "    Running".dim(), total);

    for (display_name, export_name) in &tests {
        // Fresh store per test - full memory isolation
        let mut store = Store::new(&engine, HostState);
        let start = Instant::now();

        let outcome = match linker.instantiate(&mut store, &module) {
            Err(e) => TestOutcome::Fail(format!("instantiation failed: {}", e)),
            Ok(instance) => match instance.get_typed_func::<(), ()>(&mut store, export_name) {
                Err(e) => TestOutcome::Fail(format!("export error: {}", e)),
                Ok(func) => match func.call(&mut store, ()) {
                    Ok(()) => TestOutcome::Pass,
                    Err(e) => {
                        // Walk the error chain to find our assertion message
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
                            e.to_string().lines().next().unwrap_or("unknown error").to_string()
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
                // Bold the assertion message part
                let display = if let Some(msg) = reason.strip_prefix("assertion failed: ") {
                    format!("assertion failed: {}", msg.bold())
                } else {
                    reason.to_string()
                };
                println!("        {} {} - {}", "FAIL".red(), f.name, display);
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
