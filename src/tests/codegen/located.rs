//! End-to-end tests for located (`AT %…`) VAR_GLOBALs: the three I/O bands, a
//! host writing the input image, and a program writing the output one.

use crate::tests::codegen::TestPlc;
use crate::tests::codegen::{compile_to_mir_and_wasm, with_db};
use rstest::*;

/// An export name appears verbatim in the module's export section, so a byte
/// search answers whether the band was emitted at all — which is the point of
/// gating the exports: an export is a root the optimizer cannot remove.
fn exports(wasm: &[u8], name: &str) -> bool {
    wasm.windows(name.len()).any(|w| w == name.as_bytes())
}

/// The host owns `%I`: it writes the process image into the input band before
/// a scan, and the program reads it through the variable's own name.
#[rstest]
fn the_host_writes_an_input_the_program_reads(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR_EXTERNAL sensor : INT; END_VAR
        VAR RETAIN seen : INT; END_VAR
            seen := sensor;
        END_PROGRAM

        CONFIGURATION Cfg
        VAR_GLOBAL sensor AT %IW4 : INT; END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);

    // One INT input, and nothing in the other two areas.
    assert_eq!(mir.input_size, 4);
    assert_eq!(mir.output_size, 0);
    assert_eq!(mir.marker_size, 0);
    // A located global is NOT in the globals band: the host copies a whole
    // direction at once, so it travels with its own area instead.
    assert_eq!(mir.globals_size, 0);
    assert!(!exports(&wasm, "output_base"), "no %Q was declared");

    let mut plc = TestPlc::load(&wasm).expect("load");
    assert_eq!(plc.input_region().size, 4);

    plc.write_inputs(0, &1234i32.to_le_bytes())
        .expect("write the input image");
    plc.run(1).expect("scan");
    assert_eq!(
        i32::from_le_bytes(plc.read_retain()[..4].try_into().unwrap()),
        1234,
        "the program read what the host put in the input band"
    );
}

/// `%Q` runs the other way: the program writes it and the host reads the band
/// back after the scan.
#[rstest]
fn the_program_writes_an_output_the_host_reads(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR_EXTERNAL valve : INT; END_VAR
            valve := valve + 2;
        END_PROGRAM

        CONFIGURATION Cfg
        VAR_GLOBAL valve AT %QW0 : INT; END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    assert_eq!(mir.input_size, 0);
    assert_eq!(mir.output_size, 4);
    assert!(!exports(&wasm, "input_base"), "no %I was declared");

    let mut plc = TestPlc::load(&wasm).expect("load");
    plc.run(3).expect("scans");
    assert_eq!(
        i32::from_le_bytes(plc.read_outputs()[..4].try_into().unwrap()),
        6,
        "three scans of +2, read back from the output band"
    );
}

/// Each area is one contiguous range, in `%I`, `%Q`, `%M` order, and the
/// globals band follows all three. A host copies one range per direction, so
/// no area may be interleaved with another.
#[rstest]
fn each_area_is_one_contiguous_band(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR_EXTERNAL i1 : INT; q1 : INT; m1 : INT; g : INT; END_VAR
            q1 := i1 + m1 + g;
        END_PROGRAM

        CONFIGURATION Cfg
        VAR_GLOBAL
            i1 AT %IW0 : INT;
            q1 AT %QW0 : INT;
            m1 AT %MW0 : INT;
            g : INT;
        END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (mir, _wasm) = compile_to_mir_and_wasm(&mut with_db, source);

    assert_eq!((mir.input_size, mir.output_size, mir.marker_size), (4, 4, 4));
    assert_eq!(mir.globals_size, 4, "only the unlocated global is here");
    assert_eq!(mir.output_base, mir.input_base + 4);
    assert_eq!(mir.marker_base, mir.output_base + 4);
    assert!(
        mir.globals_base >= mir.marker_base + 4,
        "the globals band follows the located ones"
    );
}

/// Two addresses in the same area are laid out by the ADDRESS, not by the
/// order the CONFIGURATION happens to declare them: an unrelated edit must
/// not shift every address in the debug symbols.
#[rstest]
fn a_band_is_ordered_by_address_not_by_declaration(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR_EXTERNAL late : INT; early : INT; END_VAR
        VAR RETAIN seen : INT; END_VAR
            seen := early * 100 + late;
        END_PROGRAM

        CONFIGURATION Cfg
        VAR_GLOBAL
            late AT %IW8 : INT;
            early AT %IW2 : INT;
        END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    assert_eq!(mir.input_size, 8, "two INTs in the input band");

    // `%IW2` is declared second and still comes first in the band, so the
    // first four bytes a host writes are `early`'s.
    let mut plc = TestPlc::load(&wasm).expect("load");
    plc.write_inputs(0, &1i32.to_le_bytes()).expect("first slot");
    plc.write_inputs(4, &2i32.to_le_bytes()).expect("second slot");
    plc.run(1).expect("scan");
    assert_eq!(
        i32::from_le_bytes(plc.read_retain()[..4].try_into().unwrap()),
        102,
        "the low slot is %IW2 (`early`), not the first-declared %IW8"
    );
}

