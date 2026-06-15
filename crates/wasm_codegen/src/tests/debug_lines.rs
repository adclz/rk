//! `debug-lines`: per-function (within-body offset → source position). Verifies
//! codegen records each statement's source line, and that `DebugInfo` maps an
//! absolute wasm `pc` back to it (the offset crux confirmed by the runtime's
//! `framehandle_spike::wasm_pc_is_absolute_operator_offset`).

use crate::tests::{compile_to_mir_and_wasm, with_db};
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
