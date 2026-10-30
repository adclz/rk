// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! Debug-symbol table tests: the `debug-symbols` custom section maps every
//! debuggable IEC variable (config/resource globals + program-instance fields,
//! down to elementary leaves) to its absolute linear-memory address. Programs
//! are instance-based, so a CONFIGURATION is needed to allocate an instance.

use crate::tests::codegen::{compile_to_mir_and_wasm, with_db};
use debug_format::{DebugInfo, VarValue};
use mir::debug_symbols::{DEBUG_SYMBOLS_SECTION, DEBUG_SYMBOLS_VERSION, DebugSymbols, SymType};
use rstest::*;

use crate::tests::codegen::TestPlc;

/// Builtin shadow-stack + data live below this; no IEC variable may sit lower.
const BUILTIN_RESERVED_FLOOR: u32 = 16_384;

/// Extract and decode the `debug-symbols` custom section from a core module.
fn read_debug_symbols(wasm: &[u8]) -> DebugSymbols {
    let section = super::expect_section(wasm, DEBUG_SYMBOLS_SECTION);
    DebugSymbols::from_msgpack(section).expect("valid debug-symbols section")
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
    let mut plc = TestPlc::load(&wasm).expect("load PLC");
    let dbg = DebugInfo::from_wasm(&wasm);

    // The debug view exposes the same variables the section carries.
    let names: Vec<&str> = dbg.list_symbols().iter().map(|s| s.path.as_str()).collect();
    assert_eq!(names, vec!["Run.flag", "Run.speed", "g_count"]);
    assert!(
        plc.read_var(&dbg, "Run.nope").is_none(),
        "unknown path => None"
    );

    // After three scans, `speed := speed + 1` has run three times.
    plc.run(3).expect("scans");
    assert_eq!(plc.read_var(&dbg, "Run.speed"), Some(VarValue::I16(3)));
    assert_eq!(plc.read_var(&dbg, "Run.flag"), Some(VarValue::Bool(false)));

    // Force `speed` to 100; the next scan increments it to 101.
    plc.write_var(&dbg, "Run.speed", VarValue::I16(100))
        .expect("force speed");
    assert_eq!(plc.read_var(&dbg, "Run.speed"), Some(VarValue::I16(100)));
    plc.run(1).expect("scan");
    assert_eq!(plc.read_var(&dbg, "Run.speed"), Some(VarValue::I16(101)));

    // A config global is read/written by name the same way.
    plc.write_var(&dbg, "g_count", VarValue::I32(42))
        .expect("force global");
    assert_eq!(plc.read_var(&dbg, "g_count"), Some(VarValue::I32(42)));

    // Writing a value whose type doesn't match the symbol is rejected.
    assert!(
        plc.write_var(&dbg, "Run.speed", VarValue::Bool(true))
            .is_err()
    );
}

/// Aggregates: an array is one descriptor, its elements resolved on demand
/// by IEC subscript (1-D and row-major N-D); enums/subranges emit one
/// underlying-integer leaf; a STRING is one leaf carrying its capacity.
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

    // Arrays contribute NO leaves — the descriptor is the whole record, and
    // `locate` computes any element from it. 3 elements or 5000, the section
    // costs the same.
    assert!(
        !parsed.symbols.iter().any(|s| s.path.contains('[')),
        "no per-element leaves: {:?}",
        parsed.symbols.iter().map(|s| &s.path).collect::<Vec<_>>()
    );
    let arr = parsed
        .arrays
        .iter()
        .find(|a| a.path == "Run.arr")
        .expect("the 1-D descriptor");
    assert_eq!((arr.total_elements, arr.elem_size), (3, 4));
    assert_eq!(arr.dimensions, vec![(1, 3)]);

    // ...and resolution honours IEC bounds, 4 bytes apart (lower bound 1).
    let dbg = DebugInfo::from_wasm(&wasm);
    let a1 = dbg.resolve("Run.arr[1]").expect("in range").address;
    assert_eq!(dbg.resolve("Run.arr[2]").unwrap().address, a1 + 4);
    assert_eq!(dbg.resolve("Run.arr[3]").unwrap().address, a1 + 8);
    assert!(
        dbg.resolve("Run.arr[0]").is_none(),
        "lower bound is 1, not 0"
    );
    assert!(dbg.resolve("Run.arr[4]").is_none(), "upper bound is 3");

    // 2-D: row-major, rightmost dimension varying fastest.
    let g00 = dbg.resolve("Run.grid[0][0]").expect("grid[0][0]").address;
    assert_eq!(dbg.resolve("Run.grid[0][1]").unwrap().address, g00 + 4);
    assert_eq!(dbg.resolve("Run.grid[1][0]").unwrap().address, g00 + 8);
    assert_eq!(dbg.resolve("Run.grid[1][1]").unwrap().address, g00 + 12);
    for p in [
        "Run.grid[0][0]",
        "Run.grid[0][1]",
        "Run.grid[1][0]",
        "Run.grid[1][1]",
    ] {
        assert_eq!(dbg.resolve(p).unwrap().ty, SymType::DInt, "{p} type");
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
    let mut plc = TestPlc::load(&wasm).expect("load PLC");
    let dbg = DebugInfo::from_wasm(&wasm);

    // Zero-initialised buffer ⇒ empty string.
    assert_eq!(
        plc.read_var(&dbg, "Run.label"),
        Some(VarValue::String(String::new()))
    );

    // Force a value and read it back.
    plc.write_var(&dbg, "Run.label", VarValue::String("hi".into()))
        .expect("force string");
    assert_eq!(
        plc.read_var(&dbg, "Run.label"),
        Some(VarValue::String("hi".into()))
    );

    // Capacity-bounded (STRING[8]): a longer write truncates to 8 bytes.
    plc.write_var(&dbg, "Run.label", VarValue::String("0123456789".into()))
        .expect("force long string");
    assert_eq!(
        plc.read_var(&dbg, "Run.label"),
        Some(VarValue::String("01234567".into()))
    );

    // A type mismatch is still rejected.
    assert!(plc.write_var(&dbg, "Run.label", VarValue::I16(1)).is_err());
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
    let mut plc = TestPlc::load(&wasm).expect("load PLC");
    let dbg = DebugInfo::from_wasm(&wasm);

    plc.run(2).expect("scans");

    let snap: std::collections::HashMap<&str, VarValue> = plc.read_all(&dbg).into_iter().collect();
    assert_eq!(snap.len(), dbg.list_symbols().len(), "one value per symbol");
    assert_eq!(snap["Run.speed"], VarValue::I16(2));
    assert_eq!(snap["Run.flag"], VarValue::Bool(false));
    assert_eq!(snap["g_count"], VarValue::I32(0));
}

/// A large array must stay OBSERVABLE — the idiom every debug format uses:
/// describe the shape once, compute elements on demand (DWARF's array_type
/// does the same). The old table only knew eagerly-expanded
/// leaves, so an array past the cap contributed NOTHING: invisible to the
/// monitor and the debugger, with no marker saying so, and unforceable.
#[rstest]
fn a_large_array_is_described_and_addressable_not_invisible(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR
            big : ARRAY[0..4999] OF DINT;
            k : DINT;
        END_VAR
            (* write recognizable values so on-demand reads are checkable *)
            FOR k := 0 TO 4999 DO
                big[k] := k * 2;
            END_FOR;
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let table = read_debug_symbols(&wasm);

    // The descriptor: one entry, whatever the element count.
    let arr = table
        .arrays
        .iter()
        .find(|a| a.path == "P1.big")
        .expect("a 5000-element array must appear as a descriptor");
    assert_eq!(arr.total_elements, 5000);
    assert_eq!(arr.dimensions, vec![(0, 4999)]);
    assert_eq!(arr.elem_size, 4);
    assert_eq!(arr.elem_ty, Some(SymType::DInt));

    // NO per-element leaves: the descriptor above is the entire record, so a
    // 5000-element array costs the artifact one entry, not 5000 path strings.
    let leaves = table
        .symbols
        .iter()
        .filter(|s| s.path.starts_with("P1.big["))
        .count();
    assert_eq!(
        leaves, 0,
        "elements are resolved on demand, never enumerated"
    );

    // Any element resolves ON DEMAND through the descriptor: readable and
    // forceable, like adding `big[4321]` to a watch list.
    let mut plc = TestPlc::load(&wasm).expect("load");
    plc.run(1).expect("scan");
    let info = DebugInfo::from_wasm(&wasm);
    assert_eq!(
        plc.read_var(&info, "P1.big[4321]"),
        Some(VarValue::I32(8642)),
        "an element reads through the descriptor"
    );
    plc.write_var(&info, "P1.big[4321]", VarValue::I32(-7))
        .expect("forcing an un-enumerated element");
    assert_eq!(plc.read_var(&info, "P1.big[4321]"), Some(VarValue::I32(-7)));

    // Out of bounds is refused, not computed into a neighbour.
    assert_eq!(plc.read_var(&info, "P1.big[5000]"), None);
    assert_eq!(plc.read_var(&info, "P1.big[-1]"), None);
}

