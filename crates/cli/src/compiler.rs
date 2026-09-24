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
/// stderr) so a caller that serves another transport can forward them.
pub fn build_core(
    db: &RootDatabase,
    workspace: &std::path::Path,
    verbose: bool,
) -> Result<(Vec<u8>, mir::MirModule), String> {
    build_core_with_format(db, workspace, verbose, crate::cli::OutputFormat::Full)
}

/// [`build_core`] with an explicit diagnostics format — `rk compile` passes the
/// user's `--output-format`; the other callers keep `full`.
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
    // Report into a buffer, so the text can be echoed to stderr and returned
    // to the caller.
    let per_file = collect_diagnostics(db, false);
    let reporter = DiagnosticReporter::new(db, workspace).with_format(format);
    let mut rendered: Vec<u8> = Vec::new();
    let total_errors = reporter.report_files(&per_file, &mut rendered).errors;

    // Echo to stderr for CLI usage; a tool reads the returned string instead.
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

    // A library that does not check does not compile either: a broken
    // library lowers into an invalid module with no message naming the
    // cause.
    let lib_files = crate::diagnostics::collect_library_errors(db);
    if !lib_files.is_empty() {
        let mut lib_rendered: Vec<u8> = Vec::new();
        let lib_errors = reporter.report_files(&lib_files, &mut lib_rendered).errors;
        let _ = std::io::stderr().write_all(&lib_rendered);
        ui::failure(
            "compilation failed:",
            format!("{lib_errors} error(s) in library files, cannot compile."),
        );
        let mut text = String::from_utf8_lossy(&lib_rendered).into_owned();
        text.push_str(&format!(
            "\ncompilation failed: {lib_errors} error(s) in library files, cannot compile.\n"
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

    // Same library gate as `build_core_profile`.
    let lib_files = crate::diagnostics::collect_library_errors(db);
    if !lib_files.is_empty() {
        let mut lib_rendered: Vec<u8> = Vec::new();
        let lib_errors = reporter.report_files(&lib_files, &mut lib_rendered).errors;
        let mut text = String::from_utf8_lossy(&lib_rendered).into_owned();
        text.push_str(&format!(
            "\ncompilation failed: {lib_errors} error(s) in library files, cannot compile.\n"
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

/// Where a build of `workspace` lands, by profile, the only axis there
/// is. Every command writes this fresh before handing the path on;
/// nothing reads it back expecting someone else to have filled it in.
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

/// The debug artifact — what `rk test` runs and a debugger steps.
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

/// Optimize for a RELEASE artifact: wasm-opt is mandatory and its failure
/// is the build's failure (the lenient [`optimize_wasm`] serves
/// `rk test -O`). The check trusts the output, not the exit status.
pub fn optimize_wasm_release(
    wasm_bytes: Vec<u8>,
    opt_level: &str,
    verbose: bool,
) -> Result<Vec<u8>, String> {
    if !matches!(opt_level, "0" | "1" | "2" | "3" | "4" | "s" | "z") {
        return Err(format!(
            "invalid release optimization level '{opt_level}' (valid: 0-4, s, z)"
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
        let skipped = if level == "4" { " --skip-pass=flatten" } else { "" };
        ui::detail(format!("    Optimizing wasm-opt -O{level}{skipped}"));
    }
    let original_size = wasm_bytes.len();

    // wasm-opt works on files, so the module round-trips through a scratch
    // directory private to this invocation.
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
    cmd.arg(format!("-O{level}"));
    // -O4 is the level that runs Binaryen's Flatten pass, and Flatten does not
    // support `try_table`, the instruction that catches a raise: it aborts
    // (WebAssembly/binaryen#8372, no fix planned). Every module carries one,
    // the stdlib's test wrappers being compiled into all of them, so -O4
    // always runs with that pass skipped. The README is where this is said.
    if level == "4" {
        cmd.arg("--skip-pass=flatten");
    }
    cmd.arg(&infile).arg("-o").arg(&outfile);
    match cmd.output() {
        Ok(out) if out.status.success() => {}
        Ok(out) => {
            // Report what the optimizer said.
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

/// wasm-opt strips custom sections, so re-attach the load-bearing ones:
/// `retain-map` (per-field RETAIN persistence), `located-map` (which address
/// is which cell) and `rk.schedule` (what runs).
fn preserve_load_bearing_sections(original: &[u8], optimized: Vec<u8>) -> Vec<u8> {
    let mut out = optimized;
    for section in [
        debug_format::RETAIN_MAP_SECTION,
        debug_format::LOCATED_MAP_SECTION,
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
    fn a_lowering_failure_presents_as_an_ice_not_a_user_diagnostic() {
        // The failure is injected: every construct that used to pass `rk check`
        // and die in lowering has since been given its HIR refusal.
        let ws = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            ws.path().join("config.toml"),
            "[project]\nname = \"T\"\nversion = \"0.0\"\n",
        )
        .unwrap();
        std::fs::write(
            ws.path().join("main.st"),
            "FUNCTION f : INT\nVAR x : INT; END_VAR\n    f := x;\nEND_FUNCTION\n",
        )
        .unwrap();
        // Tests must not inherit the developer's library environment.
        unsafe { std::env::set_var(db::loader::STDLIB_PATH_ENV, "") };
        let db = init_db(ws.path(), false, true).expect("init db");

        // A located error, as lowering produces: the report must carry the
        // source excerpt and caret, not just a bare string.
        let (file, _) = crate::diagnostics::collect_diagnostics(&db, false)
            .into_iter()
            .next()
            .expect("one file");
        let span = auto_lsp::tree_sitter::Range {
            start_byte: 17,
            end_byte: 18,
            start_point: auto_lsp::tree_sitter::Point { row: 2, column: 9 },
            end_point: auto_lsp::tree_sitter::Point { row: 2, column: 10 },
        };
        let err = mir::lower::lower_type::LowerTypeError::UnsupportedType(
            "synthetic lowering failure".to_string(),
        )
        .with_location(file, span);

        let report = render_codegen_error(&db, ws.path(), &err, crate::cli::OutputFormat::Full)
            .expect("a located error renders");
        assert!(
            report.contains("internal compiler error"),
            "named as an ICE: {report}"
        );
        assert!(
            report.contains(ISSUES_URL),
            "tells the user where to report it: {report}"
        );
        assert!(
            report.contains("passed `rk check`"),
            "states the violated contract: {report}"
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

    /// The load-bearing sections must survive release optimization.
    /// Exercised through `preserve_load_bearing_sections` directly: without
    /// Binaryen installed, `optimize_wasm` returns its input untouched.
    #[test]
    fn load_bearing_sections_are_reattached_after_stripping() {
        for section in [
            debug_format::RETAIN_MAP_SECTION,
            debug_format::LOCATED_MAP_SECTION,
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
