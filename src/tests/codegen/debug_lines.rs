//! `debug-lines`: per-function (within-body offset → source position). Verifies
//! codegen records each statement's source line, and that `DebugInfo` maps an
//! absolute wasm `pc` back to it (the offset crux confirmed by the runtime's
//! `framehandle_spike::wasm_pc_is_absolute_operator_offset`).

use crate::tests::codegen::{add_source, compile_to_mir_and_wasm, with_db};
use hir::hir_def::semantic_index::semantic_index;
use rstest::*;
use runtime::debug::DebugInfo;

fn read_debug_lines(wasm: &[u8]) -> debug_format::DebugLines {
    for payload in wasmparser::Parser::new(0).parse_all(wasm) {
        if let Ok(wasmparser::Payload::CustomSection(reader)) = payload
            && reader.name() == debug_format::DEBUG_LINES_SECTION
        {
            return debug_format::DebugLines::from_msgpack(reader.data())
                .expect("valid debug-lines section");
        }
    }
    panic!("module is missing the `debug-lines` custom section");
}

/// The start offset (in the binary) of defined function `idx`'s code body.
fn body_start(wasm: &[u8], idx: u32) -> u32 {
    let mut i = 0;
    for payload in wasmparser::Parser::new(0).parse_all(wasm) {
        if let Ok(wasmparser::Payload::CodeSectionEntry(body)) = payload {
            if i == idx {
                return body.range().start as u32;
            }
            i += 1;
        }
    }
    panic!("no code body for defined index {idx}");
}

/// 0-based source line of the first occurrence of `needle` in `src`.
fn row_of(src: &str, needle: &str) -> u32 {
    let byte = src
        .find(needle)
        .unwrap_or_else(|| panic!("`{needle}` not in source"));
    src[..byte].bytes().filter(|&b| b == b'\n').count() as u32
}

/// Each statement gets a line entry at the source row it came from, offsets are
/// strictly increasing, and `DebugInfo` maps a synthesized absolute `wasm_pc`
/// (`body_start + offset`) back to the right line.
#[rstest]
fn statements_map_to_source_lines(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION calc : INT
        VAR_INPUT x : INT; END_VAR
        VAR y : INT; END_VAR
            y := x + 1;
            y := y * 2;
            calc := y - 3;
        END_FUNCTION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);

    let dl = read_debug_lines(&wasm);
    assert_eq!(dl.version, debug_format::DEBUG_LINES_VERSION);
    // `calc` is the only function with statements (no config/builtins here).
    assert_eq!(dl.functions.len(), 1, "only calc has a line table");
    let table = &dl.functions[0];
    let calc_idx = table.defined_index;

    // Offsets strictly increasing.
    assert!(
        table.lines.windows(2).all(|w| w[0].offset < w[1].offset),
        "offsets must be sorted + unique: {:?}",
        table.lines
    );

    // The three statements map to their source rows, in order.
    let got: Vec<u32> = table.lines.iter().map(|e| e.line).collect();
    assert_eq!(
        got,
        vec![
            row_of(source, "y := x + 1"),
            row_of(source, "y := y * 2"),
            row_of(source, "calc := y - 3"),
        ],
        "statement source lines"
    );

    // DebugInfo maps a real absolute wasm_pc (body_start + offset) → the line.
    let dbg = DebugInfo::from_wasm(&wasm);
    let bstart = body_start(&wasm, calc_idx);
    for e in &table.lines {
        let pos = dbg
            .source_position(calc_idx, bstart + e.offset)
            .expect("source position");
        assert_eq!(pos.line, e.line, "pc at offset {} → line", e.offset);
        assert_eq!(pos.col, e.col);
    }

    // A pc mid-statement (just past the first entry) still resolves to that
    // statement — the lookup is "largest offset <= within".
    let mid = bstart + table.lines[0].offset + 1;
    assert_eq!(
        dbg.source_position(calc_idx, mid).unwrap().line,
        table.lines[0].line
    );

    // A pc before the first statement (or an unknown function) → None.
    assert!(dbg.source_position(calc_idx, 0).is_none());
    assert!(dbg.source_position(9999, bstart).is_none());
}

/// `line_to_pc` (the reverse lookup a debugger uses to set a breakpoint by
/// source line) yields a wasm pc that resolves back to the same line — so
/// "break at file:line" needs no codegen, just the debug-lines table.
#[rstest]
fn line_to_pc_round_trips(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION calc : INT
        VAR_INPUT x : INT; END_VAR
        VAR y : INT; END_VAR
            y := x + 1;
            y := y * 2;
            calc := y - 3;
        END_FUNCTION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let dbg = DebugInfo::from_wasm(&wasm);

    for stmt in ["y := x + 1", "y := y * 2", "calc := y - 3"] {
        let line = row_of(source, stmt);
        let (defined, pc) = dbg
            .line_to_pc(0, line)
            .unwrap_or_else(|| panic!("no breakpoint pc for line {line} (`{stmt}`)"));
        // The breakpoint pc resolves back to the same source line.
        assert_eq!(
            dbg.source_position(defined, pc).map(|p| p.line),
            Some(line),
            "line_to_pc → source_position round-trip for `{stmt}`"
        );
    }

    // A line with no code (the blank first line) has no breakpoint location.
    assert!(dbg.line_to_pc(0, 0).is_none());
}