/// An array of aggregates contributes no leaves either — 1000 ten-field
/// structs used to enumerate 10000 paths; now the descriptor plus the interned
/// struct layout is the whole record.
#[rstest]
fn an_aggregate_array_is_one_descriptor_not_ten_thousand_leaves(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Ten : STRUCT
            f0 : DINT; f1 : DINT; f2 : DINT; f3 : DINT; f4 : DINT;
            f5 : DINT; f6 : DINT; f7 : DINT; f8 : DINT; f9 : DINT;
        END_STRUCT; END_TYPE

        PROGRAM P
        VAR
            wide : ARRAY[0..999] OF Ten;
            n : DINT;
        END_VAR
            n := n + 1;
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let table = read_debug_symbols(&wasm);
    let leaves = table
        .symbols
        .iter()
        .filter(|s| s.path.starts_with("P1.wide["))
        .count();
    assert_eq!(
        leaves, 0,
        "no per-element leaves for aggregate arrays either"
    );
    // The descriptor is present; its element is an aggregate, so it carries
    // no scalar decode type — that boundary needs a type table (a follow-up).
    let arr = table
        .arrays
        .iter()
        .find(|a| a.path == "P1.wide")
        .expect("descriptor for the struct array");
    assert_eq!(
        arr.elem_ty, None,
        "aggregate elements have no scalar decode"
    );
    assert_eq!(arr.elem_size, 40);
}

/// Multi-dimensional on-demand resolution takes both spellings the grammar
/// accepts: `[i][j]`, the chained form the tables emit, and `[i, j]`, the
/// standard's.
#[rstest]
fn multi_dimensional_paths_resolve_in_both_spellings(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR
            m : ARRAY[1..40, 1..40] OF INT;
            k : INT;
        END_VAR
            m[7][9] := 79;
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let mut plc = TestPlc::load(&wasm).expect("load");
    plc.run(1).expect("scan");
    let info = DebugInfo::from_wasm(&wasm);
    // A module array has no element leaves: [7][9] resolves through the
    // descriptor, with per-dimension lower bounds honoured.
    assert_eq!(plc.read_var(&info, "P1.m[7][9]"), Some(VarValue::I16(79)));
    assert_eq!(plc.read_var(&info, "P1.m[7,9]"), Some(VarValue::I16(79)));
    assert_eq!(
        plc.read_var(&info, "P1.m[0][9]"),
        None,
        "below the lower bound"
    );
    assert_eq!(
        plc.read_var(&info, "P1.m[7][41]"),
        None,
        "past the second dimension, inside the whole"
    );
}

/// A frame's MEMORY-resident locals — aggregates, strings — must appear in
/// `debug-locals` alongside its scalars. They lived nowhere before v2:
/// stepping into a function, its arrays and structs were uninspectable, in
/// either debug section. For a function that does not recurse their
/// addresses are static, which is what lets them reuse the module-symbol
/// machinery scoped to the frame; a recursive function's are in `frame`,
/// by offset (the next test).
#[rstest]
fn a_frame_s_aggregate_locals_are_described(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Rec : STRUCT x : INT; y : INT; END_STRUCT; END_TYPE

        FUNCTION crunch : INT
        VAR
            r : Rec;
            samples : ARRAY[0..9] OF INT;
            msg : STRING[8];
            big : ARRAY[0..4999] OF DINT;
        END_VAR
            r.x := 1;
            crunch := r.x;
        END_FUNCTION

        FUNCTION_BLOCK Holder
            METHOD PUBLIC weigh : INT
            VAR tmp : Rec; END_VAR
                tmp.y := 2;
                weigh := tmp.y;
            END_METHOD
        END_FUNCTION_BLOCK

        PROGRAM P
        VAR n : INT; h : Holder; END_VAR
            n := crunch() + h.weigh();
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);

    let read_section = |name: &str| super::expect_section(&wasm, name);
    let funcs =
        debug_format::DebugFunctions::from_msgpack(read_section("debug-functions")).unwrap();
    let locals = debug_format::DebugLocals::from_msgpack(read_section("debug-locals")).unwrap();
    let frame = |name: &str| -> &debug_format::FuncLocals {
        let idx = funcs
            .functions
            .iter()
            .find(|f| f.name == name)
            .unwrap_or_else(|| panic!("no function named {name}"))
            .defined_index;
        locals
            .functions
            .iter()
            .find(|f| f.defined_index == idx)
            .unwrap_or_else(|| panic!("no locals table for {name}"))
    };

    // The FUNCTION's frame: struct leaves, the STRING, and a descriptor for
    // every array and no element leaves, as in the module: an element
    // resolves through its descriptor.
    let crunch = frame("crunch");
    let by = |p: &str| crunch.memory.iter().find(|s| s.path == p);
    let rx = by("r.x").expect("struct leaf r.x");
    let ry = by("r.y").expect("struct leaf r.y");
    // INT occupies a 4-byte slot in this layout (the i32-lane storage
    // convention), so the second field sits at +4 — the assertion is that the
    // offsets come from the REAL frame layout, not recomputed guesses.
    assert_eq!(ry.address, rx.address + 4, "layout offsets are frame-real");
    assert!(by("samples[0]").is_none(), "no element leaf");
    assert!(
        crunch.arrays.iter().any(|a| a.path == "samples"),
        "its descriptor"
    );
    assert_eq!(
        by("msg").expect("string leaf").ty,
        SymType::String { capacity: 8 }
    );
    let r = crunch
        .containers
        .iter()
        .find(|c| c.path == "r")
        .expect("the struct local, by its base");
    assert_eq!((r.address, r.type_name.as_str()), (rx.address, "Rec"));
    let big = crunch
        .arrays
        .iter()
        .find(|a| a.path == "big")
        .expect("descriptor for the 5000-element local");
    assert_eq!(big.total_elements, 5000);
    assert_eq!(big.elem_ty, Some(SymType::DInt));
    assert!(
        !crunch.memory.iter().any(|s| s.path.starts_with("big[")),
        "5000 elements, none enumerated"
    );

    // The METHOD's frame — a different lowering path — describes its struct
    // local the same way.
    let weigh = frame("Holder#weigh");
    assert!(
        weigh.memory.iter().any(|s| s.path == "tmp.y"),
        "method-local struct leaf, got: {:?}",
        weigh.memory.iter().map(|s| &s.path).collect::<Vec<_>>()
    );
}

/// A recursive FUNCTION's aggregate locals are in the frame of each call:
/// not among the static ones, where an address would show every frame the
/// innermost call's values, but in `frame`, at offsets from the call's own
/// start. Its scalars are wasm locals, one set per frame.
#[rstest]
fn a_recursive_function_s_frame_locals_are_in_its_frame(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION FDepth : INT
        VAR_INPUT n : INT; END_VAR
        VAR a : ARRAY[0..1] OF INT; END_VAR
            a[0] := n;
            IF n > 0 THEN
                FDepth(n := n - 1);
            END_IF;
            FDepth := a[0];
        END_FUNCTION

        PROGRAM P
        VAR r : INT; END_VAR
            r := FDepth(3);
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let funcs =
        debug_format::DebugFunctions::from_msgpack(super::expect_section(&wasm, "debug-functions"))
            .unwrap();
    let locals =
        debug_format::DebugLocals::from_msgpack(super::expect_section(&wasm, "debug-locals"))
            .unwrap();
    let idx = funcs
        .functions
        .iter()
        .find(|f| f.name == "FDepth")
        .expect("FDepth")
        .defined_index;
    let frame = locals
        .functions
        .iter()
        .find(|f| f.defined_index == idx)
        .expect("a locals table for FDepth");
    assert!(
        frame.memory.is_empty() && frame.arrays.is_empty(),
        "no static address for a frame's array: {:?}",
        frame.memory.iter().map(|s| &s.path).collect::<Vec<_>>()
    );
    let in_frame = frame.frame.as_ref().expect("the frame of each call");
    assert!(
        in_frame.arrays.iter().any(|a| a.path == "a"),
        "in the frame, by offset: {:?}",
        in_frame.arrays.iter().map(|a| &a.path).collect::<Vec<_>>()
    );
    assert!(in_frame.size >= 8, "two INTs, got {}", in_frame.size);
    assert!(
        frame.locals.iter().any(|l| l.name == "n"),
        "the scalar input, per frame"
    );
}

