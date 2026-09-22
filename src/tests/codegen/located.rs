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
///
/// The full rule inside an area is `(retained, address)` — a retained `%M`
/// cell is placed last so the retain band can begin there (see
/// `a_retained_marker_is_in_the_marker_and_retain_bands`), so toggling
/// RETAIN on a marker does reorder that band. Nothing depends on the
/// placement: the located map's hash covers the addresses, not where they
/// landed.
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

// ---------------------------------------------------------------------------
// RETAIN on `%M`. A retained marker has to be in two bands at once: its own,
// which a host copies whole, and the retain band, which a power cycle
// preserves. `%I` and `%Q` cannot be retained at all (E1420).
// ---------------------------------------------------------------------------

/// The retained cells end the `%M` band so the retain band can begin there,
/// and the retain map names them. Before this, a `RETAIN` on a located
/// variable was accepted at check and then dropped: the band was empty and
/// the value silently went transient.
#[rstest]
fn a_retained_marker_is_in_the_marker_and_retain_bands(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR_EXTERNAL count : INT; scratch : INT; g : INT; END_VAR
            count := count + 1;
            scratch := g;
        END_PROGRAM

        CONFIGURATION Cfg
        VAR_GLOBAL
            scratch AT %MW4 : INT;
            g : INT;
        END_VAR
        VAR_GLOBAL RETAIN
            count AT %MW0 : INT;
        END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let plc = TestPlc::load(&wasm).expect("load");

    let count = plc.located("%MW0").expect("count");
    let scratch = plc.located("%MW4").expect("scratch");
    assert_eq!(mir.marker_size, 8, "both markers are in the marker band");

    // The transient cell comes first even though its address number is
    // higher: the retained one has to END the area.
    assert_eq!(scratch.addr, mir.marker_base);
    assert_eq!(count.addr, mir.marker_base + 4);

    // And the retain band starts exactly there, and holds that cell alone:
    // `scratch` is in the marker band ahead of it and `g` is a global past
    // where the band ends, so neither is inside.
    assert_eq!(mir.retain_base, count.addr);
    assert_eq!(mir.retain_size, 4);

    // The map names the retained cell, and only it.
    let names: Vec<&str> = mir
        .retain_map
        .ranges
        .iter()
        .map(|r| r.path.as_str())
        .collect();
    assert_eq!(names, ["count"]);
    let range = &mir.retain_map.ranges[0];
    assert_eq!((range.addr, range.size), (count.addr, 4));
}

/// End to end: the value survives a power cycle when the host restores the
/// mapped range, exactly as it does for any other RETAIN global.
#[rstest]
fn a_retained_marker_survives_a_power_cycle(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR_EXTERNAL count : INT; END_VAR
            count := count + 1;
        END_PROGRAM

        CONFIGURATION Cfg
        VAR_GLOBAL RETAIN count AT %MW0 : INT; END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let read = |plc: &TestPlc| {
        i32::from_le_bytes(
            plc.read_located("%MW0").expect("read")[..4]
                .try_into()
                .unwrap(),
        )
    };

    let mut plc = TestPlc::load(&wasm).expect("load");
    plc.run(3).expect("scans");
    assert_eq!(read(&plc), 3);

    // Snapshot the way the runtime does: the MAP's ranges, in order, not the
    // band. The band is what a legacy module without a `retain-map` section
    // falls back to.
    let saved: Vec<Vec<u8>> = mir
        .retain_map
        .ranges
        .iter()
        .map(|r| plc.read_bytes(r.addr, r.size as usize).expect("snapshot"))
        .collect();

    // Cold start: `__init` put the declared value back.
    let mut cold = TestPlc::load(&wasm).expect("reload");
    cold.run(1).expect("scan");
    assert_eq!(read(&cold), 1, "nothing restored, so it counts from zero");

    // Warm start: each range is replayed at its own address after `__init`.
    let mut warm = TestPlc::load(&wasm).expect("reload");
    for (range, bytes) in mir.retain_map.ranges.iter().zip(&saved) {
        warm.write_bytes(range.addr, bytes).expect("restore");
    }
    warm.run(1).expect("scan");
    assert_eq!(read(&warm), 4, "it picked up where the last power cycle left");
}

