use std::io::Write;

use db::RootDatabase;
use hir::hir_def::semantic_index::semantic_index;

use crate::diagnostics::{DiagnosticReporter, collect_diagnostics};
use crate::ui;

/// Where an internal compiler error should be reported.
const ISSUES_URL: &str = "https://github.com/adclz/rk/issues";

/// Render a MIR-lowering failure as an INTERNAL COMPILER ERROR:
/// diagnostic-clean source must never fail to lower, so an error here is a
/// compiler bug, not a user-code diagnostic. Rendered with the source
/// excerpt when it carries a location.
fn render_codegen_error(
    db: &RootDatabase,
    workspace: &std::path::Path,
    err: &mir::lower::lower_type::LowerTypeError,
    format: crate::cli::OutputFormat,
) -> Option<String> {
    use auto_lsp::lsp_types::DiagnosticSeverity;

    let (file, span) = err.location()?;
    let range = hir::denormalize(db, file, &span)?;
    let mut diagnostic = ide_diagnostic::diag()
        .range(range)
        .message(format!("internal compiler error: {err}"))
        .severity(DiagnosticSeverity::ERROR)
        .source("codegen".to_string())
        .call();
    diagnostic.with_note(format!(
        "the workspace passed `rk check`; this is a compiler bug or an \
         unimplemented construct; please report it at {ISSUES_URL}"
    ));

    let mut buffer: Vec<u8> = Vec::new();
    DiagnosticReporter::new(db, workspace)
        .with_format(format)
        .report_files(&[(file, vec![diagnostic])], &mut buffer);
    Some(String::from_utf8_lossy(&buffer).into_owned())
}

/// Check diagnostics and lower HIR → MIR → core WASM. On success returns the
/// core wasm + MIR; on failure returns the rendered diagnostics (also echoed to
/// stderr) so callers like the debugger can forward them over the debugger transport.
pub fn build_core(
    db: &RootDatabase,
    workspace: &std::path::Path,
    verbose: bool,
) -> Result<(Vec<u8>, mir::MirModule), String> {
    build_core_with_format(db, workspace, verbose, crate::cli::OutputFormat::Full)
}

/// [`build_core`] with an explicit diagnostics format — `rk compile` passes the
/// user's `--output-format`; the other callers (debug/sim/test) keep `full`.
pub fn build_core_with_format(
    db: &RootDatabase,
    workspace: &std::path::Path,
    verbose: bool,
    format: crate::cli::OutputFormat,
) -> Result<(Vec<u8>, mir::MirModule), String> {
    build_core_profile(db, workspace, verbose, format, wasm_codegen::Profile::Debug)
}

/// [`build_core_with_format`] choosing the artifact profile, the one place
/// the choice enters the build.
pub fn build_core_profile(
    db: &RootDatabase,
    workspace: &std::path::Path,
    _verbose: bool,
    format: crate::cli::OutputFormat,
    profile: wasm_codegen::Profile,
) -> Result<(Vec<u8>, mir::MirModule), String> {
    // Report into a buffer so the text can be both echoed to stderr (CLI
    // commands) and returned to the caller (the debugger forwards it over the debugger
    // transport — stdout there is the transport, and stderr isn't shown in VSCode).
    let per_file = collect_diagnostics(db, false);
    let reporter = DiagnosticReporter::new(db, workspace).with_format(format);
    let mut rendered: Vec<u8> = Vec::new();
    let total_errors = reporter.report_files(&per_file, &mut rendered).errors;

    // Echo to stderr for CLI usage; the debugger reads the returned string instead.
    let _ = std::io::stderr().write_all(&rendered);

    if total_errors > 0 {
        ui::failure(
            "compilation failed:",
            format!("{total_errors} error(s) found, cannot compile."),
        );
        let mut text = String::from_utf8_lossy(&rendered).into_owned();
        text.push_str(&format!(
            "\ncompilation failed: {total_errors} error(s) found, cannot compile.\n"
        ));
        return Err(text);
    }

    let sem_indices: Vec<_> = crate::file_order::ordered_files_with_libraries(db)
        .into_iter()
        .map(|file| semantic_index(db, file))
        .collect();

    let mir_module = match mir::lower::lower_module::lower_modules(db, &sem_indices) {
        Ok(m) => m,
        Err(e) => {
            // An ICE, not a user error (see `render_codegen_error`).
            return match render_codegen_error(db, workspace, &e, format) {
                Some(report) => {
                    let _ = std::io::stderr().write_all(report.as_bytes());
                    ui::failure("internal compiler error:", "cannot compile.");
                    Err(format!("{report}\ninternal compiler error: cannot compile.\n"))
                }
                None => {
                    ui::error(format!(
                        "internal compiler error: {e}; please report it at {ISSUES_URL}"
                    ));
                    Err(format!(
                        "internal compiler error: {e}; please report it at {ISSUES_URL}"
                    ))
                }
            };
        }
    };

    let wasm_module = wasm_codegen::generate_wasm_profile(db, &mir_module, profile);
    Ok((wasm_module.finish(), mir_module))
}