/// The v5 type table: an AGGREGATE array element is addressable member by
/// member — `pts[1500].y`, `pts[1500].history[2]` — by indexing through the
/// descriptor and walking the element's `TypeDesc`, exactly how a debugger
/// resolves member paths from DWARF. Before v5 the descriptor said
/// `elem_ty: None` and no element of an array of structs was reachable: a
/// module array has no element leaves to fall back on.
#[rstest]
fn aggregate_elements_resolve_through_the_type_table(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Pt : STRUCT
            x : DINT;
            y : DINT;
            history : ARRAY[0..3] OF DINT;
        END_STRUCT; END_TYPE

        PROGRAM P
        VAR
            pts : ARRAY[0..1999] OF Pt;
            k : DINT;
        END_VAR
            FOR k := 0 TO 1999 DO
                pts[k].x := k;
                pts[k].y := k * 10;
                pts[k].history[2] := k + 100000;
            END_FOR;
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let table = read_debug_symbols(&wasm);

    // The descriptor names the element's layout in the type table.
    let arr = table
        .arrays
        .iter()
        .find(|a| a.path == "P1.pts")
        .expect("descriptor for the array of structs");
    assert_eq!(arr.elem_ty, None, "an aggregate element has no scalar tag");
    let elem_id = arr
        .elem_type
        .expect("v5: aggregate element carries a TypeId") as usize;
    let debug_format::TypeDesc::Struct { name, size, fields } = &table.types[elem_id] else {
        panic!(
            "Pt should be described as a struct, got {:?}",
            table.types[elem_id]
        );
    };
    assert!(
        name.contains("Pt"),
        "type name is carried for display, got {name}"
    );
    assert_eq!(*size, 24, "x(4) + y(4) + history(4*4)");
    assert_eq!(
        fields.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(),
        vec!["x", "y", "history"]
    );
    // …and the nested array field references a further descriptor whose
    // element is a scalar — the table is a graph, one hop per layer.
    let hist = &fields[2];
    let debug_format::TypeDesc::Array {
        dimensions, elem, ..
    } = &table.types[hist.ty as usize]
    else {
        panic!("history should be an Array desc");
    };
    assert_eq!(dimensions, &vec![(0, 3)]);
    assert!(matches!(
        table.types[*elem as usize],
        debug_format::TypeDesc::Scalar(SymType::DInt)
    ));

    // No element is enumerated: 1500 reads and forces through the table.
    let mut plc = TestPlc::load(&wasm).expect("load");
    plc.run(1).expect("scan");
    let info = DebugInfo::from_wasm(&wasm);
    assert!(
        info.symbol("P1.pts[1500].y").is_none(),
        "an element leaf would let the table prove nothing"
    );
    assert_eq!(
        plc.read_var(&info, "P1.pts[1500].y"),
        Some(VarValue::I32(15000))
    );
    assert_eq!(
        plc.read_var(&info, "P1.pts[1500].history[2]"),
        Some(VarValue::I32(101500)),
        "a nested array INSIDE an un-enumerated element resolves too"
    );
    plc.write_var(&info, "P1.pts[1500].x", VarValue::I32(-3))
        .expect("forcing a member of an un-enumerated element");
    assert_eq!(
        plc.read_var(&info, "P1.pts[1500].x"),
        Some(VarValue::I32(-3))
    );

    // The first element reads the same way.
    assert_eq!(plc.read_var(&info, "P1.pts[0].y"), Some(VarValue::I32(0)));

    // Refusals, not misreads:
    assert_eq!(plc.read_var(&info, "P1.pts[2000].x"), None, "element OOB");
    assert_eq!(plc.read_var(&info, "P1.pts[3].nope"), None, "unknown field");
    assert_eq!(
        plc.read_var(&info, "P1.pts[1500]"),
        None,
        "a whole struct is not a scalar value"
    );
    assert_eq!(
        plc.read_var(&info, "P1.pts[3].history[4]"),
        None,
        "nested subscript OOB"
    );
    assert_eq!(
        plc.read_var(&info, "P1.pts[3].x[0]"),
        None,
        "an accessor past a scalar leaf"
    );
}

/// The frame-local half of the same fix: a FUNCTION-local array of structs
/// gets `elem_type` into the module-level `DebugLocals::types` table.
#[rstest]
fn a_frame_s_aggregate_array_elements_carry_their_layout(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Rec : STRUCT a : DINT; b : DINT; END_STRUCT; END_TYPE

        FUNCTION crunch : DINT
        VAR
            recs : ARRAY[0..999] OF Rec;
        END_VAR
            recs[0].a := 1;
            crunch := recs[0].a;
        END_FUNCTION

        PROGRAM P
        VAR n : DINT; END_VAR
            n := crunch();
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let read_section = |name: &str| super::expect_section(&wasm, name);
    let locals = debug_format::DebugLocals::from_msgpack(read_section("debug-locals")).unwrap();
    let arr = locals
        .functions
        .iter()
        .flat_map(|f| &f.arrays)
        .find(|a| a.path == "recs")
        .expect("descriptor for the frame-local aggregate array");
    assert_eq!(arr.elem_ty, None);
    let id = arr
        .elem_type
        .expect("frame descriptors reference the shared table") as usize;
    assert!(
        matches!(&locals.types[id], debug_format::TypeDesc::Struct { fields, .. } if fields.len() == 2),
        "DebugLocals carries the type table its frames reference"
    );
}

/// Through [`debug_format::DebugInfo`], a frame's memory locals read like the
/// module's: an enum by its variant, a member of an array element by its
/// path. The frames' type table was dropped on reading, so an enum local
/// was looked up in the module's table, where its id is another type's:
/// here `Gear` would read as the program's `Lamp`.
#[rstest]
fn a_frame_s_locals_read_through_debug_info(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Lamp : (Off, On); END_TYPE
        TYPE Gear : (Low, High); END_TYPE
        TYPE Rec : STRUCT mode : Gear; n : DINT; END_STRUCT; END_TYPE

        FUNCTION crunch : DINT
        VAR
            r : Rec;
            recs : ARRAY[0..999] OF Rec;
        END_VAR
            r.mode := Gear#High;
            recs[12].n := 7;
            crunch := recs[12].n;
        END_FUNCTION

        PROGRAM P
        VAR lamp : Lamp; n : DINT; END_VAR
            lamp := Lamp#On;
            n := crunch();
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let info = debug_format::DebugInfo::from_wasm(&wasm);
    assert!(info.problems().is_empty(), "got: {:?}", info.problems());
    let funcs =
        debug_format::DebugFunctions::from_msgpack(super::expect_section(&wasm, "debug-functions"))
            .unwrap();
    let crunch = funcs
        .functions
        .iter()
        .find(|f| f.name == "crunch")
        .expect("crunch is named")
        .defined_index;

    // The program's enum, from the module's table.
    let lamp = info.symbol("P1.lamp").expect("the program's enum");
    let one = |size: u32| 1u64.to_le_bytes()[..size as usize].to_vec();
    assert!(
        matches!(
            info.decode_symbol(lamp, &one(lamp.size)),
            debug_format::VarValue::Enum { ref type_name, ref variant, .. }
                if type_name == "Lamp" && variant.as_deref() == Some("On")
        ),
        "got {:?}",
        info.decode_symbol(lamp, &one(lamp.size))
    );

    // The frame's enum, from the frames' table.
    let frame = info.frame_locals(crunch).expect("crunch has a frame");
    let mode = frame
        .memory
        .iter()
        .find(|s| s.path == "r.mode")
        .expect("the struct local's enum leaf");
    assert!(
        matches!(
            info.decode_local(mode, &one(mode.size)),
            debug_format::VarValue::Enum { ref type_name, ref variant, .. }
                if type_name == "Gear" && variant.as_deref() == Some("High")
        ),
        "got {:?}",
        info.decode_local(mode, &one(mode.size))
    );

    // A member of an element of the frame's array, never enumerated.
    let recs = frame
        .arrays
        .iter()
        .find(|a| a.path == "recs")
        .expect("the frame's array");
    let n = info
        .resolve_local(crunch, "recs[12].n")
        .expect("a path into the frame's array");
    let first = info
        .resolve_local(crunch, "recs[0].mode")
        .expect("the first element's first field");
    assert_eq!(first.address, recs.address);
    assert_eq!(n.ty, SymType::DInt);
    assert_eq!(
        n.address,
        recs.address + 12 * recs.elem_size + (n.address - recs.address) % recs.elem_size,
        "in element 12"
    );
    assert!(
        n.address > recs.address + 12 * recs.elem_size,
        "past `mode`"
    );
    assert_eq!(
        info.resolve_local(crunch, "r.mode").map(|l| l.address),
        Some(mode.address)
    );
    assert_eq!(info.resolve_local(crunch, "recs[1000].n"), None);
    assert_eq!(info.resolve("recs[12].n"), None, "not a module path");
}