/// With nothing retained in `%M`, the retain band starts where it always
/// did — at the retained globals — and the marker band is untouched.
#[rstest]
fn a_transient_marker_leaves_the_retain_band_alone(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR_EXTERNAL flag : INT; kept : INT; END_VAR
            kept := flag;
        END_PROGRAM

        CONFIGURATION Cfg
        VAR_GLOBAL flag AT %MW0 : INT; END_VAR
        VAR_GLOBAL RETAIN kept : INT; END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let plc = TestPlc::load(&wasm).expect("load");
    let flag = plc.located("%MW0").expect("flag");

    assert_eq!(mir.marker_size, 4);
    assert_eq!(
        mir.retain_base, mir.globals_base,
        "with nothing retained in %M, the band opens on the retained globals,          which are what the globals band opens on too"
    );
    assert!(
        mir.retain_base > flag.addr,
        "and that is past the marker band"
    );
    assert_eq!(
        mir.retain_map
            .ranges
            .iter()
            .map(|r| r.path.as_str())
            .collect::<Vec<_>>(),
        ["kept"],
        "a transient marker is in no retain range"
    );
}

/// A retained marker and a retained global together: the two retained groups
/// MEET, with nothing transient between them. The retained markers end `%M`
/// and the retained globals open the globals band, so the retain band closes
/// on them and holds exactly what persists.
///
/// The order matters because a host is free to restore the band rather than
/// the map's ranges. With a transient global inside it, such a host would
/// bring `loose` back from the last power cycle instead of its initializer.
#[rstest]
fn a_retained_marker_and_a_retained_global_share_one_band(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR_EXTERNAL marks : INT; loose : INT; kept : INT; END_VAR
            marks := marks + 1;
            kept := marks + loose;
        END_PROGRAM

        CONFIGURATION Cfg
        VAR_GLOBAL loose : INT; END_VAR
        VAR_GLOBAL RETAIN
            marks AT %MW0 : INT;
            kept : INT;
        END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let plc = TestPlc::load(&wasm).expect("load");
    let marks = plc.located("%MW0").expect("marks");

    // The band opens on the retained marker, and the retained globals start
    // exactly where the retained markers end.
    assert_eq!(mir.retain_base, marks.addr);
    assert_eq!(mir.globals_base, marks.addr + 4);

    // The band closes where the retained globals end, and the transient one
    // starts there — so no transient VARIABLE is inside it. Stated as the two
    // boundaries rather than as a byte count, because the band may also hold
    // alignment padding: see `a_padded_retain_band_holds_no_transient`.
    let retain_end = mir.retain_base + mir.retain_size;
    assert_eq!(
        retain_end,
        mir.globals_base + 4,
        "the band closes on the end of the retained global"
    );
    assert_eq!(mir.globals_size, 8, "the retained global, then the loose one");
    assert!(
        mir.globals_base + mir.globals_size > retain_end,
        "`loose` lies past the band"
    );

    let mut names: Vec<&str> = mir
        .retain_map
        .ranges
        .iter()
        .map(|r| r.path.as_str())
        .collect();
    names.sort();
    assert_eq!(names, ["kept", "marks"]);
    assert!(
        mir.retain_map
            .ranges
            .iter()
            .all(|r| r.addr >= mir.retain_base && r.addr + r.size <= retain_end),
        "every retained range lies inside the band"
    );
}