/// Like [`build_core`] but never writes to stderr or stdout; on failure
/// it returns the rendered diagnostics.
pub fn build_core_quiet(
    db: &RootDatabase,
    workspace: &std::path::Path,
    _verbose: bool,
) -> Result<(Vec<u8>, mir::MirModule), String> {
    let per_file = collect_diagnostics(db, false);
    let reporter = DiagnosticReporter::new(db, workspace);
    let mut rendered: Vec<u8> = Vec::new();
    let total_errors = reporter.report_files(&per_file, &mut rendered).errors;
    if total_errors > 0 {
        let mut text = String::from_utf8_lossy(&rendered).into_owned();
        text.push_str(&format!(
            "\ncompilation failed: {total_errors} error(s) found, cannot compile.\n"
        ));
        return Err(text);
    }

    let sem_indices: Vec<_> = crate::file_order::ordered_files_with_libraries(db)
        .into_iter()
        .map(|file| semantic_index(db, file))
        .collect();
    let mir_module =
        mir::lower::lower_module::lower_modules(db, &sem_indices).map_err(|e| {
            // Same ICE rendering as `build_core`, returned rather than printed.
            render_codegen_error(db, workspace, &e, crate::cli::OutputFormat::Full)
                .map(|report| format!("{report}\ninternal compiler error: cannot compile.\n"))
                .unwrap_or_else(|| {
                    format!("internal compiler error: {e}; please report it at {ISSUES_URL}")
                })
        })?;
    let wasm_module = wasm_codegen::generate_wasm(db, &mir_module);
    Ok((wasm_module.finish(), mir_module))
}

/// Where a build of `workspace` lands, by PROFILE — the only axis there is.
///
/// There are two artifacts, not five. the debugger, the runtime and `rk test`
/// all compile the same debug-profile core and differ only in which flag
/// the runtime is then launched with, so they used to write three private
/// copies of identical bytes under three names (`core.wasm`, `output.wasm`,
/// `serving.wasm`) in directories named after commands rather than profiles —
/// which left `rk_build/runtime/output.wasm` unable to say what profile it
/// even was.
///
/// Every command WRITES this fresh before handing the path to a runtime;
/// nothing reads it back expecting someone else to have filled it in. That
/// distinction is the whole lesson of the stale-artifact bug: writing your own
/// input is safe, trusting a file someone else may have written in June is not.
pub fn artifact_path(
    workspace: &std::path::Path,
    profile: wasm_codegen::Profile,
) -> std::path::PathBuf {
    let dir = match profile {
        wasm_codegen::Profile::Debug => "debug",
        wasm_codegen::Profile::Release => "release",
    };
    workspace.join("rk_build").join(dir).join("core.wasm")
}

/// The debug artifact — what every command that hands a program to a runtime
/// builds and passes along.
pub fn debug_core_path(workspace: &std::path::Path) -> std::path::PathBuf {
    artifact_path(workspace, wasm_codegen::Profile::Debug)
}

/// The post-MVP proposals this compiler's output uses. wasm-opt validates
/// against its own feature set and refuses a module using anything it
/// was not told about: bulk-memory (`memory.fill`), sign-ext,
/// exception-handling (`RAISE`), multivalue (STRING returns),
/// nontrapping-float-to-int.
const WASM_FEATURES: &[&str] = &[
    "bulk-memory",
    "sign-ext",
    "exception-handling",
    "multivalue",
    "nontrapping-float-to-int",
];

