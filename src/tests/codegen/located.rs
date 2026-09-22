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

    // And a band that was never exported reads back as empty.
    let plc = TestPlc::load(&wasm).expect("load");
    assert_eq!(plc.input_region().size, 0);
}