/// The band may hold alignment padding, and that is the reason its size is
/// not the sum of the retained ranges.
///
/// `globals_base` aligns to the largest alignment the bands contain, so a
/// retained LREAL global behind a retained INT marker leaves four bytes
/// between them. Those bytes are inside the retain band and belong to no
/// variable at all — which is the distinction that matters: a host may
/// restore the whole band without bringing anything transient back, because
/// what is unaccounted for is padding, never a variable.
#[rstest]
fn a_padded_retain_band_holds_no_transient(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR_EXTERNAL marks : INT; kept : LREAL; END_VAR
            marks := marks + 1;
            kept := kept + 1.0;
        END_PROGRAM

        CONFIGURATION Cfg
        VAR_GLOBAL RETAIN
            marks AT %MW0 : INT;
            kept : LREAL;
        END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let plc = TestPlc::load(&wasm).expect("load");
    let marks = plc.located("%MW0").expect("marks");

    // The band still opens and closes on retained cells.
    assert_eq!(mir.retain_base, marks.addr);
    let retain_end = mir.retain_base + mir.retain_size;
    assert_eq!(retain_end, mir.globals_base + 8, "the LREAL ends the band");

    // But it is LONGER than what it holds, by exactly the gap the LREAL's
    // alignment opened after the marker.
    let mapped: u32 = mir.retain_map.ranges.iter().map(|r| r.size).sum();
    assert_eq!(mapped, 12, "an INT and an LREAL");
    let padding = mir.retain_size - mapped;
    assert_eq!(
        padding,
        mir.globals_base - (marks.addr + 4),
        "the shortfall is the alignment gap and nothing else"
    );
    assert!(padding > 0, "this layout is the one that has a gap");

    // Every range is still inside, and the gap belongs to no variable: the
    // globals band starts past it, at the retained global.
    assert!(
        mir.retain_map
            .ranges
            .iter()
            .all(|r| r.addr >= mir.retain_base && r.addr + r.size <= retain_end)
    );
    assert!(mir.globals_base > marks.addr + 4);
}

/// A bare address and a VAR_GLOBAL declared `AT` it are ONE cell: lowering
/// points the bare name at the declared storage instead of giving it a cell of
/// its own. So a value the host binds to the address is seen under both
/// spellings, and a write through the bare form lands in the declared
/// variable — which is what E1421's rule, one channel one cell, requires of
/// every pair of mentions and not only of two declarations.
///
/// Observed through the located cells alone: the program copies the bare
/// input into a named output and the named input into a bare output.
#[rstest]
fn a_bare_address_and_its_declaration_are_one_cell(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR_EXTERNAL sensor : WORD; valve : WORD; echo : WORD; END_VAR
            valve := %IW0;
            %QW2 := sensor;
        END_PROGRAM

        CONFIGURATION Cfg
        VAR_GLOBAL
            sensor AT %IW0 : WORD;
            valve  AT %QW0 : WORD;
            echo   AT %QW2 : WORD;
        END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);

    // Three declared cells and no fourth or fifth for the bare mentions.
    assert_eq!(mir.input_size, 4, "one input cell, not one per spelling");
    assert_eq!(mir.output_size, 8, "two output cells");
    let mut plc = TestPlc::load(&wasm).expect("load");
    let names: Vec<(&str, &str)> = plc
        .located_map()
        .entries
        .iter()
        .map(|e| (e.address.as_str(), e.name.as_str()))
        .collect();
    assert_eq!(
        names,
        [("%IW0", "sensor"), ("%QW0", "valve"), ("%QW2", "echo")],
        "each address once, under the name it was declared with"
    );

    plc.write_located("%IW0", &0x1234i32.to_le_bytes())
        .expect("bind the input channel");
    plc.run(1).expect("scan");
    let read = |address: &str| {
        i32::from_le_bytes(plc.read_located(address).expect("read")[..4].try_into().unwrap())
    };
    assert_eq!(
        read("%QW0"),
        0x1234,
        "the bare `%IW0` read what the host wrote to `sensor`"
    );
    assert_eq!(
        read("%QW2"),
        0x1234,
        "the bare write to `%QW2` landed in `echo`"
    );
}

// ---------------------------------------------------------------------------
// Views. An address inside a wider one the workspace mentions is that
// address's bits, as in any PLC with a process image: each size counts in its
// own units (`%IW1` is bytes 2 and 3) and the image is little-endian, so
// `%IX0.3` is bit 3 of `%IW0` and `%IB1` is its high byte.
// ---------------------------------------------------------------------------

