//! Where every variable starts at entry.

use crate::tests::codegen::{TestPlc, compile_to_mir_and_wasm, with_db};
use debug_format::{DebugInfo, VarValue};
use rstest::*;

/// A VAR_EXTERNAL is the global's storage and a VAR_IN_OUT the caller's:
/// neither starts over. The external's type defaults were written at every
/// call anyway, from address 0 on, over the caller's locals.
#[rstest]
fn a_var_external_or_in_out_writes_nothing_at_entry(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE
            Big : ARRAY[0..4999] OF DINT := [5000(-1)];
            Small : ARRAY[0..9] OF DINT := [10(-1)];
        END_TYPE

        FUNCTION Touch : DINT
        VAR_EXTERNAL gb : Big; END_VAR
            Touch := gb[0];
        END_FUNCTION

        FUNCTION Locals : DINT
        VAR s : ARRAY[0..9] OF DINT; d : DINT; END_VAR
            s[0] := 42;
            s[9] := 43;
            d := Touch();
            Locals := s[0] * 100 + s[9];
        END_FUNCTION

        FUNCTION Keep : DINT
        VAR_IN_OUT b : Small; END_VAR
            Keep := b[3];
            b[4] := 7;
        END_FUNCTION

        PROGRAM P
        VAR
            fromGlobal : DINT;
            frame : DINT;
            kept : DINT;
            written : DINT;
            mine : Small;
        END_VAR
            fromGlobal := Touch();
            frame := Locals();
            mine[3] := 42;
            kept := Keep(b := mine);
            written := mine[4];
        END_PROGRAM

        CONFIGURATION Cfg
        VAR_GLOBAL gb : Big; END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let mut plc = TestPlc::load(&wasm).expect("load");
    let dbg = DebugInfo::from_wasm(&wasm);
    plc.run(1).expect("scan");
    let read = |path| plc.read_var(&dbg, path);
    assert_eq!(
        read("P1.fromGlobal"),
        Some(VarValue::I32(-1)),
        "__init wrote the global"
    );
    assert_eq!(
        read("P1.frame"),
        Some(VarValue::I32(4243)),
        "the caller's array is untouched"
    );
    assert_eq!(
        read("P1.kept"),
        Some(VarValue::I32(42)),
        "the caller's element"
    );
    assert_eq!(read("P1.written"), Some(VarValue::I32(7)));
}