/// The artifact cost of an array is its DESCRIPTOR, not its length.
///
/// An `ARRAY[0..4999] OF DINT` used to spend ~22 KB — 98% of the module — on
/// element path strings. The whole point of the descriptor is that 5000
/// addresses one multiplication apart need describing once.
#[rstest]
fn an_arrays_symbol_cost_does_not_grow_with_its_length() {
    // A fresh database per measurement: both sources declare `Main`/`Res`, so
    // registering them side by side would be a duplicate, not two programs.
    let section_size = |decl: &str| {
        let db = &mut db::RootDatabase::default();
        let src = format!(
            r#"
PROGRAM Main
VAR {decl} n : DINT; END_VAR
    n := n + 1;
END_PROGRAM
CONFIGURATION Cfg
    RESOURCE Res ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH T : Main;
    END_RESOURCE
END_CONFIGURATION
"#
        );
        let (_mir, wasm) = compile_to_mir_and_wasm(db, &src);
        super::custom_section(&wasm, DEBUG_SYMBOLS_SECTION)
            .expect("the debug build carries symbols")
            .len()
    };

    let ten = section_size("a : ARRAY[0..9] OF DINT;");
    let five_thousand = section_size("a : ARRAY[0..4999] OF DINT;");
    // Not byte-identical (msgpack spends a couple more bytes writing `4999`
    // than `9`), but within a fixed slack — never per-element.
    assert!(
        five_thousand <= ten + 8,
        "5000 elements must not cost more than 10 plus integer-width slack: \
         {ten} vs {five_thousand}"
    );
}

/// A non-ASCII literal reaches the debugger as the text it was written as:
/// the literal is stored as UTF-8 bytes and read back as UTF-8. Transcoded
/// to Latin-1 it read back with a replacement character.
#[rstest]
fn runtime_reads_a_non_ascii_literal_as_written(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM Main
        VAR
            label : STRING[8] := 'café';
        END_VAR
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM Run WITH T : Main;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let plc = TestPlc::load(&wasm).expect("load PLC");
    let dbg = DebugInfo::from_wasm(&wasm);
    assert_eq!(
        plc.read_var(&dbg, "Run.label"),
        Some(VarValue::String("café".into()))
    );
}

/// A CHAR is its code point, and the debugger sees the whole of it: read
/// as U32, written as U32. It used to be truncated to a byte on the way
/// out, so anything past U+00FF read back wrong.
#[rstest]
fn runtime_reads_a_char_as_its_code_point(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM Main
        VAR
            c : CHAR := CHAR#'中';
        END_VAR
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM Run WITH T : Main;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let mut plc = TestPlc::load(&wasm).expect("load PLC");
    let dbg = DebugInfo::from_wasm(&wasm);
    assert_eq!(plc.read_var(&dbg, "Run.c"), Some(VarValue::U32(0x4E2D)));
    plc.write_var(&dbg, "Run.c", VarValue::U32(u32::from('é')))
        .expect("force a char");
    assert_eq!(plc.read_var(&dbg, "Run.c"), Some(VarValue::U32(0xE9)));
}

/// A STRING cut through a character still reads back: the debugger decodes
/// lossily, so the stray byte shows as U+FFFD rather than hiding the value
/// the program is using.
#[rstest]
fn runtime_reads_a_split_character_lossily(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM Main
        VAR
            v : STRING := 'abé';
            s : STRING[3];
        END_VAR
            s := v;
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM Run WITH T : Main;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let mut plc = TestPlc::load(&wasm).expect("load PLC");
    let dbg = DebugInfo::from_wasm(&wasm);
    plc.run(1).expect("scan");
    assert_eq!(
        plc.read_var(&dbg, "Run.s"),
        Some(VarValue::String("ab\u{FFFD}".into()))
    );
}

/// An aliased type's default reaches a memory-resident host, the static
/// store path and not the wasm-local one: a RETAIN field of a PROGRAM and a
/// struct member of an aliased type both start at their type's default.
#[rstest]
fn a_type_default_reaches_a_program_field(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE
            Pct : INT (0..100) := 50;
            Point : STRUCT
                x : INT := 3;
                n : Pct;
            END_STRUCT;
            Origin : Point := (x := 7);
        END_TYPE

        PROGRAM Main
        VAR RETAIN
            c : Pct;
        END_VAR
        VAR
            o : Origin;
        END_VAR
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM Run WITH T : Main;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let plc = TestPlc::load(&wasm).expect("load PLC");
    let dbg = DebugInfo::from_wasm(&wasm);
    assert_eq!(plc.read_var(&dbg, "Run.c"), Some(VarValue::I16(50)));
    assert_eq!(plc.read_var(&dbg, "Run.o.x"), Some(VarValue::I16(7)));
    assert_eq!(plc.read_var(&dbg, "Run.o.n"), Some(VarValue::I16(50)));
}

