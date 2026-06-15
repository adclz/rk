//! `debug-functions` / wasm `name` section: map a wasm `DefinedFuncIndex` (as
//! reported by wasmtime's `FrameHandle`) to its IEC function name, for naming a
//! debugger's stack frames.

use crate::tests::{compile_to_mir_and_wasm, with_db};
use rstest::*;
use runtime::{Config, Plc};

/// Extract and decode the `debug-functions` custom section from a core module.
fn read_debug_functions(wasm: &[u8]) -> debug_format::DebugFunctions {
    for payload in wasmparser::Parser::new(0).parse_all(wasm) {
        if let Ok(wasmparser::Payload::CustomSection(reader)) = payload
            && reader.name() == debug_format::DEBUG_FUNCTIONS_SECTION
        {
            return debug_format::DebugFunctions::from_msgpack(reader.data())
                .expect("valid debug-functions section");
        }
    }
    panic!("module is missing the `debug-functions` custom section");
}

/// Each defined function is named by its `DefinedFuncIndex`, and the runtime
/// resolves that index back to the same name.
#[rstest]
fn functions_named_by_defined_index(mut with_db: db::RootDatabase) {
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
    let df = read_debug_functions(&wasm);

    // Entries are sorted, uniquely indexed, and cover the functions we wrote
    // plus the synthesized program body.
    assert_eq!(df.version, debug_format::DEBUG_FUNCTIONS_VERSION);
    assert!(df.functions.windows(2).all(|w| w[0].defined_index < w[1].defined_index));
    let names: Vec<&str> = df.functions.iter().map(|f| f.name.as_str()).collect();
    assert!(names.iter().any(|n| n.contains("add")), "no `add` in {names:?}");
    assert!(names.iter().any(|n| n.contains("mul")), "no `mul` in {names:?}");
    assert!(names.iter().any(|n| n.contains("Main")), "no program body in {names:?}");

    // The runtime resolves every DefinedFuncIndex back to its name; unknown
    // indices (imports, builtins, out-of-range) resolve to None.
    let plc = Plc::load(&wasm, Config::default()).expect("load PLC");
    assert!(!df.functions.is_empty());
    for f in &df.functions {
        assert_eq!(
            plc.function_name(f.defined_index),
            Some(f.name.as_str()),
            "name for defined index {}",
            f.defined_index
        );
    }
    assert!(plc.function_name(u32::MAX).is_none());
}
