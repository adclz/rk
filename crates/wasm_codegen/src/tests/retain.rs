//! Persistent-state tests: the RETAIN band and its exported bounds. Programs
//! are instance-based, so they are only runnable when instantiated in a
//! CONFIGURATION and driven by the runtime.

use crate::tests::{compile_to_mir_and_wasm, with_db};
use rstest::*;
use runtime::{Config, Plc};

/// Builtin shadow-stack + data live below this; no IEC variable may sit lower.
const BUILTIN_RESERVED_FLOOR: u32 = 16_384;

fn temp_path(tag: &str) -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("rk_retain_{tag}_{}.bin", std::process::id()));
    let _ = std::fs::remove_file(&p);
    p
}

/// Read the module's exported `retain_base` / `retain_size` globals.
fn read_retain_globals(wasm: &[u8]) -> (i32, i32) {
    let engine = wasmtime::Engine::default();
    let module = wasmtime::Module::new(&engine, wasm).expect("valid module");
    let mut store = wasmtime::Store::new(&engine, ());
    let memory =
        wasmtime::Memory::new(&mut store, wasmtime::MemoryType::new(1, None)).expect("memory");
    let mut linker = wasmtime::Linker::new(&engine);
    linker
        .define(&store, "env", "memory", memory)
        .expect("define env.memory");
    let instance = linker
        .instantiate(&mut store, &module)
        .expect("instantiate");
    let read = |store: &mut wasmtime::Store<()>, name: &str| {
        instance
            .get_global(&mut *store, name)
            .unwrap_or_else(|| panic!("module exports global '{name}'"))
            .get(store)
            .i32()
            .expect("global is i32")
    };
    let base = read(&mut store, "retain_base");
    let size = read(&mut store, "retain_size");
    (base, size)
}

/// A `VAR RETAIN` scalar in a configured program lands in the exported retain
/// band and persists across a power cycle.
#[rstest]
fn retain_var_lives_in_band_and_persists(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM ProgA
        VAR RETAIN counter : INT; END_VAR
            counter := counter + 1;
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P WITH T : ProgA;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);

    // One retained INT => a 4-byte band above the builtin floor, and the
    // exported globals match the MIR.
    assert_eq!(mir.retain_size, 4, "one INT retained => 4-byte band");
    assert!(mir.retain_base >= BUILTIN_RESERVED_FLOOR);
    assert_eq!(
        read_retain_globals(&wasm),
        (mir.retain_base as i32, mir.retain_size as i32)
    );

    // Driven by the runtime: the counter persists across a power cycle.
    let counter = |plc: &Plc| i32::from_le_bytes(plc.read_retain()[..4].try_into().unwrap());
    let path = temp_path("persist");
    {
        let mut plc = Plc::load(
            &wasm,
            Config {
                entry: None,
                retain_path: Some(path.clone()),
            },
        )
        .expect("load (boot 1)");
        plc.run(3).expect("scans");
        assert_eq!(counter(&plc), 3);
        plc.snapshot_retain().expect("snapshot");
    }
    {
        let mut plc = Plc::load(
            &wasm,
            Config {
                entry: None,
                retain_path: Some(path.clone()),
            },
        )
        .expect("load (boot 2)");
        assert_eq!(counter(&plc), 3, "restored from snapshot");
        plc.run(2).expect("scans");
        assert_eq!(counter(&plc), 5);
    }
    std::fs::remove_file(&path).ok();
}

/// A plain (non-RETAIN) program var produces no retain band.
#[rstest]
fn plain_program_var_has_no_retain_band(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM ProgA
        VAR counter : INT; END_VAR
            counter := counter + 1;
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P WITH T : ProgA;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    assert_eq!(mir.retain_size, 0, "no RETAIN vars => empty retain band");
    assert_eq!(read_retain_globals(&wasm).1, 0);
}