/// Every aggregate is named by its base address, which is what a frame is
/// CALLED with. The leaves alone could not answer "whose state is this frame
/// running on": an address that names a whole instance matched no symbol.
#[rstest]
fn every_aggregate_is_named_by_its_base_address(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Point : STRUCT x : INT; y : INT; END_STRUCT END_TYPE

        FUNCTION_BLOCK Gear
        VAR
            ratio : DINT;
            here : Point;
        END_VAR
        END_FUNCTION_BLOCK

        PROGRAM Main
        VAR
            n : INT;
            g : Gear;
            bank : ARRAY[1..2] OF Gear;
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
    let (mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let parsed = read_debug_symbols(&wasm);
    assert_eq!(parsed, mir.debug_symbols);

    let got: Vec<&str> = parsed.containers.iter().map(|c| c.path.as_str()).collect();
    assert_eq!(
        got,
        vec![
            "Run",
            "Run.bank[1]",
            "Run.bank[1].here",
            "Run.bank[2]",
            "Run.bank[2].here",
            "Run.g",
            "Run.g.here",
        ],
        "the instance, each block in the array, and every struct inside them"
    );

    // A container's address is a real one: the instance's own base is where
    // its first field lives, and every leaf under a container sits at or
    // after it.
    let base = |path: &str| {
        parsed
            .containers
            .iter()
            .find(|c| c.path == path)
            .unwrap_or_else(|| panic!("no container {path}"))
            .address
    };
    let leaf = |path: &str| {
        parsed
            .symbols
            .iter()
            .find(|s| s.path == path)
            .unwrap_or_else(|| panic!("no symbol {path}"))
            .address
    };
    assert!(base("Run") >= BUILTIN_RESERVED_FLOOR);
    assert_eq!(
        base("Run.g"),
        leaf("Run.g.ratio"),
        "first field at the base"
    );
    assert!(base("Run.g.here") > base("Run.g"));
    assert_ne!(base("Run.bank[1]"), base("Run.bank[2]"));
}

/// An aggregate whose FIRST field is itself an aggregate shares its base
/// address, so an address alone cannot say which of them a frame is running.
#[rstest]
fn nested_instances_can_share_a_base_address(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Inner
        VAR n : DINT; END_VAR
            n := n + 1;
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK Outer
        VAR
            deep : Inner;
            k : DINT;
        END_VAR
            deep();
        END_FUNCTION_BLOCK

        PROGRAM Main
        VAR
            belt : Outer;
            parts : DINT;
        END_VAR
            belt();
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : Main;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let parsed = read_debug_symbols(&wasm);
    let base = |path: &str| {
        parsed
            .containers
            .iter()
            .find(|c| c.path == path)
            .unwrap_or_else(|| panic!("no container {path}"))
            .address
    };
    assert_eq!(
        (base("P1"), base("P1.belt")),
        (base("P1"), base("P1.belt.deep")),
        "three instances, one address: `belt` is Main's first field and \
         `deep` is Outer's, so the whole chain starts where P1 does"
    );
}

/// A container table written before instances carried their type still names
/// something, and says that it is stale.
///
/// Matching on the missing type resolved NOTHING: every frame instanceless,
/// the variables view back to one undivided program, and no hint why. That is
/// what a half-rebuilt tree produces — a fresh runtime reading a module an
/// older compiler wrote — so it must degrade, not vanish.
#[rstest]
fn a_container_table_without_types_degrades_and_says_so() {
    use debug_format::{ContainerSym, DebugSymbols, SymType, Symbol};

    let table = DebugSymbols {
        version: debug_format::DEBUG_SYMBOLS_VERSION,
        symbols: vec![Symbol {
            path: "P1.belt.n".into(),
            address: 64,
            size: 4,
            ty: SymType::DInt,
            global: false,
            named_type: None,
            bits: None,
        }],
        arrays: vec![],
        types: vec![],
        // Both at one address, as a first-field-aggregate chain really is,
        // and neither saying what it is an instance of.
        containers: vec![
            ContainerSym {
                path: "P1".into(),
                address: 64,
                global: false,
                type_name: String::new(),
            },
            ContainerSym {
                path: "P1.belt".into(),
                address: 64,
                global: false,
                type_name: String::new(),
            },
        ],
        pointers: vec![],
        stack: None,
    };
    let wasm = wasm_with_debug_symbols(&table.to_msgpack());
    let info = DebugInfo::from_wasm(&wasm);

    assert_eq!(
        info.container_at(64, "Main"),
        Some("P1"),
        "the outermost name, which is all such a table can answer"
    );
    assert!(
        info.problems()
            .iter()
            .any(|p| p.contains("without their type")),
        "and it says the build is stale: {:?}",
        info.problems()
    );
}

/// A module carrying just a `debug-symbols` section, for reading it back.
fn wasm_with_debug_symbols(payload: &[u8]) -> Vec<u8> {
    let mut module = wasm_encoder::Module::new();
    module.section(&wasm_encoder::CustomSection {
        name: std::borrow::Cow::Borrowed(DEBUG_SYMBOLS_SECTION),
        data: std::borrow::Cow::Borrowed(payload),
    });
    module.finish()
}

/// A program with every kind of pointer: a `REF_TO` variable, a `REF_TO`
/// member of a STRUCT that points at itself, an aggregate VAR_IN_OUT and a
/// `REF_TO` member of an instance, an array of `REF_TO`. The scalar
/// VAR_IN_OUT beside them is a member, not a pointer.
const POINTERS: &str = r#"
    TYPE Node : STRUCT
        value : INT;
        next : REF_TO Node;
    END_STRUCT; END_TYPE

    TYPE Pt : STRUCT x : REAL; y : REAL; END_STRUCT; END_TYPE

    FUNCTION_BLOCK Motor
    VAR_IN_OUT speed : INT; pos : Pt; END_VAR
    VAR_EXTERNAL hold : Pt; END_VAR
    VAR target : REF_TO Pt; count : INT; END_VAR
        speed := speed + 1;
        pos.x := pos.x + 1.0;
        count := count + 1;
        target := REF(hold);
    END_FUNCTION_BLOCK

    FUNCTION Bump : INT
    VAR_IN_OUT v : INT; p : Pt; END_VAR
    VAR_INPUT r : REF_TO INT; END_VAR
        v := v + 1;
        p.x := p.x + 1.0;
        IF r <> NULL THEN r^ := r^ + 1; END_IF;
        Bump := v;
    END_FUNCTION

    PROGRAM Main
    VAR
        a : Node; b : Node;
        head : REF_TO Node;
        pt : Pt;
        rp : REF_TO Pt;
        ri : REF_TO INT;
        refs : ARRAY[0..2] OF REF_TO INT;
        n : INT;
        m : Motor;
        arr : ARRAY[0..5] OF INT;
        rf : REF_TO REAL;
        ra : REF_TO INT;
    END_VAR
        a.value := 1; b.value := 2;
        arr[3] := 33;
        rf := REF(pt.y);
        ra := REF(arr[3]);
        a.next := REF(b);
        head := REF(a);
        pt.y := 2.5;
        rp := REF(pt);
        ri := REF(n);
        refs[1] := REF(n);
        m(speed := n, pos := pt);
        n := Bump(n, pt, ri);
    END_PROGRAM

    CONFIGURATION Cfg
    VAR_GLOBAL hold : Pt; END_VAR
        RESOURCE Res ON CPU
            TASK T(INTERVAL := T#10ms, PRIORITY := 1);
            PROGRAM P1 WITH T : Main;
        END_RESOURCE
    END_CONFIGURATION
"#;

/// Every pointer in memory is listed with what it points at (v9). A
/// `REF_TO` was dropped from the tables, and a `REF_TO` member was "opaque":
/// a debugger could not follow `head^`, nor tell what `m.speed` was bound
/// to.
#[rstest]
fn pointers_are_listed_with_their_targets(mut with_db: db::RootDatabase) {
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, POINTERS);
    let table = read_debug_symbols(&wasm);
    assert_eq!(table.version, debug_format::DEBUG_SYMBOLS_VERSION);
    let pointer = |path: &str| {
        table
            .pointers
            .iter()
            .find(|p| p.path == path)
            .unwrap_or_else(|| panic!("no pointer {path} in {:?}", table.pointers))
    };
    let struct_name = |id: Option<u32>| match &table.types[id.expect("a target") as usize] {
        debug_format::TypeDesc::Struct { name, .. } => name.clone(),
        other => panic!("not a struct: {other:?}"),
    };

    // The STRUCT that points at itself: its member's target is the STRUCT,
    // whose member's target is the STRUCT, by one entry.
    let next = pointer("P1.a.next");
    assert_eq!(struct_name(next.target), "Node");
    let debug_format::TypeDesc::Struct { fields, .. } = &table.types[next.target.unwrap() as usize]
    else {
        unreachable!()
    };
    let field = fields
        .iter()
        .find(|f| f.name == "next")
        .expect("the next field");
    assert_eq!(
        table.types[field.ty as usize],
        debug_format::TypeDesc::Opaque {
            size: 4,
            target: next.target,
            by_ref: false,
            conformand: None,
        }
    );
    assert_eq!(struct_name(pointer("P1.head").target), "Node");
    assert_eq!(struct_name(pointer("P1.rp").target), "Pt");

    // An instance's aggregate VAR_IN_OUT reads through, its REF_TO does
    // not, and its scalar VAR_IN_OUT is a value of its own.
    let pos = pointer("P1.m.pos");
    assert!(pos.by_ref);
    assert_eq!(struct_name(pos.target), "Pt");
    assert!(
        table.symbols.iter().any(|s| s.path == "P1.m.speed"),
        "a scalar VAR_IN_OUT is a member the instance holds"
    );
    let target = pointer("P1.m.target");
    assert!(!target.by_ref);
    assert_eq!(struct_name(target.target), "Pt");

    // An array of REF_TO: a descriptor whose element is a pointer.
    let refs = table
        .arrays
        .iter()
        .find(|a| a.path == "P1.refs")
        .expect("the array's descriptor");
    assert_eq!(refs.elem_ty, None);
    let debug_format::TypeDesc::Opaque {
        size: 4,
        target: Some(elem),
        by_ref: false,
        conformand: None,
    } = table.types[refs.elem_type.unwrap() as usize]
    else {
        panic!("the element is a pointer");
    };
    assert_eq!(
        table.types[elem as usize],
        debug_format::TypeDesc::Scalar(SymType::Int)
    );

    // A pointer is not a symbol: it has no type of its own to decode.
    assert!(table.symbols.iter().all(|s| s.path != "P1.head"));
    assert!(
        table.pointers.windows(2).all(|w| w[0].path < w[1].path),
        "sorted"
    );
}

/// The pointers a FUNCTION holds in wasm locals, and the frame of a
/// recursive function (`debug-locals` v5). A `REF_TO` input was listed as
/// an INT, and a recursive function's arrays and structs not at all.
#[rstest]
fn a_function_s_pointers_and_a_recursive_frame_are_described(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Pt : STRUCT x : REAL; y : REAL; END_STRUCT; END_TYPE

        FUNCTION Bump : INT
        VAR_IN_OUT v : INT; p : Pt; END_VAR
        VAR_INPUT r : REF_TO INT; END_VAR
        VAR q : REF_TO Pt; END_VAR
            q := REF(p);
            v := v + 1;
            IF r <> NULL THEN r^ := r^ + 1; END_IF;
            Bump := v;
        END_FUNCTION

        FUNCTION Depth : INT
        VAR_INPUT n : INT; END_VAR
        VAR trail : ARRAY[0..3] OF INT; pt : Pt; END_VAR
            trail[0] := n;
            pt.x := n;
            IF n > 0 THEN Depth := Depth(n - 1) + 1; ELSE Depth := 0; END_IF;
        END_FUNCTION

        PROGRAM P
        VAR n : INT; pt : Pt; END_VAR
            n := Bump(n, pt, NULL) + Depth(2);
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let read_section = |name: &str| super::expect_section(&wasm, name);
    let funcs =
        debug_format::DebugFunctions::from_msgpack(read_section("debug-functions")).unwrap();
    let locals = debug_format::DebugLocals::from_msgpack(read_section("debug-locals")).unwrap();
    assert_eq!(locals.version, debug_format::DEBUG_LOCALS_VERSION);
    let frame = |name: &str| -> &debug_format::FuncLocals {
        let idx = funcs
            .functions
            .iter()
            .find(|f| f.name == name)
            .unwrap_or_else(|| panic!("no function named {name}"))
            .defined_index;
        locals
            .functions
            .iter()
            .find(|f| f.defined_index == idx)
            .unwrap_or_else(|| panic!("no locals table for {name}"))
    };

    let bump = frame("Bump");
    let ptr = |name: &str| {
        bump.local_pointers
            .iter()
            .find(|p| p.name == name)
            .unwrap_or_else(|| panic!("no pointer {name} in {:?}", bump.local_pointers))
    };
    let is = |id: Option<u32>, desc: &debug_format::TypeDesc| {
        &locals.types[id.expect("a target") as usize] == desc
    };
    assert!(
        ptr("v").by_ref
            && is(
                ptr("v").target,
                &debug_format::TypeDesc::Scalar(SymType::Int)
            )
    );
    assert!(ptr("p").by_ref);
    assert!(
        matches!(&locals.types[ptr("p").target.unwrap() as usize], debug_format::TypeDesc::Struct { name, .. } if name == "Pt")
    );
    assert!(
        !ptr("r").by_ref
            && is(
                ptr("r").target,
                &debug_format::TypeDesc::Scalar(SymType::Int)
            )
    );
    assert!(!ptr("q").by_ref, "a REF_TO local");
    assert!(
        bump.locals.iter().all(|l| l.name != "r" && l.name != "q"),
        "a pointer is not an integer local: {:?}",
        bump.locals
    );
    assert!(
        bump.local_pointers
            .windows(2)
            .all(|w| w[0].wasm_index < w[1].wasm_index),
        "by slot"
    );
    assert!(bump.frame.is_none(), "Bump does not recurse");

    let depth = frame("Depth");
    assert!(
        depth.memory.is_empty() && depth.arrays.is_empty(),
        "nothing static"
    );
    let frame = depth.frame.as_ref().expect("a recursive function's frame");
    let by = |p: &str| {
        frame
            .memory
            .iter()
            .find(|s| s.path == p)
            .unwrap_or_else(|| panic!("no {p} in {:?}", frame.memory))
    };
    assert_eq!(
        by("pt.y").address,
        by("pt.x").address + 4,
        "offsets in the frame"
    );
    let trail = frame
        .arrays
        .iter()
        .find(|a| a.path == "trail")
        .expect("the array's descriptor");
    assert!(
        trail.address + 16 <= frame.size && by("pt.y").address + 4 <= frame.size,
        "inside the frame"
    );
    assert!(
        depth.locals.iter().all(|l| l.wasm_index != frame.slot),
        "the frame's slot is no declared local"
    );
    let pt = frame
        .containers
        .iter()
        .find(|c| c.path == "pt")
        .expect("the struct local in the frame, by its offset");
    assert_eq!(
        (pt.address, pt.type_name.as_str()),
        (by("pt.x").address, "Pt")
    );
    let (base, end) = read_debug_symbols(&wasm)
        .stack
        .expect("a stack, since Depth recurses");
    assert!(
        base < end && end - base >= frame.size,
        "the stack holds a frame"
    );
}

/// Pointers followed on a running module: `head^.value` through the
/// `REF_TO`, `m.pos` through the VAR_IN_OUT, an element of an array of
/// `REF_TO`, and a pointer's own value as the variable it points at.
#[rstest]
fn pointers_are_followed_on_a_running_module(mut with_db: db::RootDatabase) {
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, POINTERS);
    let mut plc = TestPlc::load(&wasm).expect("load");
    plc.run(1).expect("scan");
    let info = DebugInfo::from_wasm(&wasm);
    assert!(info.problems().is_empty(), "{:?}", info.problems());
    let live = &mut |address: u32, size: u32| plc.read_bytes(address, size as usize).ok();
    let value = |place: Option<debug_format::Place>| match place {
        Some(debug_format::Place::Value(loc)) => loc.decode(
            &plc.read_bytes(loc.address, loc.size as usize)
                .expect("in memory"),
        ),
        other => panic!("not a value: {other:?}"),
    };
    let n = plc.read_var(&info, "P1.n").expect("n");

    assert_eq!(value(info.follow("P1.head^.value", live)), VarValue::I16(1));
    assert_eq!(
        value(info.follow("P1.head^.next^.value", live)),
        VarValue::I16(2)
    );
    assert_eq!(
        value(info.follow("P1.a.next^.value", live)),
        VarValue::I16(2)
    );
    assert_eq!(
        info.follow("P1.head^.next^.next^.value", live),
        Some(debug_format::Place::Null {
            pointer: "P1.head^.next^.next".to_string()
        }),
        "b.next is null"
    );
    assert_eq!(
        value(info.follow("P1.m.speed", live)),
        VarValue::I16(1),
        "the member: n was 0 when the call copied it in, and the body added one"
    );
    assert_eq!(
        value(info.follow("P1.m.pos^.y", live)),
        VarValue::F32(2.5),
        "through the VAR_IN_OUT"
    );
    assert_eq!(
        value(info.follow("P1.m.pos.y", live)),
        VarValue::F32(2.5),
        "the same without the caret"
    );
    assert_eq!(value(info.follow("P1.ri^", live)), n);
    assert_eq!(value(info.follow("P1.refs[1]^", live)), n);
    assert_eq!(
        info.follow("P1.refs[0]^", live),
        Some(debug_format::Place::Null {
            pointer: "P1.refs[0]".to_string()
        })
    );
    assert_eq!(value(info.follow("P1.rp^.y", live)), VarValue::F32(2.5));
    assert_eq!(
        value(info.follow("P1.m.target^.x", live)),
        VarValue::F32(0.0),
        "the global"
    );

    let mut points_at = |path: &str| match info.follow(path, live) {
        Some(debug_format::Place::Pointer(p)) => p.variable,
        other => panic!("not a pointer: {other:?}"),
    };
    assert_eq!(points_at("P1.head").as_deref(), Some("P1.a"));
    assert_eq!(points_at("P1.a.next").as_deref(), Some("P1.b"));
    assert_eq!(points_at("P1.ri").as_deref(), Some("P1.n"));
    assert_eq!(points_at("P1.rp").as_deref(), Some("P1.pt"));
    assert_eq!(points_at("P1.m.target").as_deref(), Some("hold"));
    // Into a variable: a field, and an element no leaf lists.
    assert_eq!(points_at("P1.rf").as_deref(), Some("P1.pt.y"));
    assert_eq!(points_at("P1.ra").as_deref(), Some("P1.arr[3]"));
    assert_eq!(value(info.follow("P1.ra^", live)), VarValue::I16(33));
    let n_at = info.resolve("P1.n").unwrap().address;
    assert_eq!(info.variable_at(n_at).as_deref(), Some("P1.n"));
}