/// Run the optimizer over `wasm_bytes`, or return them unchanged.
///
/// The optimizer is an external binary — see [`crate::wasm_opt`] for how it is
/// found. A failure at any point keeps the unoptimized module, which is correct
/// but larger; it is never a reason to fail the build.
/// Optimize for a RELEASE artifact: wasm-opt is mandatory and its failure is
/// the build's failure.
///
/// The lenient [`optimize_wasm`] exists for `rk test -O`, where a missing
/// optimizer degrades to a correct-but-unoptimized run with a warning. A
/// release artifact is different: silently shipping the unoptimized build
/// while calling it a release would make "release" a hope rather than a
/// property. The check trusts the OUTPUT, not the exit status — Binaryen has
/// aborted with status 0 before, so the only thing believed is a wasm module
/// on disk.
pub fn optimize_wasm_release(
    wasm_bytes: Vec<u8>,
    opt_level: &str,
    verbose: bool,
) -> Result<Vec<u8>, String> {
    if !matches!(opt_level, "0" | "1" | "2" | "3" | "s" | "z") {
        // -O4 is refused: Binaryen's Flatten pass aborts on `try_table` up to
        // and including 131.
        return Err(format!(
            "invalid release optimization level '{opt_level}' (valid: 0-3, s, z)"
        ));
    }
    let original = wasm_bytes.clone();
    let optimized = optimize_wasm(wasm_bytes, Some(opt_level), verbose);
    // The lenient path signals every failure the same way: by returning the
    // input unchanged. For a release that signal becomes an error...
    if optimized == original {
        return Err(
            "wasm-opt did not produce an optimized module; a release build requires it.
                    Install Binaryen 119+ (CI pins 131) and ensure `wasm-opt` is on PATH."
                .to_string(),
        );
    }
    // ...and the output is verified as a module regardless of what the
    // process claimed.
    if !optimized.starts_with(b"\0asm") {
        return Err("the optimizer's output is not a WebAssembly module".to_string());
    }
    Ok(optimized)
}

pub fn optimize_wasm(wasm_bytes: Vec<u8>, opt_level: Option<&str>, verbose: bool) -> Vec<u8> {
    let Some(level) = opt_level else {
        return wasm_bytes;
    };
    if !matches!(level, "0" | "1" | "2" | "3" | "4" | "s" | "z") {
        ui::warn(format!(
            "unknown optimization level '{level}' (valid: 0-4, s, z)"
        ));
        return wasm_bytes;
    }
    let Some(wasm_opt) = crate::wasm_opt::find(verbose) else {
        ui::warn("could not optimize: no wasm-opt available. The build is correct but NOT optimized.");
        return wasm_bytes;
    };

    if verbose {
        ui::detail(format!("    Optimizing wasm-opt -O{level}"));
    }
    let original_size = wasm_bytes.len();

    // wasm-opt works on files, so the module makes a round trip through disk.
    // The scratch directory must be PRIVATE to this invocation: it used to be
    // two fixed names in the shared temp dir, so two `rk compile` runs on one
    // machine read and deleted each other's files — artifacts came back holding
    // the other workspace's program, or truncated, and the truncated ones were
    // still reported as a successful compile.
    let scratch = match tempfile::Builder::new().prefix("rk-wasm-opt-").tempdir() {
        Ok(dir) => dir,
        Err(e) => {
            ui::warn(format!("failed to create temp dir: {e}"));
            return wasm_bytes;
        }
    };
    let infile = scratch.path().join("in.wasm");
    let outfile = scratch.path().join("out.wasm");
    if let Err(e) = std::fs::write(&infile, &wasm_bytes) {
        ui::warn(format!("failed to write temp file: {e}"));
        return wasm_bytes;
    }

    let mut cmd = std::process::Command::new(&wasm_opt);
    for feature in WASM_FEATURES {
        cmd.arg(format!("--enable-{feature}"));
    }
    cmd.arg(format!("-O{level}"))
        .arg(&infile)
        .arg("-o")
        .arg(&outfile);
    match cmd.output() {
        Ok(out) if out.status.success() => {}
        Ok(out) => {
            // Binaryen's Flatten pass does not handle `try_table`, so `-O4`
            // aborts; report what it said.
            let stderr = String::from_utf8_lossy(&out.stderr);
            ui::warn(format!(
                "could not optimize: `wasm-opt -O{level}` failed: {}. The build is correct \
                 but NOT optimized.",
                stderr.lines().next().unwrap_or("no output").trim()
            ));
            return wasm_bytes;
        }
        Err(e) => {
            ui::warn(format!("could not run {}: {e}", wasm_opt.display()));
            return wasm_bytes;
        }
    }

    // Never ship what is not recognisably a wasm module: a partial output
    // degrades to the unoptimized bytes.
    let optimized = match std::fs::read(&outfile) {
        Ok(bytes) if bytes.starts_with(b"\0asm") => bytes,
        Ok(bytes) => {
            ui::warn(format!(
                "the optimizer produced {} bytes that are not a wasm module; keeping the \
                 unoptimized build",
                bytes.len()
            ));
            return wasm_bytes;
        }
        Err(e) => {
            ui::warn(format!(
                "could not read the optimizer's output ({e}); keeping the unoptimized build"
            ));
            return wasm_bytes;
        }
    };
    let optimized = preserve_load_bearing_sections(&wasm_bytes, optimized);

    if verbose {
        let savings = original_size as f64 - optimized.len() as f64;
        let pct = (savings / original_size as f64) * 100.0;
        ui::detail(format!(
            "    Optimized {} → {} bytes ({:.1}% reduction)",
            original_size,
            optimized.len(),
            pct
        ));
    }
    optimized
}