/// The porting idiom: read a bit and a byte of an input word, set bits and a
/// byte of an output word. One cell per word, however many of its parts the
/// program names — bare or declared.
#[rstest]
fn a_narrower_address_is_bits_of_the_wider_one(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR_EXTERNAL ready : BOOL; END_VAR
            %QX0.0 := %IX0.3;
            %QX0.1 := ready;
            %QB1 := %IB1;
        END_PROGRAM

        CONFIGURATION Cfg
        VAR_GLOBAL
            status AT %IW0   : WORD;
            ready  AT %IX1.7 : BOOL;
            lamps  AT %QW0   : WORD;
        END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    assert_eq!(mir.input_size, 4, "`%IX0.3`, `%IB1` and `ready` are `status`'s bits");
    assert_eq!(mir.output_size, 4, "`%QX0.0`, `%QX0.1` and `%QB1` are `lamps`'s bits");

    let mut plc = TestPlc::load(&wasm).expect("load");
    // Bit 3 set, and the high byte 0x8A, whose top bit is `%IX1.7`.
    plc.write_located("%IW0", &0x8A08i32.to_le_bytes())
        .expect("write the input word");
    plc.run(1).expect("scan");
    let lamps = i32::from_le_bytes(plc.read_located("%QW0").expect("read")[..4].try_into().unwrap());
    assert_eq!(
        lamps, 0x8A03,
        "bit 0 from `%IX0.3`, bit 1 from `ready`, the high byte copied"
    );
}

/// A store into a view is a read-modify-write of the owner: only its bits
/// change, and the rest of the word keeps what it held.
#[rstest]
fn a_write_into_a_view_keeps_the_other_bits(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR_EXTERNAL lamps : WORD; END_VAR
            lamps := 16#FFFF;
            %QX0.3 := FALSE;
            %QB1 := 16#12;
        END_PROGRAM

        CONFIGURATION Cfg
        VAR_GLOBAL lamps AT %QW0 : WORD; END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let mut plc = TestPlc::load(&wasm).expect("load");
    plc.run(1).expect("scan");
    let lamps = i32::from_le_bytes(plc.read_located("%QW0").expect("read")[..4].try_into().unwrap());
    assert_eq!(lamps, 0x12F7, "bit 3 cleared, high byte replaced, the rest kept");
}

/// The owner is the WIDEST container mentioned, so a whole nest shares one
/// cell: here `%ID0` owns `%IW1`, `%IB2` and `%IX3.7` alike, and each reads
/// its own bits of it. `%IW1` is bytes 2 and 3 — each size counts in its own
/// units.
#[rstest]
fn a_nest_is_one_cell_owned_by_its_widest_address(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR_EXTERNAL whole : DWORD; END_VAR
            %QW0 := %IW1;
            %QB2 := %IB2;
            %QX3.0 := %IX3.7;
        END_PROGRAM

        CONFIGURATION Cfg
        VAR_GLOBAL whole AT %ID0 : DWORD; out AT %QD0 : DWORD; END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    assert_eq!(mir.input_size, 4, "one DWORD cell for the whole nest");
    let mut plc = TestPlc::load(&wasm).expect("load");
    plc.write_located("%ID0", &(0x9A34_5678u32 as i32).to_le_bytes())
        .expect("write the input dword");
    plc.run(1).expect("scan");
    let out = u32::from_le_bytes(plc.read_located("%QD0").expect("read")[..4].try_into().unwrap());
    assert_eq!(out & 0xFFFF, 0x9A34, "`%IW1` is the high word");
    assert_eq!((out >> 16) & 0xFF, 0x34, "`%IB2` is byte 2");
    assert_eq!((out >> 24) & 1, 1, "`%IX3.7` is the top bit");
}