/// A bare code offset resolves to its function: the one `line_to_pc` places
/// a line in, and nothing before the first body.
#[rstest]
fn a_code_offset_resolves_to_its_function(mut with_db: db::RootDatabase) {
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, POINTERS);
    let info = DebugInfo::from_wasm(&wasm);
    let lines = info.breakable_lines();
    assert!(!lines.is_empty());
    for (file, line) in lines {
        let (defined, pc) = info
            .line_to_pc(file, line)
            .expect("a pc for a breakable line");
        assert_eq!(info.function_at(pc), Some(defined), "line {line}");
    }
    assert_eq!(info.function_at(0), None);
}

/// An enumeration reached through an element or a field reads by its
/// variant: the descriptor and the field name their entry, which carries
/// the variants. It read as its integer, where a plain variable read `On`.
#[rstest]
fn an_enum_element_reads_by_its_variant(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Lamp : (Off, On); END_TYPE
        TYPE Gear : (Low, High); END_TYPE
        TYPE Rec : STRUCT mode : Gear; n : DINT; END_STRUCT; END_TYPE

        PROGRAM P
        VAR
            lamps : ARRAY[0..3] OF Lamp;
            recs : ARRAY[0..1] OF Rec;
            lamp : Lamp;
        END_VAR
            lamps[3] := Lamp#On;
            recs[1].mode := Gear#High;
            lamp := Lamp#On;
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let mut plc = TestPlc::load(&wasm).expect("load");
    plc.run(1).expect("scan");
    let info = DebugInfo::from_wasm(&wasm);
    let variant = |path: &str| match plc.read_var(&info, path) {
        Some(VarValue::Enum {
            type_name, variant, ..
        }) => format!("{type_name}#{}", variant.unwrap_or_default()),
        other => panic!("{path} is not an enumeration: {other:?}"),
    };
    assert_eq!(variant("P1.lamp"), "Lamp#On");
    assert_eq!(variant("P1.lamps[3]"), "Lamp#On");
    assert_eq!(variant("P1.lamps[0]"), "Lamp#Off");
    assert_eq!(variant("P1.recs[1].mode"), "Gear#High");
    assert_eq!(variant("P1.recs[0].mode"), "Gear#Low");
    assert_eq!(
        plc.read_var(&info, "P1.recs[1].n"),
        Some(VarValue::I32(0)),
        "a DINT is one"
    );
}