/// A workspace with no located variable exports no band at all: an export is
/// a root the optimizer cannot remove, and a module with no I/O must not pay
/// for six of them.
#[rstest]
fn no_located_variable_exports_no_band(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR RETAIN n : INT; END_VAR
            n := n + 1;
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    assert_eq!((mir.input_size, mir.output_size, mir.marker_size), (0, 0, 0));
    for name in ["input_base", "input_size", "output_base", "marker_base"] {
        assert!(!exports(&wasm, name), "{name} must not be exported");
    }

    // And a band that was never exported reads back as empty, with no map
    // beside it.
    let plc = TestPlc::load(&wasm).expect("load");
    assert_eq!(plc.input_region().size, 0);
    assert!(plc.located_map().entries.is_empty());
    assert!(
        !exports(&wasm, debug_format::LOCATED_MAP_SECTION),
        "no located-map section either"
    );
}

// ---------------------------------------------------------------------------
// The `located-map` section: which address is which cell. The band exports
// say where the three areas are; this says what lives inside them.
// ---------------------------------------------------------------------------

/// Every located variable appears once, with the address as written, the
/// name the program calls it, its area, its decoded levels, the width its
/// size letter names, and a linear-memory address inside its own band.
#[rstest]
fn the_map_names_every_address(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR_EXTERNAL start : BOOL; lamp : BOOL; deep : WORD; END_VAR
            lamp := start;
            deep := deep + 1;
        END_PROGRAM

        CONFIGURATION Cfg
        VAR_GLOBAL
            start AT %IX0.0   : BOOL;
            lamp  AT %QX0.0   : BOOL;
            deep  AT %MW1.7.9 : WORD;
        END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let plc = TestPlc::load(&wasm).expect("load");
    let map = plc.located_map();

    assert_eq!(map.version, debug_format::LOCATED_MAP_VERSION);
    assert_eq!(map.entries.len(), 3);

    let start = plc.located("%IX0.0").expect("start");
    assert_eq!(start.name, "start");
    assert_eq!(start.area, debug_format::LocatedArea::Input);
    assert_eq!(start.path, vec![0, 0]);
    assert_eq!(start.width, 1, "X names one bit");
    assert_eq!(start.size, 4, "and the BOOL that holds it takes four bytes");
    assert_eq!(start.ty, Some(debug_format::SymType::Bool));
    assert_eq!(start.addr, mir.input_base);

    let deep = plc.located("%MW1.7.9").expect("deep");
    assert_eq!(deep.path, vec![1, 7, 9], "three levels, decoded in order");
    assert_eq!(deep.width, 16);
    assert_eq!(deep.area, debug_format::LocatedArea::Marker);
    assert_eq!(deep.ty, Some(debug_format::SymType::Word));
    assert_eq!(deep.addr, mir.marker_base);

    let lamp = plc.located("%QX0.0").expect("lamp");
    assert_eq!(lamp.area, debug_format::LocatedArea::Output);
    assert_eq!(lamp.addr, mir.output_base);

    // Every entry lands inside the band its area names.
    for (entry, base, size) in [
        (start, mir.input_base, mir.input_size),
        (lamp, mir.output_base, mir.output_size),
        (deep, mir.marker_base, mir.marker_size),
    ] {
        assert!(
            entry.addr >= base && entry.addr + entry.size <= base + size,
            "{} is outside its band",
            entry.address
        );
    }
}

/// A host binds a channel to an ADDRESS and never computes a band offset of
/// its own: the map hands it the linear-memory address, and the program sees
/// the value under the variable's name.
#[rstest]
fn a_host_binds_by_address(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR_EXTERNAL dial : INT; echo : INT; END_VAR
            echo := dial * 2;
        END_PROGRAM

        CONFIGURATION Cfg
        VAR_GLOBAL
            dial AT %IW4 : INT;
            echo AT %QW0 : INT;
        END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let mut plc = TestPlc::load(&wasm).expect("load");

    plc.write_located("%IW4", &21i32.to_le_bytes())
        .expect("bind and write the input channel");
    plc.run(1).expect("scan");
    assert_eq!(
        i32::from_le_bytes(plc.read_located("%QW0").expect("read")[..4].try_into().unwrap()),
        42
    );

    // An address the module does not declare is a failed binding, named.
    let err = plc.located("%IW6").expect_err("not declared");
    assert!(err.to_string().contains("%IW4"), "the error lists what is");
}

/// The layout hash covers the addresses a host bound, not where they landed:
/// adding an unrelated global re-bases every band and must not invalidate a
/// binding, while adding an address must.
#[rstest]
fn the_layout_hash_ignores_rebasing(#[allow(unused)] with_db: db::RootDatabase) {
    let program = |extra_global: &str, extra_addr: &str| {
        format!(
            r#"
        PROGRAM P
        VAR_EXTERNAL dial : INT; END_VAR
        VAR RETAIN seen : INT; END_VAR
            seen := dial;
        END_PROGRAM

        CONFIGURATION Cfg
        VAR_GLOBAL
            dial AT %IW4 : INT;
            {extra_global}
            {extra_addr}
        END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#
        )
    };
    // One database per variant: each is a whole workspace of its own.
    let compile = |extra_global: &str, extra_addr: &str| {
        let mut db = db::RootDatabase::default();
        compile_to_mir_and_wasm(&mut db, &program(extra_global, extra_addr)).0
    };
    let bare = compile("", "");
    let padded = compile("pad : LREAL;", "");
    let grown = compile("", "knob AT %IW8 : INT;");

    assert_ne!(
        bare.input_base, padded.input_base,
        "the unrelated global re-based the bands"
    );
    assert_eq!(
        bare.located_map.layout_hash, padded.located_map.layout_hash,
        "but the set of addresses did not change"
    );
    assert_ne!(
        bare.located_map.layout_hash, grown.located_map.layout_hash,
        "a new address is a new layout"
    );
}