/// With several functions plus a CONFIGURATION (so the synthesized `__init`
/// and builtins share the module and the defined-index base is non-trivial),
/// `debug-functions` and `debug-lines` must agree on each function's
/// `DefinedFuncIndex`: the function whose body holds a statement is the one
/// named for that index, and its breakpoint pc round-trips to the statement's
/// row. A mismatched `widx − n_func_imports` base between the two sections would
/// resolve a statement to the wrong (or unnamed) function — a silent corruption
/// the single-function tests above cannot catch.
#[rstest]
fn functions_and_lines_agree_on_index(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION add : INT
        VAR_INPUT a : INT; b : INT; END_VAR
            add := a + b;
        END_FUNCTION

        FUNCTION mul : INT
        VAR_INPUT a : INT; b : INT; END_VAR
            mul := a * b;
        END_FUNCTION

        PROGRAM Main
        VAR count : INT; END_VAR
            count := add(a := count, b := mul(a := 2, b := 3));
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM Run WITH T : Main;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let dbg = DebugInfo::from_wasm(&wasm);

    // The function whose body holds each statement is the one named for it, and
    // the breakpoint pc resolves back to the statement's row.
    for (needle, stmt) in [
        ("add", "add := a + b"),
        ("mul", "mul := a * b"),
        ("Main", "count := add(a := count"),
    ] {
        let row = row_of(source, stmt);
        let (idx, pc) = dbg
            .line_to_pc(0, row)
            .unwrap_or_else(|| panic!("no breakpoint pc for `{stmt}` (row {row})"));
        let name = dbg
            .function_name(idx)
            .unwrap_or_else(|| panic!("`{stmt}` resolved to unnamed defined index {idx}"));
        assert!(
            name.contains(needle),
            "`{stmt}` (row {row}) resolved to fn `{name}` at idx {idx}, expected `{needle}` — \
             debug-functions and debug-lines disagree on the defined-index base"
        );
        assert_eq!(
            dbg.source_position(idx, pc).map(|p| p.line),
            Some(row),
            "breakpoint pc for `{stmt}` must resolve back to row {row}"
        );
    }
}

/// Two files, each with a function whose key statement sits on the SAME source
/// row. Multi-file debug-lines must disambiguate them by `file`: a breakpoint at
/// "file A : row" resolves to A's function, "file B : row" to B's — exactly the
/// stdlib collision the old hardcoded `file_id = 0` got wrong.
#[rstest]
fn breakpoints_resolve_per_file(mut with_db: db::RootDatabase) {
    // `fa := x + 1` and `fb := x + 2` are both on row 3 of their respective files.
    let src_a = r#"
        FUNCTION fa : INT
        VAR_INPUT x : INT; END_VAR
            fa := x + 1;
        END_FUNCTION
    "#;
    let src_b = r#"
        FUNCTION fb : INT
        VAR_INPUT x : INT; END_VAR
            fb := x + 2;
        END_FUNCTION
    "#;
    let file_a = add_source(&mut with_db, src_a);
    let file_b = add_source(&mut with_db, src_b);
    let idx_a = semantic_index(&with_db, file_a);
    let idx_b = semantic_index(&with_db, file_b);
    let module = mir::lower::lower_module::lower_modules(&with_db, &[idx_a, idx_b])
        .expect("multi-file lowering");
    let wasm = wasm_codegen::generate_wasm(&with_db, &module).finish();
    let dbg = DebugInfo::from_wasm(&wasm);

    // Both sources appear in the files table; find each file's index by URL.
    let url_a = file_a.url(&with_db).as_str().to_string();
    let url_b = file_b.url(&with_db).as_str().to_string();
    let files = dbg.source_files();
    assert_eq!(files.len(), 2, "two source files, got {files:?}");
    let fi_a = files
        .iter()
        .position(|f| f == &url_a)
        .expect("file A in table") as u32;
    let fi_b = files
        .iter()
        .position(|f| f == &url_b)
        .expect("file B in table") as u32;
    assert_ne!(fi_a, fi_b);

    let row = row_of(src_a, "fa := x + 1");
    assert_eq!(
        row,
        row_of(src_b, "fb := x + 2"),
        "test relies on identical rows"
    );

    // Same row, different file ⇒ different function.
    let (ia, pa) = dbg.line_to_pc(fi_a, row).expect("breakpoint in file A");
    assert!(
        dbg.function_name(ia).unwrap().contains("fa"),
        "file A row → fa"
    );
    assert_eq!(
        dbg.source_position(ia, pa).map(|p| (p.file, p.line)),
        Some((fi_a, row))
    );

    let (ib, pb) = dbg.line_to_pc(fi_b, row).expect("breakpoint in file B");
    assert!(
        dbg.function_name(ib).unwrap().contains("fb"),
        "file B row → fb"
    );
    assert_eq!(
        dbg.source_position(ib, pb).map(|p| (p.file, p.line)),
        Some((fi_b, row))
    );

    assert_ne!(ia, ib, "the two files' functions are distinct");
}
