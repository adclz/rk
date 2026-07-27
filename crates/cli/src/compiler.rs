use std::io::Write;

use auto_lsp::default::db::BaseDatabase;
use db::{RootDatabase, WorkspaceDataBase};
use hir::hir_def::semantic_index::semantic_index;

use crate::diagnostics::{DiagnosticReporter, collect_diagnostics};
use crate::ui;

/// Render a codegen failure the same way a type error is rendered — source
/// excerpt, caret, file:line — when the error carries a location.
///
/// Lowering pins its errors to the offending expression (or, failing that, the
/// POU declaration), so an unsupported construct points at the construct.
/// Errors raised before any location is known still fall back to the bare
/// message.
fn render_codegen_error(
    db: &RootDatabase,
    workspace: &std::path::Path,
    err: &mir::lower::lower_type::LowerTypeError,
) -> Option<String> {
    use auto_lsp::lsp_types::DiagnosticSeverity;

    let (file, span) = err.location()?;
    let range = hir::denormalize(db, file, &span)?;
    let diagnostic = ide_diagnostic::diag()
        .range(range)
        .message(format!("{err}"))
        .severity(DiagnosticSeverity::ERROR)
        .source("codegen".to_string())
        .call();

    let mut buffer: Vec<u8> = Vec::new();
    DiagnosticReporter::new(db, workspace).report_files(&[(file, vec![diagnostic])], &mut buffer);
    Some(String::from_utf8_lossy(&buffer).into_owned())
}

/// Check diagnostics and lower HIR → MIR → core WASM. On success returns the
/// core wasm + MIR; on failure returns the rendered diagnostics (also echoed to
/// stderr) so callers like the debugger can forward them over the debugger transport.
pub fn build_core(
    db: &RootDatabase,
    workspace: &std::path::Path,
    _verbose: bool,
) -> Result<(Vec<u8>, mir::MirModule), String> {
    // Report into a buffer so the text can be both echoed to stderr (CLI
    // commands) and returned to the caller (the debugger forwards it over the debugger
    // transport — stdout there is the transport, and stderr isn't shown in VSCode).
    let per_file = collect_diagnostics(db, false);
    let reporter = DiagnosticReporter::new(db, workspace);
    let mut rendered: Vec<u8> = Vec::new();
    let (total_errors, _total_warnings) = reporter.report_files(&per_file, &mut rendered);

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

    let sem_indices: Vec<_> = db
        .get_files()
        .iter()
        .chain(db.get_std_lib_files().iter())
        .map(|file| semantic_index(db, *file))
        .collect();

    let mir_module = match mir::lower::lower_module::lower_modules(db, &sem_indices) {
        Ok(m) => m,
        Err(e) => {
            // Prefer the located rendering (source excerpt + caret); fall back
            // to the bare message when the error carries no location.
            return match render_codegen_error(db, workspace, &e) {
                Some(report) => {
                    let _ = std::io::stderr().write_all(report.as_bytes());
                    ui::failure("compilation failed:", "1 error(s) found, cannot compile.");
                    Err(format!(
                        "{report}\ncompilation failed: 1 error(s) found, cannot compile.\n"
                    ))
                }
                None => {
                    ui::error(format!("codegen: {e}"));
                    Err(format!("codegen error: {e}"))
                }
            };
        }
    };

    let wasm_module = wasm_codegen::generate_wasm(db, &mir_module);
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
    let (total_errors, _total_warnings) = reporter.report_files(&per_file, &mut rendered);
    if total_errors > 0 {
        let mut text = String::from_utf8_lossy(&rendered).into_owned();
        text.push_str(&format!(
            "\ncompilation failed: {total_errors} error(s) found, cannot compile.\n"
        ));
        return Err(text);
    }

    let sem_indices: Vec<_> = db
        .get_files()
        .iter()
        .chain(db.get_std_lib_files().iter())
        .map(|file| semantic_index(db, *file))
        .collect();
    let mir_module =
        mir::lower::lower_module::lower_modules(db, &sem_indices).map_err(|e| {
            // Same located rendering as `build_core`, returned (never printed)
            // so the TUI can show it after leaving the alternate screen.
            render_codegen_error(db, workspace, &e)
                .map(|report| {
                    format!("{report}\ncompilation failed: 1 error(s) found, cannot compile.\n")
                })
                .unwrap_or_else(|| format!("codegen error: {e}"))
        })?;
    let wasm_module = wasm_codegen::generate_wasm(db, &mir_module);
    Ok((wasm_module.finish(), mir_module))
}

/// Path of the debug **core** artifact: `rk compile --debug` writes it and
/// the debugger loads it.
pub fn debug_core_path(workspace: &std::path::Path) -> std::path::PathBuf {
    workspace.join("rk_build").join("debug").join("core.wasm")
}