/// Two blocks holding each other through VAR_IN_OUT members: each member
/// is listed with the other block as its target. The one lowered second
/// closed the cycle with no type at all, and a debugger could not read
/// through it.
#[rstest]
fn blocks_holding_each_other_name_each_other(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Ping
        VAR_IN_OUT peer : Pong; END_VAR
        VAR n : INT; END_VAR
            n := n + 1;
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK Pong
        VAR_IN_OUT peer : Ping; END_VAR
        VAR n : INT; END_VAR
            n := n + 2;
        END_FUNCTION_BLOCK

        PROGRAM Main
        VAR a : Ping; b : Pong; END_VAR
            a(peer := b);
            b(peer := a);
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : Main;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let table = read_debug_symbols(&wasm);
    let target_of = |path: &str| {
        let p = table
            .pointers
            .iter()
            .find(|p| p.path == path)
            .unwrap_or_else(|| panic!("no pointer {path} in {:?}", table.pointers));
        assert!(p.by_ref, "{path} is a VAR_IN_OUT");
        match &table.types[p.target.expect("a target") as usize] {
            debug_format::TypeDesc::Struct { name, .. } => name.clone(),
            other => panic!("{path} points at {other:?}"),
        }
    };
    assert_eq!(target_of("P1.a.peer"), "Pong");
    assert_eq!(target_of("P1.b.peer"), "Ping");

    // Read through, each way round, on the running module.
    let mut plc = TestPlc::load(&wasm).expect("load");
    plc.run(1).expect("scan");
    let info = DebugInfo::from_wasm(&wasm);
    let live = &mut |address: u32, size: u32| plc.read_bytes(address, size as usize).ok();
    let value = |place: Option<debug_format::Place>| match place {
        Some(debug_format::Place::Value(loc)) => loc.decode(
            &plc.read_bytes(loc.address, loc.size as usize)
                .expect("in memory"),
        ),
        other => panic!("not a value: {other:?}"),
    };
    assert_eq!(
        value(info.follow("P1.a.peer.n", live)),
        VarValue::I16(2),
        "b's n"
    );
    assert_eq!(
        value(info.follow("P1.b.peer.n", live)),
        VarValue::I16(1),
        "a's n"
    );
    assert_eq!(
        value(info.follow("P1.a.peer.peer.n", live)),
        VarValue::I16(1),
        "and back"
    );
}

/// `source` after `USING Std.Arrays;`, the stdlib's array functions loaded
/// with it: for a body that reads the bounds of an `ARRAY[*]`.
fn compile_with_bounds(db: &mut db::RootDatabase, source: &str) -> Vec<u8> {
    crate::tests::codegen::compile_with_libraries(
        db,
        &[include_str!("../../../stdlib/Arrays.st")],
        &format!("USING Std.Arrays;\n{source}"),
    )
}

/// A VAR_IN_OUT that is an `ARRAY[*]` has no bounds of its own, and the
/// instance keeps the address alone: each copy of the body has the bounds
/// as constants. The table gives its pointer no target, and what the
/// declaration says: how many dimensions, and the element. It reads as the
/// array of that rank and element that starts at the address, with that
/// array's bounds, which are those of the last call. Once bound, a debugger
/// asking for it got nothing.
///
/// An address alone says where an array starts and not which: a matrix and
/// its first row start together, and so do an array of STRUCTs and the
/// array its first element begins with. Read by the descriptor at the
/// address, `member.r[1]` was the STRUCT `recs2[1]`.
#[rstest]
fn a_conformand_in_out_reads_as_the_array_of_its_rank_and_element(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Rec : STRUCT hist : ARRAY[0..1, 0..1] OF USINT; k : USINT; END_STRUCT; END_TYPE
        TYPE Rec2 : STRUCT list : ARRAY[0..1] OF USINT; k : USINT; END_STRUCT; END_TYPE

        FUNCTION_BLOCK Walk
        VAR_IN_OUT a : ARRAY[*, *] OF USINT; END_VAR
        VAR_OUTPUT seen : USINT; END_VAR
            seen := a[LOWER_BOUND(a, 1), LOWER_BOUND(a, 2)];
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK Row
        VAR_IN_OUT r : ARRAY[*] OF USINT; END_VAR
        VAR_OUTPUT seen : USINT; END_VAR
            seen := r[UPPER_BOUND(r, 1)];
        END_FUNCTION_BLOCK

        PROGRAM Main
        VAR
            mat : ARRAY[-1..0, 2..3] OF USINT := [1, 2, 3, 4];
            other : ARRAY[0..2, 5..5] OF USINT := [7, 8, 9];
            recs : ARRAY[0..2] OF Rec;
            recs2 : ARRAY[0..2] OF Rec2;
            w, twice, first, inner, idle : Walk;
            top, low, member : Row;
        END_VAR
            recs[0].hist[1, 1] := 5;
            recs[2].hist[1, 0] := 6;
            recs2[0].list[1] := 9;
            w(a := mat);
            twice(a := mat);
            twice(a := other);
            first(a := recs[0].hist);
            inner(a := recs[2].hist);
            top(r := mat[-1]);
            low(r := mat[0]);
            member(r := recs2[0].list);
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : Main;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let wasm = compile_with_bounds(&mut with_db, source);
    let table = read_debug_symbols(&wasm);
    let pointer = table
        .pointers
        .iter()
        .find(|p| p.path == "P1.w.a")
        .expect("the VAR_IN_OUT is listed");
    assert!(pointer.by_ref);
    assert_eq!(pointer.target, None, "an ARRAY[*] has no type of its own");
    let said = pointer.conformand.expect("what the declaration says");
    assert_eq!(said.rank, 2);
    assert_eq!(
        table.types[said.elem as usize],
        debug_format::TypeDesc::Scalar(SymType::USInt)
    );
    let array = |path: &str| {
        table
            .arrays
            .iter()
            .find(|a| a.path == path)
            .unwrap_or_else(|| panic!("{path} is described"))
    };
    let (mat, recs) = (array("P1.mat"), array("P1.recs"));

    let mut plc = TestPlc::load(&wasm).expect("load");
    plc.run(1).expect("scan");
    let info = DebugInfo::from_wasm(&wasm);
    let live = &mut |address: u32, size: u32| plc.read_bytes(address, size as usize).ok();
    let value = |place: Option<debug_format::Place>| match place {
        Some(debug_format::Place::Value(loc)) => loc.decode(
            &plc.read_bytes(loc.address, loc.size as usize)
                .expect("in memory"),
        ),
        other => panic!("not a value: {other:?}"),
    };
    // The array a path ends at: where it is, and the bounds that list its
    // elements.
    let shape = |place: Option<debug_format::Place>| match place {
        Some(debug_format::Place::Array(shape)) => (shape.address, shape.dimensions),
        other => panic!("not an array: {other:?}"),
    };

    // A matrix, each subscript within its own bounds.
    assert_eq!(
        shape(info.follow("P1.w.a", live)),
        (mat.address, vec![(-1, 0), (2, 3)])
    );
    assert_eq!(value(info.follow("P1.w.a[-1, 2]", live)), VarValue::U8(1));
    assert_eq!(value(info.follow("P1.w.a[0, 3]", live)), VarValue::U8(4));
    assert_eq!(
        value(info.follow("P1.w.a[-1, 2]", live)),
        plc.read_var(&info, "P1.w.seen").expect("the output")
    );
    assert_eq!(
        info.follow("P1.w.a[-1, 4]", live),
        None,
        "past the second dimension, inside the whole"
    );
    assert_eq!(info.follow("P1.w.a[1, 2]", live), None, "past the first");

    assert_eq!(
        value(info.follow("P1.twice.a[2, 5]", live)),
        VarValue::U8(9),
        "the last binding's bounds"
    );
    assert_eq!(
        info.follow("P1.twice.a[-1, 2]", live),
        None,
        "not the first binding's"
    );
    assert_eq!(plc.read_var(&info, "P1.twice.seen"), Some(VarValue::U8(7)));

    // Where `recs` starts, an array of one dimension, of Rec.
    assert_eq!(
        shape(info.follow("P1.first.a", live)),
        (recs.address, vec![(0, 1), (0, 1)])
    );
    assert_eq!(
        value(info.follow("P1.first.a[1, 1]", live)),
        VarValue::U8(5)
    );
    // Where no descriptor starts.
    assert_eq!(
        shape(info.follow("P1.inner.a", live)),
        (recs.address + 2 * recs.elem_size, vec![(0, 1), (0, 1)])
    );
    assert_eq!(
        value(info.follow("P1.inner.a[1, 0]", live)),
        VarValue::U8(6)
    );

    // The rows of the matrix, the first starting where it does.
    assert_eq!(
        shape(info.follow("P1.top.r", live)),
        (mat.address, vec![(2, 3)])
    );
    assert_eq!(value(info.follow("P1.top.r[2]", live)), VarValue::U8(1));
    assert_eq!(value(info.follow("P1.top.r[3]", live)), VarValue::U8(2));
    assert_eq!(info.follow("P1.top.r[4]", live), None, "the next row");
    assert_eq!(plc.read_var(&info, "P1.top.seen"), Some(VarValue::U8(2)));
    assert_eq!(
        shape(info.follow("P1.low.r", live)),
        (mat.address + 2 * mat.elem_size, vec![(2, 3)])
    );
    assert_eq!(value(info.follow("P1.low.r[3]", live)), VarValue::U8(4));
    assert_eq!(plc.read_var(&info, "P1.low.seen"), Some(VarValue::U8(4)));

    // Not the STRUCT `recs2[1]`, which the descriptor's subscript names.
    assert_eq!(value(info.follow("P1.member.r[1]", live)), VarValue::U8(9));
    assert_eq!(info.follow("P1.member.r[2]", live), None);
    assert_eq!(plc.read_var(&info, "P1.member.seen"), Some(VarValue::U8(9)));

    // A block never called holds no address, and reads as null.
    let null = Some(debug_format::Place::Null {
        pointer: "P1.idle.a".to_string(),
    });
    assert_eq!(info.follow("P1.idle.a", live), null);
    assert_eq!(info.follow("P1.idle.a[0, 3]", live), null);
}

