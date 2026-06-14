//! Debug-symbol table tests: the `debug-symbols` custom section maps every
//! debuggable IEC variable (config/resource globals + program-instance fields,
//! down to elementary leaves) to its absolute linear-memory address. Programs
//! are instance-based, so a CONFIGURATION is needed to allocate an instance.

use crate::tests::{compile_to_mir_and_wasm, with_db};
use mir::debug_symbols::{DEBUG_SYMBOLS_SECTION, DEBUG_SYMBOLS_VERSION, DebugSymbols, SymType};
use rstest::*;

/// Builtin shadow-stack + data live below this; no IEC variable may sit lower.
const BUILTIN_RESERVED_FLOOR: u32 = 16_384;

/// Extract and decode the `debug-symbols` custom section from a core module.
fn read_debug_symbols(wasm: &[u8]) -> DebugSymbols {
    for payload in wasmparser::Parser::new(0).parse_all(wasm) {
        if let Ok(wasmparser::Payload::CustomSection(reader)) = payload
            && reader.name() == DEBUG_SYMBOLS_SECTION
        {
            return DebugSymbols::from_msgpack(reader.data()).expect("valid debug-symbols section");
        }
    }
    panic!("module is missing the `{DEBUG_SYMBOLS_SECTION}` custom section");
}

/// Elementary program vars (incl. a nested FB field) and a config global are
/// each emitted as a typed symbol at a stable address; nested fields get dotted
/// paths, and the section round-trips to what the MIR built.
#[rstest]
fn elementary_program_and_global_symbols(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Motor
        VAR
            rpm : DINT;
        END_VAR
        END_FUNCTION_BLOCK

        PROGRAM Main
        VAR
            speed : INT;
            flag : BOOL;
            motor : Motor;
        END_VAR
            speed := speed + 1;
        END_PROGRAM

        CONFIGURATION Cfg
            VAR_GLOBAL
                g_count : DINT;
            END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM Run WITH T : Main;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);

    // The embedded section round-trips byte-for-byte to what the MIR built.
    let parsed = read_debug_symbols(&wasm);
    assert_eq!(parsed, mir.debug_symbols);
    assert_eq!(parsed.version, DEBUG_SYMBOLS_VERSION);

    // Exactly the elementary leaves we expect, with their types, sorted by path.
    // `motor` (a nested FB) is traversed, not emitted, yielding `Run.motor.rpm`.
    let got: Vec<(&str, SymType)> = parsed
        .symbols
        .iter()
        .map(|s| (s.path.as_str(), s.ty))
        .collect();
    assert_eq!(
        got,
        vec![
            ("Run.flag", SymType::Bool),
            ("Run.motor.rpm", SymType::DInt),
            ("Run.speed", SymType::Int),
            ("g_count", SymType::DInt),
        ]
    );

    // Sizes track the elementary type (BOOL/INT/DINT are all 4 bytes here).
    for s in &parsed.symbols {
        assert_eq!(s.size, 4, "{} size", s.path);
    }

    // Program fields fall inside their instance's allocated state; the global
    // lands in the exported globals band; all addresses are real and distinct.
    let by_path = |p: &str| parsed.symbols.iter().find(|s| s.path == p).unwrap();
    let inst = &mir.schedule.as_ref().unwrap().tasks[0].programs[0];
    let base = inst.instance_addr;
    for p in ["Run.speed", "Run.flag", "Run.motor.rpm"] {
        let a = by_path(p).address;
        assert!(a >= base, "{p} addr {a} below instance base {base}");
        assert!(a >= BUILTIN_RESERVED_FLOOR);
    }
    let g = by_path("g_count").address;
    assert!(
        g >= mir.globals_base && g < mir.globals_base + mir.globals_size,
        "g_count addr {g} outside globals band [{}, {})",
        mir.globals_base,
        mir.globals_base + mir.globals_size
    );

    let mut addrs: Vec<u32> = parsed.symbols.iter().map(|s| s.address).collect();
    let n = addrs.len();
    addrs.sort_unstable();
    addrs.dedup();
    assert_eq!(addrs.len(), n, "symbol addresses must be distinct");
}

/// A module with no CONFIGURATION (no program instances, no globals) still emits
/// a well-formed, empty `debug-symbols` section — the runtime can always read it.
#[rstest]
fn no_config_emits_empty_symbol_table(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION add : INT
        VAR_INPUT a : INT; b : INT; END_VAR
            add := a + b;
        END_FUNCTION
    "#;
    let (mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let parsed = read_debug_symbols(&wasm);
    assert_eq!(parsed, mir.debug_symbols);
    assert!(
        parsed.symbols.is_empty(),
        "no instances/globals => no symbols, got {:?}",
        parsed.symbols
    );
    assert_eq!(parsed.version, DEBUG_SYMBOLS_VERSION);
}