/// wasm-opt strips custom sections, so re-attach the ones a module cannot run
/// without.
///
/// Two are LOAD-BEARING, and neither is debug information despite sharing a
/// crate with it:
///
/// * `retain-map` — per-field RETAIN persistence. Losing it degrades the
///   runtime to legacy whole-band snapshots, the wrong IEC cold-start
///   semantics for non-retained state.
/// * `rk.schedule` — what runs, and when. Losing it leaves a module with
///   code and no statement of what to execute, so it cannot be scanned.
fn preserve_load_bearing_sections(original: &[u8], optimized: Vec<u8>) -> Vec<u8> {
    let mut out = optimized;
    for section in [
        debug_format::RETAIN_MAP_SECTION,
        debug_format::SCHEDULE_SECTION,
        // The monitoring tier survives optimization: symbols are addresses,
        // which wasm-opt does not relayout.
        mir::debug_symbols::DEBUG_SYMBOLS_SECTION,
        // And an optimized test build still has to know what to call.
        debug_format::test_manifest::TEST_MANIFEST_SECTION,
    ] {
        out = preserve_section(original, out, section);
    }
    out
}

/// Re-attach one custom section if `original` had it and optimization dropped it.
fn preserve_section(original: &[u8], optimized: Vec<u8>, section: &str) -> Vec<u8> {
    let find = |bytes: &[u8]| -> Option<Vec<u8>> {
        for payload in wasmparser::Parser::new(0).parse_all(bytes) {
            if let Ok(wasmparser::Payload::CustomSection(r)) = payload
                && r.name() == section
            {
                return Some(r.data().to_vec());
            }
        }
        None
    };
    let Some(data) = find(original) else {
        return optimized; // the module never had one
    };
    if find(&optimized).is_some() {
        return optimized; // survived optimization
    }
    // Append the custom section: id 0x00, LEB128 payload size, then
    // LEB128 name length + name + data. Appending at the end is valid wasm.
    let name = section.as_bytes();
    let mut payload = Vec::with_capacity(1 + name.len() + data.len());
    write_leb128(&mut payload, name.len() as u64);
    payload.extend_from_slice(name);
    payload.extend_from_slice(&data);
    let mut out = optimized;
    out.push(0x00);
    write_leb128(&mut out, payload.len() as u64);
    out.extend_from_slice(&payload);
    out
}