/// The array an `ARRAY[*]` is bound to is found by its rank and its element
/// wherever the tables describe it: a slice of a larger array, an array of
/// STRUCTs, of STRINGs of another capacity, of an enumeration, and one an
/// instance in an instance holds. An `ARRAY[*]` member of an element of an
/// array of instances has no entry of its own in `pointers`: the type table
/// says its rank and element.
#[rstest]
fn a_conformand_in_out_is_found_whatever_holds_the_array(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Lamp : (Off, On, Blink); END_TYPE
        TYPE Pt : STRUCT x : INT; y : INT; END_STRUCT; END_TYPE

        FUNCTION_BLOCK Walk
        VAR_IN_OUT a : ARRAY[*, *] OF USINT; END_VAR
        VAR_OUTPUT seen : USINT; END_VAR
            seen := a[UPPER_BOUND(a, 1), UPPER_BOUND(a, 2)];
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK Row
        VAR_IN_OUT r : ARRAY[*] OF USINT; END_VAR
        VAR_OUTPUT seen : USINT; END_VAR
            seen := r[UPPER_BOUND(r, 1)];
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK Points
        VAR_IN_OUT p : ARRAY[*] OF Pt; END_VAR
        VAR_OUTPUT seen : INT; END_VAR
            seen := p[UPPER_BOUND(p, 1)].y;
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK Names
        VAR_IN_OUT n : ARRAY[*] OF STRING; END_VAR
        VAR_OUTPUT seen : DINT; END_VAR
            seen := UPPER_BOUND(n, 1);
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK Lamps
        VAR_IN_OUT l : ARRAY[*] OF Lamp; END_VAR
        VAR_OUTPUT seen : Lamp; END_VAR
            seen := l[UPPER_BOUND(l, 1)];
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK Holder
        VAR
            inner : Row;
            own : ARRAY[3..4] OF USINT := [30, 40];
        END_VAR
            inner(r := own);
        END_FUNCTION_BLOCK

        PROGRAM Main
        VAR
            other : ARRAY[0..2, 5..5] OF USINT := [7, 8, 9];
            cube : ARRAY[0..1, 0..1, 0..1] OF USINT := [1, 2, 3, 4, 5, 6, 7, 8];
            pts : ARRAY[1..2] OF Pt;
            names : ARRAY[0..1] OF STRING[4] := ['ab', 'cd'];
            lamps : ARRAY[0..2] OF Lamp := [Lamp#Off, Lamp#On, Lamp#Blink];
            blocks : ARRAY[0..1] OF Walk;
            slice : Walk;
            line : Row;
            points : Points;
            strings : Names;
            lights : Lamps;
            holder : Holder;
        END_VAR
            pts[2].y := 22;
            blocks[1](a := other);
            slice(a := cube[1]);
            line(r := cube[1, 0]);
            points(p := pts);
            strings(n := names);
            lights(l := lamps);
            holder();
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : Main;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let wasm = compile_with_bounds(&mut with_db, source);
    let table = read_debug_symbols(&wasm);
    assert!(
        !table
            .pointers
            .iter()
            .any(|p| p.path.starts_with("P1.blocks")),
        "an element's pointer is reached through the type table"
    );
    let array = |path: &str| {
        table
            .arrays
            .iter()
            .find(|a| a.path == path)
            .unwrap_or_else(|| panic!("{path} is described"))
    };
    let cube = array("P1.cube");

    let mut plc = TestPlc::load(&wasm).expect("load");
    plc.run(1).expect("scan");
    let info = DebugInfo::from_wasm(&wasm);
    let live = &mut |address: u32, size: u32| plc.read_bytes(address, size as usize).ok();
    let value = |place: Option<debug_format::Place>| match place {
        Some(debug_format::Place::Value(loc)) => info.decode_at(
            &loc,
            &plc.read_bytes(loc.address, loc.size as usize)
                .expect("in memory"),
        ),
        other => panic!("not a value: {other:?}"),
    };
    let shape = |place: Option<debug_format::Place>| match place {
        Some(debug_format::Place::Array(shape)) => shape,
        other => panic!("not an array: {other:?}"),
    };

    // In an element of an array of instances, and unbound in its neighbor.
    let bound = shape(info.follow("P1.blocks[1].a", live));
    assert_eq!(bound.address, array("P1.other").address);
    assert_eq!(bound.dimensions, [(0, 2), (5, 5)]);
    assert_eq!(
        value(info.follow("P1.blocks[1].a[2, 5]", live)),
        VarValue::U8(9)
    );
    assert_eq!(
        info.follow("P1.blocks[0].a", live),
        Some(debug_format::Place::Null {
            pointer: "P1.blocks[0].a".to_string()
        })
    );

    // The second half of a cube, and the first row of that half.
    let half = shape(info.follow("P1.slice.a", live));
    assert_eq!(half.address, cube.address + 4 * cube.elem_size);
    assert_eq!(half.dimensions, [(0, 1), (0, 1)]);
    assert_eq!(
        value(info.follow("P1.slice.a[0, 0]", live)),
        VarValue::U8(5)
    );
    assert_eq!(
        value(info.follow("P1.slice.a[1, 1]", live)),
        VarValue::U8(8)
    );
    let row = shape(info.follow("P1.line.r", live));
    assert_eq!((row.address, row.total_elements), (half.address, 2));
    assert_eq!(value(info.follow("P1.line.r[1]", live)), VarValue::U8(6));

    // An array of STRUCTs: its elements keep their members.
    let points = shape(info.follow("P1.points.p", live));
    assert_eq!((points.elem_ty, points.dimensions), (None, vec![(1, 2)]));
    assert_eq!(
        value(info.follow("P1.points.p[2].y", live)),
        VarValue::I16(22)
    );

    // STRINGs keep the capacity their array declared, and an enumeration
    // reads by its variant.
    let strings = shape(info.follow("P1.strings.n", live));
    assert_eq!(strings.elem_ty, Some(SymType::String { capacity: 4 }));
    assert_eq!(
        value(info.follow("P1.strings.n[1]", live)),
        VarValue::String("cd".to_string())
    );
    assert_eq!(
        value(info.follow("P1.lights.l[2]", live)),
        VarValue::Enum {
            type_name: "Lamp".to_string(),
            variant: Some("Blink".to_string()),
            raw: 2,
        }
    );

    // An instance in an instance, bound to its owner's array.
    assert_eq!(
        shape(info.follow("P1.holder.inner.r", live)).dimensions,
        [(3, 4)]
    );
    assert_eq!(
        value(info.follow("P1.holder.inner.r[4]", live)),
        VarValue::U8(40)
    );
    assert_eq!(
        plc.read_var(&info, "P1.holder.inner.seen"),
        Some(VarValue::U8(40))
    );
}

/// The search is over the module's memory. A temporary is among the locals
/// of the body that declares it, which the module's tables do not describe:
/// the path stops at the pointer, with the address it holds, and reaches
/// nothing through it.
#[rstest]
fn a_conformand_in_out_bound_to_a_temporary_stops_at_the_pointer(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Row
        VAR_IN_OUT r : ARRAY[*] OF USINT; END_VAR
        VAR_OUTPUT seen : USINT; END_VAR
            seen := r[UPPER_BOUND(r, 1)];
        END_FUNCTION_BLOCK

        PROGRAM Main
        VAR
            line : Row;
        END_VAR
        VAR_TEMP
            scratch : ARRAY[3..5] OF USINT;
        END_VAR
            scratch[5] := 55;
            line(r := scratch);
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : Main;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let wasm = compile_with_bounds(&mut with_db, source);
    let mut plc = TestPlc::load(&wasm).expect("load");
    plc.run(1).expect("scan");
    let info = DebugInfo::from_wasm(&wasm);
    assert_eq!(plc.read_var(&info, "P1.line.seen"), Some(VarValue::U8(55)));
    let scratch = info
        .frame_tables()
        .flat_map(|(_, table)| &table.arrays)
        .find(|a| a.path == "scratch")
        .expect("the temporary, in its body's table");

    let live = &mut |address: u32, size: u32| plc.read_bytes(address, size as usize).ok();
    let stop = Some(debug_format::Place::Pointer(debug_format::Pointee {
        address: scratch.address,
        target: None,
        by_ref: true,
        variable: None,
        in_frame: None,
    }));
    assert_eq!(info.follow("P1.line.r", live), stop);
    assert_eq!(info.follow("P1.line.r[5]", live), stop);
}
