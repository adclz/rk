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

/// A retained program embedding an FB with a VAR_IN_OUT field. The retain band
/// is whole-instance granular, so the FB's by-ref pointer field sits inside the
/// snapshot — and a restored (stale) pointer is harmless because the call site
/// re-stores `&arg` before every `$__body__` call. IEC-wise VAR_IN_OUT itself
/// can never be RETAIN (grammar-rejected, E0050); this covers the embedded
/// case.
#[rstest]
fn retained_program_with_fb_inout_survives_power_cycle(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK doubler
            VAR_IN_OUT v : INT; END_VAR
            v := v * 2;
        END_FUNCTION_BLOCK

        PROGRAM ProgA
        VAR RETAIN acc : INT; END_VAR
        VAR d : doubler; END_VAR
            acc := acc + 1;
            d(v := acc);
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P WITH T : ProgA;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);

    // Whole-instance banding: acc (4) + the doubler instance (one 4-byte
    // pointer field) => 8 bytes. The pointer IS part of the snapshot today.
    assert_eq!(mir.retain_size, 8, "acc + embedded doubler pointer field");

    // acc doubles through the inout each scan: 0 ->2 ->6 ->14.
    let acc = |plc: &Plc| i32::from_le_bytes(plc.read_retain()[..4].try_into().unwrap());
    let path = temp_path("fb_inout_retain");
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
        assert_eq!(acc(&plc), 14);
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
        assert_eq!(acc(&plc), 14, "restored from snapshot");
        // The restored pointer field is stale until the first call re-stores
        // it — the scan must still work: (14+1)*2 = 30.
        plc.run(1).expect("scan after restore");
        assert_eq!(acc(&plc), 30, "inout works after restore");
    }
    std::fs::remove_file(&path).ok();
}

/// FB-internal `VAR RETAIN` persists for every instance:
/// the program itself declares nothing RETAIN — its only retained state lives
/// inside the nested FB. (Regression: the retain walk only looked at the
/// program's own qualifiers, so this state was silently never persisted —
/// retain_size was 0.)
#[rstest]
fn fb_internal_retain_persists_power_cycle(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Counter
        VAR RETAIN c : INT; END_VAR
            c := c + 1;
        END_FUNCTION_BLOCK

        PROGRAM ProgA
        VAR d : Counter; END_VAR
            d();
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P WITH T : ProgA;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);

    // Whole-instance banding: the program instance is just the Counter (4 B).
    assert_eq!(mir.retain_size, 4, "nested FB retain state must be banded");

    let c = |plc: &Plc| i32::from_le_bytes(plc.read_retain()[..4].try_into().unwrap());
    let path = temp_path("fb_internal_retain");
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
        assert_eq!(c(&plc), 3);
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
        assert_eq!(c(&plc), 3, "FB-internal retained counter restored");
        plc.run(2).expect("scans");
        assert_eq!(c(&plc), 5);
    }
    std::fs::remove_file(&path).ok();
}

/// Config-level `PROGRAM RETAIN`: persists the instance even though the
/// program declaration has no RETAIN qualifiers at all.
#[rstest]
fn config_level_retain_bands_program(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM ProgA
        VAR counter : INT; END_VAR
            counter := counter + 1;
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM RETAIN P WITH T : ProgA;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    assert_eq!(mir.retain_size, 4, "config-level RETAIN bands the instance");

    let c = |plc: &Plc| i32::from_le_bytes(plc.read_retain()[..4].try_into().unwrap());
    let path = temp_path("cfg_retain");
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
        assert_eq!(c(&plc), 3);
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
        assert_eq!(c(&plc), 3, "persisted via config-level RETAIN");
    }
    std::fs::remove_file(&path).ok();
}

/// Config-level `PROGRAM NON_RETAIN` suppresses persistence even though the
/// program declares `VAR RETAIN` state. (Regression: the builder read the
/// qualifier with `is_some()`, so NON_RETAIN parsed as retained.)
#[rstest]
fn config_level_non_retain_suppresses(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM ProgA
        VAR RETAIN counter : INT; END_VAR
            counter := counter + 1;
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM NON_RETAIN P WITH T : ProgA;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (mir, _wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    assert_eq!(
        mir.retain_size, 0,
        "NON_RETAIN overrides the declaration's RETAIN qualifier"
    );
}

/// Explicit NON_RETAIN prunes the whole subtree: the
/// program var is NON_RETAIN, and even though its FB type declares a RETAIN
/// fb variable inside, nothing persists — explicit NON_RETAIN is a hard
/// "never persist this" switch, unlike mere absence of a qualifier.
#[rstest]
fn non_retain_var_prunes_internal_retain(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Inner
        VAR RETAIN c : INT; END_VAR
            c := c + 1;
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK Mid
        VAR RETAIN f : Inner; END_VAR
            f();
        END_FUNCTION_BLOCK

        PROGRAM ProgA
        VAR NON_RETAIN b : Mid; END_VAR
            b();
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P WITH T : ProgA;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (mir, _wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    assert_eq!(
        mir.retain_size, 0,
        "NON_RETAIN on the instance prunes internal RETAIN state"
    );
}

/// Mere ABSENCE of a qualifier lets nested RETAIN shine through, across two
/// unqualified levels: prog -> Mid -> Inner(VAR RETAIN c).
#[rstest]
fn unqualified_nesting_lets_retain_shine_through(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Inner
        VAR RETAIN c : INT; END_VAR
            c := c + 1;
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK Mid
        VAR f : Inner; END_VAR
            f();
        END_FUNCTION_BLOCK

        PROGRAM ProgA
        VAR b : Mid; END_VAR
            b();
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P WITH T : ProgA;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (mir, _wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    assert_eq!(
        mir.retain_size, 4,
        "two unqualified levels above a VAR RETAIN still band the instance"
    );
}
