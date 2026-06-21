//! Debug-symbol table tests: the `debug-symbols` custom section maps every
//! debuggable IEC variable (config/resource globals + program-instance fields,
//! down to elementary leaves) to its absolute linear-memory address. Programs
//! are instance-based, so a CONFIGURATION is needed to allocate an instance.

use crate::tests::{compile_to_mir_and_wasm, with_db};
use mir::debug_symbols::{DEBUG_SYMBOLS_SECTION, DEBUG_SYMBOLS_VERSION, DebugSymbols, SymType};
use rstest::*;
use runtime::debug::{DebugInfo, VarValue};
use runtime::{Config, Plc};

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

/// End-to-end monitoring: a `DebugInfo` (parsed from the binary) reads/writes a
/// running PLC's variables by qualified name through the Plc's address-based
/// memory access — the Plc itself stays name-agnostic.
#[rstest]
fn runtime_reads_and_writes_vars_by_name(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM Main
        VAR
            speed : INT;
            flag : BOOL;
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
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let mut plc = Plc::load(&wasm, Config::default()).expect("load PLC");
    let dbg = DebugInfo::from_wasm(&wasm);

    // The debug view exposes the same variables the section carries.
    let names: Vec<&str> = dbg.list_symbols().iter().map(|s| s.path.as_str()).collect();
    assert_eq!(names, vec!["Run.flag", "Run.speed", "g_count"]);
    assert!(
        dbg.read_var(&plc, "Run.nope").is_none(),
        "unknown path => None"
    );

    // After three scans, `speed := speed + 1` has run three times.
    plc.run(3).expect("scans");
    assert_eq!(dbg.read_var(&plc, "Run.speed"), Some(VarValue::I16(3)));
    assert_eq!(dbg.read_var(&plc, "Run.flag"), Some(VarValue::Bool(false)));

    // Force `speed` to 100; the next scan increments it to 101.
    dbg.write_var(&mut plc, "Run.speed", VarValue::I16(100))
        .expect("force speed");
    assert_eq!(dbg.read_var(&plc, "Run.speed"), Some(VarValue::I16(100)));
    plc.run(1).expect("scan");
    assert_eq!(dbg.read_var(&plc, "Run.speed"), Some(VarValue::I16(101)));

    // A config global is read/written by name the same way.
    dbg.write_var(&mut plc, "g_count", VarValue::I32(42))
        .expect("force global");
    assert_eq!(dbg.read_var(&plc, "g_count"), Some(VarValue::I32(42)));

    // Writing a value whose type doesn't match the symbol is rejected.
    assert!(
        dbg.write_var(&mut plc, "Run.speed", VarValue::Bool(true))
            .is_err()
    );
}