/// Unsigned LEB128 encoding (wasm's varint).
fn write_leb128(out: &mut Vec<u8>, mut v: u64) {
    loop {
        let mut b = (v & 0x7f) as u8;
        v >>= 7;
        if v != 0 {
            b |= 0x80;
        }
        out.push(b);
        if v == 0 {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::init_db;

    /// A diagnostic-clean workspace whose lowering still fails must present
    /// as an INTERNAL COMPILER ERROR, without a fabricated error count.
    #[test]
    fn hir_clean_lowering_failure_presents_as_ice() {
        let ws = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            ws.path().join("config.toml"),
            "[project]\nname = \"T\"\nversion = \"0.0\"\n",
        )
        .unwrap();
        std::fs::write(
            ws.path().join("main.st"),
            "PROGRAM Main\nVAR x : BOOL; END_VAR\n    x := %IX0.0;\nEND_PROGRAM\n\n\
             CONFIGURATION Cfg\n    RESOURCE Res ON CPU\n        \
             TASK T(INTERVAL := T#10ms, PRIORITY := 1);\n        \
             PROGRAM Run WITH T : Main;\n    END_RESOURCE\nEND_CONFIGURATION\n",
        )
        .unwrap();
        // Tests must not inherit the developer's library environment.
        unsafe { std::env::remove_var(db::loader::STDLIB_PATH_ENV) };
        let db = init_db(ws.path(), false, true).expect("init db");

        // Precondition: the workspace is diagnostic-clean (the ICE contract).
        let per_file = crate::diagnostics::collect_diagnostics(&db, false);
        assert!(
            per_file.iter().all(|(_, d)| d.is_empty()),
            "fixture must pass `rk check`"
        );

        let err = build_core(&db, ws.path(), false).expect_err("lowering must fail");
        assert!(
            err.contains("internal compiler error"),
            "presented as an ICE: {err}"
        );
        assert!(err.contains(ISSUES_URL), "carries the report-it URL: {err}");
        assert!(
            !err.contains("error(s) found"),
            "no fabricated diagnostic count: {err}"
        );
    }

    /// Two optimizations running at once must not read, overwrite or delete
    /// each other's scratch files. Each module carries a distinctly sized
    /// custom section, so a swap or a truncation shows as a wrong length.
    #[test]
    fn concurrent_optimizations_do_not_share_scratch_files() {
        fn module_with_padding(pad: usize) -> Vec<u8> {
            let mut module = vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];
            let name = b"rk-test-pad";
            let mut payload = Vec::new();
            write_leb128(&mut payload, name.len() as u64);
            payload.extend_from_slice(name);
            payload.extend(std::iter::repeat_n(0xAB, pad));
            module.push(0x00);
            write_leb128(&mut module, payload.len() as u64);
            module.extend_from_slice(&payload);
            module
        }

        let inputs: Vec<Vec<u8>> = (0..8).map(|i| module_with_padding(64 + i * 32)).collect();
        let handles: Vec<_> = inputs
            .iter()
            .map(|m| {
                let m = m.clone();
                std::thread::spawn(move || optimize_wasm(m, Some("4"), false))
            })
            .collect();
        let results: Vec<Vec<u8>> = handles.into_iter().map(|h| h.join().unwrap()).collect();

        for (i, out) in results.iter().enumerate() {
            assert!(
                out.starts_with(b"\0asm"),
                "thread {i} produced {} bytes that are not a wasm module",
                out.len()
            );
        }
        // wasm-opt drops custom sections, so compare against the
        // single-threaded answer for the same input.
        for (i, input) in inputs.iter().enumerate() {
            let expected = optimize_wasm(input.clone(), Some("4"), false);
            assert_eq!(
                results[i], expected,
                "thread {i}'s result differs from the same input optimized alone"
            );
        }
    }

    /// A module's load-bearing sections must survive release optimization.
    ///
    /// This exercises `preserve_load_bearing_sections` DIRECTLY rather than
    /// going through `optimize_wasm`: wasm-opt is an external binary, and when
    /// it is absent — as on any machine that has not installed Binaryen —
    /// optimization returns its input untouched, so a round-trip test passes
    /// without the re-attachment ever running. It was vacuous that way for
    /// `retain-map` before `rk.schedule` joined it.
    #[test]
    fn load_bearing_sections_are_reattached_after_stripping() {
        for section in [
            debug_format::RETAIN_MAP_SECTION,
            debug_format::SCHEDULE_SECTION,
        ] {
            let data = b"dummy-manifest-bytes";
            let original = module_with_section(section, data);
            // What wasm-opt hands back: the same module, custom sections gone.
            let stripped = vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];

            let restored = preserve_load_bearing_sections(&original, stripped);

            let mut found = None;
            for p in wasmparser::Parser::new(0).parse_all(&restored) {
                if let Ok(wasmparser::Payload::CustomSection(r)) = p
                    && r.name() == section
                {
                    found = Some(r.data().to_vec());
                }
            }
            assert_eq!(
                found.as_deref(),
                Some(data.as_slice()),
                "{section} must be re-attached after being stripped"
            );
        }
    }

    /// A minimal valid module carrying one custom section.
    fn module_with_section(section: &str, data: &[u8]) -> Vec<u8> {
        let mut module = vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];
        let name = section.as_bytes();
        let mut payload = Vec::new();
        write_leb128(&mut payload, name.len() as u64);
        payload.extend_from_slice(name);
        payload.extend_from_slice(data);
        module.push(0x00);
        write_leb128(&mut module, payload.len() as u64);
        module.extend_from_slice(&payload);
        module
    }
}