/// Two addresses of the same size never overlap, and an address with no byte
/// reading — three levels — stays a cell of its own even beside its parent.
#[rstest]
fn siblings_and_deep_addresses_keep_their_own_cells(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR RETAIN a : WORD; b : WORD; c : WORD; d : WORD; END_VAR
            a := %IW0;
            b := %IW1;
            c := %MW1.7;
            d := %MW1.7.9;
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (mir, _wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    assert_eq!(mir.input_size, 8, "`%IW0` and `%IW1` are bytes 0-1 and 2-3");
    assert_eq!(mir.marker_size, 8, "`%MW1.7.9` has no byte reading");
}

/// The idiom the whole change is for: an FB output bound straight to a bit of
/// an output word (`ton(..., Q => %QX0.3)`). The output lands in a scratch
/// and the statement after the call rewrites the word with that bit — inside
/// the body the call is in, so an IF's branch keeps its own.
#[rstest]
fn an_fb_output_bound_to_a_view_sets_its_bit(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Pass
        VAR_INPUT i : BOOL; END_VAR
        VAR_OUTPUT q : BOOL; END_VAR
            q := i;
        END_FUNCTION_BLOCK

        PROGRAM P
        VAR_EXTERNAL lamps : WORD; END_VAR
        VAR f : Pass; g : Pass; END_VAR
            lamps := 16#00F0;
            f(i := %IX0.0, q => %QX0.3);
            IF %IX0.1 THEN
                g(i := TRUE, q => %QX1.0);
            END_IF;
        END_PROGRAM

        CONFIGURATION Cfg
        VAR_GLOBAL status AT %IW0 : WORD; lamps AT %QW0 : WORD; END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    assert_eq!(mir.output_size, 4, "`%QX0.3` and `%QX1.0` are `lamps`'s bits");
    let read = |plc: &TestPlc| {
        i32::from_le_bytes(plc.read_located("%QW0").expect("read")[..4].try_into().unwrap())
    };

    let mut plc = TestPlc::load(&wasm).expect("load");
    plc.write_located("%IW0", &0b01i32.to_le_bytes()).expect("input");
    plc.run(1).expect("scan");
    assert_eq!(read(&plc), 0x00F8, "bit 3 set, the preset bits kept");

    plc.write_located("%IW0", &0b10i32.to_le_bytes()).expect("input");
    plc.run(1).expect("scan");
    assert_eq!(read(&plc), 0x01F0, "bit 3 cleared, and the IF's call set bit 8");
}

/// The located map lists every part at its owner's cell, with the bits it
/// is, so a host binds `%IX1.7` as surely as `%IW0` — here through the
/// harness, which reads and writes parts the way the map tells a host to.
#[rstest]
fn the_map_lists_a_part_at_its_owners_cell(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR_EXTERNAL ready : BOOL; END_VAR
            %QX0.0 := ready;
            %QB1 := %IB1;
        END_PROGRAM

        CONFIGURATION Cfg
        VAR_GLOBAL
            status AT %IW0   : WORD;
            ready  AT %IX1.7 : BOOL;
            lamps  AT %QW0   : WORD;
        END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let mut plc = TestPlc::load(&wasm).expect("load");

    let status = plc.located("%IW0").expect("status").clone();
    assert!(status.part_of.is_none(), "the owner is a cell");
    let ready = plc.located("%IX1.7").expect("ready").clone();
    assert_eq!(ready.name, "ready", "a declared part keeps its name");
    let part = ready.part_of.as_ref().expect("a part");
    assert_eq!((part.owner.as_str(), part.shift), ("%IW0", 15));
    assert_eq!(ready.addr, status.addr, "it lives in the owner's cell");
    let byte = plc.located("%IB1").expect("a bare part");
    assert_eq!(byte.part_of.as_ref().map(|p| p.shift), Some(8));

    // Bound as a host would: by the part's own address.
    plc.write_located("%IX1.7", &1i32.to_le_bytes()).expect("set the bit");
    plc.write_located("%IB1", &0x80i32.to_le_bytes()).expect("the byte, same bit");
    plc.run(1).expect("scan");
    let low = |plc: &TestPlc, a: &str| {
        i32::from_le_bytes(plc.read_located(a).expect("read")[..4].try_into().unwrap())
    };
    assert_eq!(low(&plc, "%IW0"), 0x8000, "the byte's top bit is `ready`");
    assert_eq!(low(&plc, "%QX0.0"), 1, "`ready` reached the output bit");
    assert_eq!(low(&plc, "%QB1"), 0x80, "and the byte its output byte");
}

/// A part written into an owner declared with a SIGNED type keeps the
/// owner's sign: rk holds a 16-bit INT sign-extended in its 32-bit lane, and
/// the read-modify-write rebuilds only its low 16 bits. -8 with bit 0 set is
/// -7, not 65529. (The flags go to `%QX2.x`: `%QW1` is bytes 2 and 3, and
/// `%QX1.0` would be byte 1 — the high byte of `x` itself.)
#[rstest]
fn a_part_written_into_a_signed_owner_keeps_its_sign(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR_EXTERNAL x : INT; END_VAR
            x := -8;
            %QX0.0 := TRUE;
            %QX2.0 := x = -7;
            %QX2.1 := x < 0;
        END_PROGRAM

        CONFIGURATION Cfg
        VAR_GLOBAL x AT %QW0 : INT; flags AT %QW1 : WORD; END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let mut plc = TestPlc::load(&wasm).expect("load");
    plc.run(1).expect("scan");
    let flags = i32::from_le_bytes(plc.read_located("%QW1").expect("read")[..4].try_into().unwrap());
    assert_eq!(flags & 0b11, 0b11, "x is -7 and still negative");
}

/// A host writes a part by replacing its bits in the owner's cell, with no
/// notion of the owner's type. Setting the top bit of an INT that way makes
/// it negative: only the owner's own two bytes are its value, whatever the
/// rest of its four-byte slot holds.
#[rstest]
fn a_host_setting_the_sign_bit_of_a_signed_owner_makes_it_negative(
    mut with_db: db::RootDatabase,
) {
    let source = r#"
        PROGRAM P
        VAR_EXTERNAL level : INT; END_VAR
            %QX0.0 := level < 0;
            %QX0.1 := level = -32768;
        END_PROGRAM

        CONFIGURATION Cfg
        VAR_GLOBAL level AT %IW0 : INT; flags AT %QW0 : WORD; END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let mut plc = TestPlc::load(&wasm).expect("load");
    // `%IX1.7` is not in the program: the map lists only what it names, so
    // the host sets bit 15 through the owner, the way a part is written.
    let cell = i32::from_le_bytes(plc.read_located("%IW0").expect("read")[..4].try_into().unwrap());
    plc.write_located("%IW0", &(cell | 0x8000).to_le_bytes())
        .expect("set the top bit");
    plc.run(1).expect("scan");
    let flags = i32::from_le_bytes(plc.read_located("%QW0").expect("read")[..4].try_into().unwrap());
    assert_eq!(flags & 0b11, 0b11, "16#8000 in an INT is -32768");
}

/// A part may be declared with any type of its width. A signed one reads
/// its bits sign-extended, and a write stores its low bits into the owner:
/// 16#80 in the high byte of `%IW0` is -128 in `hi`, and -2 in the high word
/// of `%MD0` is 16#FFFE there.
#[rstest]
fn a_signed_part_reads_and_writes_signed(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR_EXTERNAL lo : SINT; hi : SINT; m : INT; END_VAR
            %QX0.0 := lo = -1;
            %QX0.1 := hi = -128;
            %QX0.2 := hi < lo;
            m := -2;
            %QX0.3 := m = -2;
        END_PROGRAM

        CONFIGURATION Cfg
        VAR_GLOBAL
            status AT %IW0 : WORD;
            lo     AT %IB0 : SINT;
            hi     AT %IB1 : SINT;
            frame  AT %MD0 : DWORD;
            m      AT %MW1 : INT;
            flags  AT %QW0 : WORD;
        END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let mut plc = TestPlc::load(&wasm).expect("load");
    plc.write_located("%IW0", &0x80FFi32.to_le_bytes())
        .expect("write the input word");
    plc.run(1).expect("scan");
    let read = |plc: &TestPlc, a: &str| {
        u32::from_le_bytes(plc.read_located(a).expect("read")[..4].try_into().unwrap())
    };
    assert_eq!(read(&plc, "%QW0") & 0b1111, 0b1111, "-1 and -128, and m read back -2");
    assert_eq!(read(&plc, "%MD0"), 0xFFFE_0000, "m's bits are the high word");
}

/// A 32-bit part of a 64-bit address is four whole bytes of its cell, so a
/// REAL there reads and writes its bits unconverted, as do a DINT and a bare
/// `%ID1`.
#[rstest]
fn a_real_part_is_its_bits(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR_EXTERNAL count : DINT; gain : REAL; out : REAL; lo : DINT; hi : REAL; END_VAR
            out := gain * 2.0;
            lo := count - 1;
            hi := 1.5;
            %QD5 := %ID1;
        END_PROGRAM

        CONFIGURATION Cfg
        VAR_GLOBAL
            frame AT %IL0 : LWORD;
            count AT %ID0 : DINT;
            gain  AT %ID1 : REAL;
            out   AT %QD4 : REAL;
            reply AT %QL0 : LWORD;
            lo    AT %QD0 : DINT;
            hi    AT %QD1 : REAL;
        END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let mut plc = TestPlc::load(&wasm).expect("load");
    let frame = (u64::from(2.5f32.to_bits()) << 32) | u64::from(-4i32 as u32);
    plc.write_located("%IL0", &frame.to_le_bytes())
        .expect("write the input frame");
    plc.run(1).expect("scan");
    let word = |plc: &TestPlc, a: &str| {
        u32::from_le_bytes(plc.read_located(a).expect("read")[..4].try_into().unwrap())
    };
    assert_eq!(f32::from_bits(word(&plc, "%QD4")), 5.0, "gain was 2.5");
    assert_eq!(word(&plc, "%QD5"), 2.5f32.to_bits(), "the bare part, copied raw");
    let reply = u64::from_le_bytes(plc.read_located("%QL0").expect("read")[..8].try_into().unwrap());
    assert_eq!(
        reply,
        (u64::from(1.5f32.to_bits()) << 32) | u64::from(-5i32 as u32),
        "lo in the low half, hi's bits in the high one"
    );
}

/// An owner declared REAL is sliced as the bit string it is in memory:
/// `%QX3.7` is the sign bit of the REAL at `%QD0`.
#[rstest]
fn a_bit_of_a_real_owner_is_a_bit_of_its_encoding(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR_EXTERNAL r : REAL; END_VAR
            r := 1.5;
            %QX3.7 := TRUE;
            %QX4.0 := r = -1.5;
            %QX4.1 := %QX3.6;
        END_PROGRAM

        CONFIGURATION Cfg
        VAR_GLOBAL r AT %QD0 : REAL; END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let mut plc = TestPlc::load(&wasm).expect("load");
    plc.run(1).expect("scan");
    let word = |plc: &TestPlc, a: &str| {
        u32::from_le_bytes(plc.read_located(a).expect("read")[..4].try_into().unwrap())
    };
    assert_eq!(f32::from_bits(word(&plc, "%QD0")), -1.5);
    assert_eq!(word(&plc, "%QX4.0"), 1, "the program reads -1.5 back");
    assert_eq!(word(&plc, "%QX4.1"), 0, "bit 30 of 1.5 is clear");
}

/// An output's initial value is written by `__init`, so the output is in
/// that state before the first scan — the startup value a host sees first.
#[rstest]
fn an_output_starts_at_its_initial_value(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR_EXTERNAL lamps : WORD; END_VAR
            lamps := lamps OR 16#0001;
        END_PROGRAM

        CONFIGURATION Cfg
        VAR_GLOBAL lamps AT %QW0 : WORD := 16#00F0; END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let mut plc = TestPlc::load(&wasm).expect("load");
    let read = |plc: &TestPlc| {
        i32::from_le_bytes(plc.read_located("%QW0").expect("read")[..4].try_into().unwrap())
    };
    assert_eq!(read(&plc), 0x00F0, "`__init` wrote the startup value");
    plc.run(1).expect("scan");
    assert_eq!(read(&plc), 0x00F1);
}

/// A bit may leave out its size character (Table 16 row 4b), and is then the
/// same address with `X`: `%Q0.3` is bit 3 of a `%QW0` beside it, and `%I1`
/// is `%IX1`, a cell of its own.
#[rstest]
fn an_address_without_a_width_letter_is_a_bit(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR_EXTERNAL lamps : WORD; END_VAR
            %Q0.3 := TRUE;
            %Q0.0 := %I1;
        END_PROGRAM

        CONFIGURATION Cfg
        VAR_GLOBAL lamps AT %QW0 : WORD; END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    assert_eq!(mir.output_size, 4, "`%Q0.3` and `%Q0.0` are `lamps`'s bits");
    let mut plc = TestPlc::load(&wasm).expect("load");
    plc.write_located("%I1", &1i32.to_le_bytes()).expect("input bit");
    plc.run(1).expect("scan");
    let lamps = i32::from_le_bytes(plc.read_located("%QW0").expect("read")[..4].try_into().unwrap());
    assert_eq!(lamps, 0b1001);
}