/// Aggregates: arrays expand to per-element leaves with IEC subscripts (1-D and
/// row-major N-D), enums/subranges emit one underlying-integer leaf; STRING is
/// still skipped (needs a length-prefix wire type).
#[rstest]
fn aggregate_symbols(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Color : (Red, Green, Blue); END_TYPE

        PROGRAM Main
        VAR
            arr  : ARRAY[1..3] OF INT;
            grid : ARRAY[0..1, 0..1] OF DINT;
            col  : Color;
            pct  : INT (0..100);
            name : STRING[10];
        END_VAR
            arr[1] := arr[1] + 1;
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM Run WITH T : Main;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let parsed = read_debug_symbols(&wasm);
    assert_eq!(
        parsed, mir.debug_symbols,
        "section round-trips the MIR table"
    );

    let by_path = |p: &str| parsed.symbols.iter().find(|s| s.path == p);

    // 1-D array → one INT leaf per IEC subscript (lower bound 1), 4 bytes apart.
    for p in ["Run.arr[1]", "Run.arr[2]", "Run.arr[3]"] {
        assert_eq!(
            by_path(p).unwrap_or_else(|| panic!("missing {p}")).ty,
            SymType::Int
        );
    }
    let a1 = by_path("Run.arr[1]").unwrap().address;
    assert_eq!(by_path("Run.arr[2]").unwrap().address, a1 + 4);
    assert_eq!(by_path("Run.arr[3]").unwrap().address, a1 + 8);
    assert!(by_path("Run.arr[0]").is_none(), "lower bound is 1, not 0");
    assert!(by_path("Run.arr[4]").is_none(), "upper bound is 3");

    // 2-D array → row-major subscripts: [0,1] is the 2nd element (+4), [1,0] the
    // 3rd (+8) — the rightmost dimension varies fastest.
    let g00 = by_path("Run.grid[0,0]").expect("grid[0,0]").address;
    assert_eq!(by_path("Run.grid[0,1]").unwrap().address, g00 + 4);
    assert_eq!(by_path("Run.grid[1,0]").unwrap().address, g00 + 8);
    assert_eq!(by_path("Run.grid[1,1]").unwrap().address, g00 + 12);
    for p in [
        "Run.grid[0,0]",
        "Run.grid[0,1]",
        "Run.grid[1,0]",
        "Run.grid[1,1]",
    ] {
        assert_eq!(by_path(p).unwrap().ty, SymType::DInt, "{p} type");
    }

    // Enum / subrange → one leaf of the underlying integer.
    assert_eq!(by_path("Run.pct").expect("subrange leaf").ty, SymType::Int);
    let col = by_path("Run.col").expect("enum leaf");
    assert!(
        matches!(col.ty, SymType::SInt | SymType::Int | SymType::DInt),
        "enum stored as an integer, got {:?}",
        col.ty
    );

    // STRING → one leaf carrying its capacity; slot is 4 (len) + capacity bytes.
    let name = by_path("Run.name").expect("STRING leaf");
    assert_eq!(name.ty, SymType::String { capacity: 10 });
    assert_eq!(name.size, 4 + 10);
}

/// A STRING variable round-trips by name through the address-based memory: force
/// a value, read it back; the length-prefixed buffer decodes to a Rust String,
/// and writes are capacity-bounded.
#[rstest]
fn runtime_reads_writes_string_by_name(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM Main
        VAR
            label : STRING[8];
            n : INT;
        END_VAR
            n := n + 1;
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM Run WITH T : Main;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let mut plc = Plc::load(&wasm, Config::default()).expect("load PLC");
    let dbg = DebugInfo::from_wasm(&wasm);

    // Zero-initialised buffer ⇒ empty string.
    assert_eq!(
        dbg.read_var(&plc, "Run.label"),
        Some(VarValue::String(String::new()))
    );

    // Force a value and read it back.
    dbg.write_var(&mut plc, "Run.label", VarValue::String("hi".into()))
        .expect("force string");
    assert_eq!(
        dbg.read_var(&plc, "Run.label"),
        Some(VarValue::String("hi".into()))
    );

    // Capacity-bounded (STRING[8]): a longer write truncates to 8 bytes.
    dbg.write_var(&mut plc, "Run.label", VarValue::String("0123456789".into()))
        .expect("force long string");
    assert_eq!(
        dbg.read_var(&plc, "Run.label"),
        Some(VarValue::String("01234567".into()))
    );

    // A type mismatch is still rejected.
    assert!(
        dbg.write_var(&mut plc, "Run.label", VarValue::I16(1))
            .is_err()
    );
}

/// `read_all` snapshots every monitorable variable's current value in one call —
/// the watch/trace bulk read, no engine or breakpoint involved.
#[rstest]
fn read_all_snapshots_all_variables(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM Main
        VAR
            speed : INT;
            flag : BOOL;
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
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let mut plc = Plc::load(&wasm, Config::default()).expect("load PLC");
    let dbg = DebugInfo::from_wasm(&wasm);

    plc.run(2).expect("scans");

    let snap: std::collections::HashMap<&str, VarValue> = dbg.read_all(&plc).into_iter().collect();
    assert_eq!(snap.len(), dbg.list_symbols().len(), "one value per symbol");
    assert_eq!(snap["Run.speed"], VarValue::I16(2));
    assert_eq!(snap["Run.flag"], VarValue::Bool(false));
    assert_eq!(snap["g_count"], VarValue::I32(0));
}