/// Run wasm-opt on the WASM bytes if an optimization level is specified.
pub fn optimize_wasm(wasm_bytes: Vec<u8>, opt_level: Option<&str>, verbose: bool) -> Vec<u8> {
    let Some(level) = opt_level else {
        return wasm_bytes;
    };

    let mut opts = match level {
        "0" => wasm_opt::OptimizationOptions::new_opt_level_0(),
        "1" => wasm_opt::OptimizationOptions::new_opt_level_1(),
        "2" => wasm_opt::OptimizationOptions::new_opt_level_2(),
        "3" => wasm_opt::OptimizationOptions::new_opt_level_3(),
        "4" => wasm_opt::OptimizationOptions::new_opt_level_4(),
        "s" => wasm_opt::OptimizationOptions::new_optimize_for_size(),
        "z" => wasm_opt::OptimizationOptions::new_optimize_for_size_aggressively(),
        _ => {
            ui::warn(format!(
                "unknown optimization level '{level}' (valid: 0-4, s, z)"
            ));
            return wasm_bytes;
        }
    };

    // Enable the post-MVP proposals the codegen actually emits. Binaryen
    // validates against its own feature set first, and with the MVP default it
    // rejects the whole module — silently falling back to UNOPTIMIZED output.
    // `bulk-memory` is required by the `memory.fill` that resets aggregate
    // VAR_TEMP on entry; `sign-ext` by the sub-width normalization
    // (`i32.extend8_s`/`extend16_s`); `exception-handling` by `__RAISE`.
    opts.enable_feature(wasm_opt::Feature::BulkMemory)
        .enable_feature(wasm_opt::Feature::SignExt)
        .enable_feature(wasm_opt::Feature::ExceptionHandling);

    if verbose {
        ui::detail(format!("    Optimizing wasm-opt -O{level}"));
    }

    let original_size = wasm_bytes.len();

    // wasm-opt works on files, so the module makes a round trip through disk.
    //
    // The scratch directory must be PRIVATE to this invocation. It used to be
    // two fixed names in the shared temp dir, so two `rk compile` runs on one
    // machine read and deleted each other's files: artifacts came back holding
    // the other workspace's program, or truncated, or silently unoptimized -
    // and the truncated ones were still reported as a successful compile.
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

    match opts.run(&infile, &outfile) {
        Ok(()) => {
            // Never ship what we cannot recognise as a wasm module: a partial
            // or absent output degrades to the unoptimized bytes rather than
            // being written out as the artifact.
            let optimized = match std::fs::read(&outfile) {
                Ok(bytes) if bytes.starts_with(b"\0asm") => bytes,
                Ok(bytes) => {
                    ui::warn(format!(
                        "wasm-opt produced {} bytes that are not a wasm module; keeping the \
                         unoptimized build",
                        bytes.len()
                    ));
                    return wasm_bytes;
                }
                Err(e) => {
                    ui::warn(format!(
                        "could not read wasm-opt output ({e}); keeping the unoptimized build"
                    ));
                    return wasm_bytes;
                }
            };
            let optimized = preserve_retain_map(&wasm_bytes, optimized);

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
        Err(e) => {
            ui::warn(format!("wasm-opt failed: {e}"));
            wasm_bytes
        }
    }
}

/// The `retain-map` custom section is LOAD-BEARING (per-field RETAIN
/// persistence — without it the runtime degrades to legacy whole-band
/// snapshots with wrong IEC cold-start semantics). wasm-opt strips custom
/// sections, so re-attach it to the optimized module if it got dropped.
fn preserve_retain_map(original: &[u8], optimized: Vec<u8>) -> Vec<u8> {
    let find = |bytes: &[u8]| -> Option<Vec<u8>> {
        for payload in wasmparser::Parser::new(0).parse_all(bytes) {
            if let Ok(wasmparser::Payload::CustomSection(r)) = payload
                && r.name() == debug_format::RETAIN_MAP_SECTION
            {
                return Some(r.data().to_vec());
            }
        }
        None
    };
    let Some(data) = find(original) else {
        return optimized; // module has no retained state
    };
    if find(&optimized).is_some() {
        return optimized; // survived optimization
    }
    // Append the custom section: id 0x00, LEB128 payload size, then
    // LEB128 name length + name + data. Appending at the end is valid wasm.
    let name = debug_format::RETAIN_MAP_SECTION.as_bytes();
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
            .cloned()
            .map(|m| std::thread::spawn(move || optimize_wasm(m, Some("4"), false)))
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

    /// A minimal module carrying a retain-map section must still carry it
    /// after release optimization (wasm-opt strips custom sections; we
    /// re-attach).
    #[test]
    fn retain_map_survives_wasm_opt() {
        // Minimal valid module: magic + version, plus the retain-map custom
        // section with dummy payload bytes.
        let mut module = vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];
        let name = debug_format::RETAIN_MAP_SECTION.as_bytes();
        let data = b"dummy-manifest-bytes";
        let mut payload = Vec::new();
        write_leb128(&mut payload, name.len() as u64);
        payload.extend_from_slice(name);
        payload.extend_from_slice(data);
        module.push(0x00);
        write_leb128(&mut module, payload.len() as u64);
        module.extend_from_slice(&payload);

        let optimized = optimize_wasm(module, Some("s"), false);

        let mut found = None;
        for p in wasmparser::Parser::new(0).parse_all(&optimized) {
            if let Ok(wasmparser::Payload::CustomSection(r)) = p
                && r.name() == debug_format::RETAIN_MAP_SECTION
            {
                found = Some(r.data().to_vec());
            }
        }
        assert_eq!(
            found.as_deref(),
            Some(data.as_slice()),
            "retain-map section must survive optimization"
        );
    }
}
