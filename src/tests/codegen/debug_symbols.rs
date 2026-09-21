//! Debug-symbol table tests: the `debug-symbols` custom section maps every
//! debuggable IEC variable (config/resource globals + program-instance fields,
//! down to elementary leaves) to its absolute linear-memory address. Programs
//! are instance-based, so a CONFIGURATION is needed to allocate an instance.

use crate::tests::codegen::{compile_to_mir_and_wasm, with_db};
use mir::debug_symbols::{DEBUG_SYMBOLS_SECTION, DEBUG_SYMBOLS_VERSION, DebugSymbols, SymType};
use rstest::*;
use debug_format::{DebugInfo, VarValue};

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
    assert!(dbg.resolve("Run.arr[0]").is_none(), "lower bound is 1, not 0");
    assert!(dbg.resolve("Run.arr[4]").is_none(), "upper bound is 3");

    // 2-D: row-major, rightmost dimension varying fastest.
    let g00 = dbg.resolve("Run.grid[0][0]").expect("grid[0][0]").address;
    assert_eq!(dbg.resolve("Run.grid[0][1]").unwrap().address, g00 + 4);
    assert_eq!(dbg.resolve("Run.grid[1][0]").unwrap().address, g00 + 8);
    assert_eq!(dbg.resolve("Run.grid[1][1]").unwrap().address, g00 + 12);
    for p in ["Run.grid[0][0]", "Run.grid[0][1]", "Run.grid[1][0]", "Run.grid[1][1]"] {
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
    assert!(
        plc.write_var(&dbg, "Run.label", VarValue::I16(1))
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
    assert_eq!(leaves, 0, "elements are resolved on demand, never enumerated");

    // Any element resolves ON DEMAND through the descriptor: readable and
    // forceable, like adding `big[4321]` to a watch list.
    let mut plc = TestPlc::load(&wasm).expect("load");
    plc.run(1).expect("scan");
    let info = DebugInfo::from_wasm(&wasm);
    assert_eq!(
        plc.read_var(&info, "P1.big[4321]"),
        Some(VarValue::I32(8642)),
        "an element far past the leaf budget reads through the descriptor"
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
    assert_eq!(leaves, 0, "no per-element leaves for aggregate arrays either");
    // The descriptor is present; its element is an aggregate, so it carries
    // no scalar decode type — that boundary needs a type table (a follow-up).
    let arr = table
        .arrays
        .iter()
        .find(|a| a.path == "P1.wide")
        .expect("descriptor for the struct array");
    assert_eq!(arr.elem_ty, None, "aggregate elements have no scalar decode");
    assert_eq!(arr.elem_size, 40);
}

/// Multi-dimensional on-demand resolution uses the emitted `[i][j]` form —
/// what the grammar accepts — and the legacy `[i,j]` spelling still resolves.
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
    // 1600 elements > budget, so [7][9] is not an eager leaf — it resolves
    // through the descriptor with per-dimension lower bounds honoured.
    assert_eq!(plc.read_var(&info, "P1.m[7][9]"), Some(VarValue::I16(79)));
    assert_eq!(plc.read_var(&info, "P1.m[7,9]"), Some(VarValue::I16(79)));
    assert_eq!(plc.read_var(&info, "P1.m[0][9]"), None, "below the lower bound");
}

/// A frame's MEMORY-resident locals — aggregates, strings — must appear in
/// `debug-locals` alongside its scalars. They lived nowhere before v2:
/// stepping into a function, its arrays and structs were uninspectable, in
/// either debug section. Their addresses are static (IEC forbids recursion,
/// so a function's memory locals have fixed homes), which is what lets them
/// reuse the module-symbol machinery scoped to the frame.
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

    // The FUNCTION's frame: struct leaves, array leaves, the STRING, and a
    // descriptor for every array — including the big one, whose leaves are
    // budget-bounded rather than enumerated or dropped.
    let crunch = frame("crunch");
    let by = |p: &str| crunch.memory.iter().find(|s| s.path == p);
    let rx = by("r.x").expect("struct leaf r.x");
    let ry = by("r.y").expect("struct leaf r.y");
    // INT occupies a 4-byte slot in this layout (the i32-lane storage
    // convention), so the second field sits at +4 — the assertion is that the
    // offsets come from the REAL frame layout, not recomputed guesses.
    assert_eq!(ry.address, rx.address + 4, "layout offsets are frame-real");
    assert!(by("samples[0]").is_some(), "array leaf");
    assert_eq!(
        by("msg").expect("string leaf").ty,
        SymType::String { capacity: 8 }
    );
    let big = crunch
        .arrays
        .iter()
        .find(|a| a.path == "big")
        .expect("descriptor for the 5000-element local");
    assert_eq!(big.total_elements, 5000);
    assert_eq!(big.elem_ty, Some(SymType::DInt));
    let big_leaves = crunch
        .memory
        .iter()
        .filter(|s| s.path.starts_with("big["))
        .count();
    assert!(
        big_leaves <= 1024,
        "the frame's leaf budget holds, got {big_leaves}"
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

/// The v5 type table: an AGGREGATE array element far past the leaf budget is
/// addressable member by member — `pts[1500].y`, `pts[1500].history[2]` —
/// by indexing through the descriptor and walking the element's `TypeDesc`,
/// exactly how a debugger resolves member paths from DWARF. Before v5 the
/// descriptor said `elem_ty: None` and every un-enumerated element of an
/// array of structs was unreachable — readable for the first budget's worth
/// of leaves and silently invisible past that.
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
    let elem_id = arr.elem_type.expect("v5: aggregate element carries a TypeId") as usize;
    let debug_format::TypeDesc::Struct { name, size, fields } = &table.types[elem_id] else {
        panic!("Pt should be described as a struct, got {:?}", table.types[elem_id]);
    };
    assert!(name.contains("Pt"), "type name is carried for display, got {name}");
    assert_eq!(*size, 24, "x(4) + y(4) + history(4*4)");
    assert_eq!(
        fields.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(),
        vec!["x", "y", "history"]
    );
    // …and the nested array field references a further descriptor whose
    // element is a scalar — the table is a graph, one hop per layer.
    let hist = &fields[2];
    let debug_format::TypeDesc::Array { dimensions, elem, .. } = &table.types[hist.ty as usize]
    else {
        panic!("history should be an Array desc");
    };
    assert_eq!(dimensions, &vec![(0, 3)]);
    assert!(matches!(
        table.types[*elem as usize],
        debug_format::TypeDesc::Scalar(SymType::DInt)
    ));

    // 2000 elements × 6 leaves each ≫ budget: element 1500 was never
    // enumerated. It reads and forces through the table.
    let mut plc = TestPlc::load(&wasm).expect("load");
    plc.run(1).expect("scan");
    let info = DebugInfo::from_wasm(&wasm);
    assert!(
        info.symbol("P1.pts[1500].y").is_none(),
        "the probe element must be past the leaf budget for this test to prove anything"
    );
    assert_eq!(plc.read_var(&info, "P1.pts[1500].y"), Some(VarValue::I32(15000)));
    assert_eq!(
        plc.read_var(&info, "P1.pts[1500].history[2]"),
        Some(VarValue::I32(101500)),
        "a nested array INSIDE an un-enumerated element resolves too"
    );
    plc.write_var(&info, "P1.pts[1500].x", VarValue::I32(-3))
        .expect("forcing a member of an un-enumerated element");
    assert_eq!(plc.read_var(&info, "P1.pts[1500].x"), Some(VarValue::I32(-3)));

    // In-budget elements still read through the eager leaf table and agree.
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
    let id = arr.elem_type.expect("frame descriptors reference the shared table") as usize;
    assert!(
        matches!(&locals.types[id], debug_format::TypeDesc::Struct { fields, .. } if fields.len() == 2),
        "DebugLocals carries the type table its frames reference"
    );
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
    use debug_format::{ContainerSym, DebugSymbols, Symbol, SymType};

    let table = DebugSymbols {
        version: debug_format::DEBUG_SYMBOLS_VERSION,
        symbols: vec![Symbol {
            path: "P1.belt.n".into(),
            address: 64,
            size: 4,
            ty: SymType::DInt,
            global: false,
            named_type: None,
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
    };
    let wasm = wasm_with_debug_symbols(&table.to_msgpack());
    let info = DebugInfo::from_wasm(&wasm);

    assert_eq!(
        info.container_at(64, "Main"),
        Some("P1"),
        "the outermost name, which is all such a table can answer"
    );
    assert!(
        info.problems().iter().any(|p| p.contains("without their type")),
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
