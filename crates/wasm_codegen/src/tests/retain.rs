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

/// THE per-field proof: a non-retained var sharing a program with a retained
/// one must COLD-START on every boot (keep its __init value), while the
/// retained one is restored. Under whole-instance persistence both came back —
/// `b` resurrected as 103 instead of re-initializing to 100.
#[rstest]
fn non_retained_var_cold_starts_beside_retained(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM ProgA
        VAR RETAIN a : INT; END_VAR
        VAR b : INT := 100; END_VAR
            a := a + 1;
            b := b + 1;
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P WITH T : ProgA;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);

    // Band layout: [a: 0..4][b: 4..8] (whole instance still relocated).
    let a = |plc: &Plc| i32::from_le_bytes(plc.read_retain()[0..4].try_into().unwrap());
    let b = |plc: &Plc| i32::from_le_bytes(plc.read_retain()[4..8].try_into().unwrap());
    let path = temp_path("per_field_proof");
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
        assert_eq!(a(&plc), 3);
        assert_eq!(b(&plc), 103);
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
        assert_eq!(a(&plc), 3, "retained var restored");
        assert_eq!(b(&plc), 100, "non-retained var must cold-start to its init");
        plc.run(1).expect("scan");
        assert_eq!(a(&plc), 4);
        assert_eq!(b(&plc), 101);
    }
    std::fs::remove_file(&path).ok();
}

/// Property test over the retain-map manifest: ranges are path-sorted,
/// disjoint, inside the band, and never overlap a by-ref pointer hole.
#[rstest]
fn retain_map_ranges_are_sound(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK doubler
            VAR_IN_OUT v : INT; END_VAR
            v := v * 2;
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK worker
        VAR RETAIN hits : INT; END_VAR
        VAR NON_RETAIN scratch : INT; END_VAR
        VAR d : doubler; x : INT; END_VAR
            x := x + 1;
            d(v := x);
            hits := hits + 1;
        END_FUNCTION_BLOCK

        PROGRAM ProgA
        VAR RETAIN a : INT; s : STRING[8]; END_VAR
        VAR b : INT := 100; END_VAR
        VAR w : worker; END_VAR
            a := a + 1;
            b := b + 1;
            w();
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P WITH T : ProgA;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (mir, _wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let map = &mir.retain_map;
    assert!(!map.ranges.is_empty());

    // Sorted by path, disjoint, in-band.
    let band = mir.retain_base..mir.retain_base + mir.retain_size;
    let mut sorted_by_addr: Vec<_> = map.ranges.iter().collect();
    sorted_by_addr.sort_by_key(|r| r.addr);
    for w in map.ranges.windows(2) {
        assert!(w[0].path < w[1].path, "ranges sorted by path");
    }
    for w in sorted_by_addr.windows(2) {
        assert!(
            w[0].addr + w[0].size <= w[1].addr,
            "ranges disjoint: {} and {}",
            w[0].path,
            w[1].path
        );
    }
    for r in &map.ranges {
        assert!(
            band.contains(&r.addr) && r.addr + r.size <= band.end,
            "range {} in band",
            r.path
        );
        assert!(r.size > 0);
    }
    // Expected retained set: a, s (RETAIN) + w.hits (internal RETAIN);
    // excluded: b (plain), w.scratch (NON_RETAIN), w.d.v (by-ref pointer),
    // w.x (plain).
    let paths: Vec<&str> = map.ranges.iter().map(|r| r.path.as_str()).collect();
    assert_eq!(paths, vec!["P.a", "P.s", "P.w.hits"]);
    assert_eq!(map.payload_size(), 4 + (4 + 8) + 4, "a + STRING[8] + hits");
}

/// The NON_RETAIN carve-out INSIDE a retained parent — expressible only with
/// per-field persistence: `VAR RETAIN w : Fb` retains the instance deeply,
/// but the FB's NON_RETAIN member cold-starts.
#[rstest]
fn non_retain_carveout_inside_retained_parent(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK holder
        VAR c : INT; END_VAR
        VAR NON_RETAIN scratch : INT; END_VAR
            c := c + 1;
            scratch := scratch + 1;
        END_FUNCTION_BLOCK

        PROGRAM ProgA
        VAR RETAIN h : holder; END_VAR
            h();
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P WITH T : ProgA;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    // Only h.c is persisted; h.scratch is carved out.
    let paths: Vec<&str> = mir.retain_map.ranges.iter().map(|r| r.path.as_str()).collect();
    assert_eq!(paths, vec!["P.h.c"]);

    let c = |plc: &Plc| i32::from_le_bytes(plc.read_retain()[0..4].try_into().unwrap());
    let scratch = |plc: &Plc| i32::from_le_bytes(plc.read_retain()[4..8].try_into().unwrap());
    let path = temp_path("carveout");
    {
        let mut plc = Plc::load(
            &wasm,
            Config {
                entry: None,
                retain_path: Some(path.clone()),
            },
        )
        .expect("boot 1");
        plc.run(3).expect("scans");
        assert_eq!((c(&plc), scratch(&plc)), (3, 3));
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
        .expect("boot 2");
        assert_eq!(c(&plc), 3, "retained member restored");
        assert_eq!(scratch(&plc), 0, "NON_RETAIN member cold-starts");
    }
    std::fs::remove_file(&path).ok();
}

/// `VAR RETAIN d : SomeFb` retains the nested instance deeply (unqualified
/// members inherit retention), across a power cycle.
#[rstest]
fn retained_fb_instance_deep_persists(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK counter
        VAR c : INT; END_VAR
            c := c + 1;
        END_FUNCTION_BLOCK

        PROGRAM ProgA
        VAR RETAIN d : counter; END_VAR
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
    let paths: Vec<&str> = mir.retain_map.ranges.iter().map(|r| r.path.as_str()).collect();
    assert_eq!(paths, vec!["P.d.c"]);

    let c = |plc: &Plc| i32::from_le_bytes(plc.read_retain()[0..4].try_into().unwrap());
    let path = temp_path("deep_retain");
    {
        let mut plc = Plc::load(
            &wasm,
            Config {
                entry: None,
                retain_path: Some(path.clone()),
            },
        )
        .expect("boot 1");
        plc.run(2).expect("scans");
        assert_eq!(c(&plc), 2);
        plc.snapshot_retain().expect("snapshot");
    }
    {
        let plc = Plc::load(
            &wasm,
            Config {
                entry: None,
                retain_path: Some(path.clone()),
            },
        )
        .expect("boot 2");
        assert_eq!(c(&plc), 2, "deep-retained member restored");
    }
    std::fs::remove_file(&path).ok();
}

/// A retained STRING round-trips through the v2 file.
#[rstest]
fn retained_string_survives_power_cycle(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM ProgA
        VAR RETAIN s : STRING; END_VAR
        VAR done : BOOL; END_VAR
            IF NOT done THEN
                s := 'persisted!';
                done := TRUE;
            END_IF;
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P WITH T : ProgA;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let read_s = |plc: &Plc| {
        let r = plc.read_retain();
        let len = i32::from_le_bytes(r[0..4].try_into().unwrap()) as usize;
        String::from_utf8_lossy(&r[4..4 + len]).to_string()
    };
    let path = temp_path("retain_string");
    {
        let mut plc = Plc::load(
            &wasm,
            Config {
                entry: None,
                retain_path: Some(path.clone()),
            },
        )
        .expect("boot 1");
        plc.run(1).expect("scan");
        assert_eq!(read_s(&plc), "persisted!");
        plc.snapshot_retain().expect("snapshot");
    }
    {
        let plc = Plc::load(
            &wasm,
            Config {
                entry: None,
                retain_path: Some(path.clone()),
            },
        )
        .expect("boot 2");
        assert_eq!(read_s(&plc), "persisted!", "string restored before any scan");
    }
    std::fs::remove_file(&path).ok();
}

/// The layout-hash guard: a file whose payload LENGTH matches but whose
/// retained layout differs (different path set) is rejected → cold start.
/// This is exactly the hole the old length-only check couldn't catch.
#[rstest]
fn same_length_different_layout_file_is_rejected(mut with_db: db::RootDatabase) {
    let make = |db: &mut db::RootDatabase, var: &str| {
        let source = format!(
            r#"
        PROGRAM ProgA
        VAR RETAIN {var} : INT; END_VAR
            {var} := {var} + 1;
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P WITH T : ProgA;
            END_RESOURCE
        END_CONFIGURATION
    "#
        );
        compile_to_mir_and_wasm(db, &source).1
    };
    let wasm_a = make(&mut with_db, "alpha");
    let mut db2 = db::RootDatabase::default();
    let wasm_b = make(&mut db2, "beta");

    let counter = |plc: &Plc| i32::from_le_bytes(plc.read_retain()[0..4].try_into().unwrap());
    let path = temp_path("layout_guard");
    {
        let mut plc = Plc::load(
            &wasm_a,
            Config {
                entry: None,
                retain_path: Some(path.clone()),
            },
        )
        .expect("boot A");
        plc.run(5).expect("scans");
        plc.snapshot_retain().expect("snapshot");
    }
    {
        // Same payload length (one INT), different path (alpha vs beta) →
        // layout hash differs → cold start, not a silent wrong restore.
        let plc = Plc::load(
            &wasm_b,
            Config {
                entry: None,
                retain_path: Some(path.clone()),
            },
        )
        .expect("boot B");
        assert_eq!(counter(&plc), 0, "different layout must cold-start");
    }
    std::fs::remove_file(&path).ok();
}

/// Retained vs non-retained VAR_GLOBALs: per-variable persistence held before
/// and still holds — g1 restored, g2 cold-starts.
#[rstest]
fn retained_global_persists_nonretained_cold_starts(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM ProgA
        VAR_EXTERNAL g1 : INT; g2 : INT; END_VAR
            g1 := g1 + 1;
            g2 := g2 + 1;
        END_PROGRAM

        CONFIGURATION Cfg
            VAR_GLOBAL RETAIN g1 : INT; END_VAR
            VAR_GLOBAL g2 : INT; END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P WITH T : ProgA;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let paths: Vec<&str> = mir.retain_map.ranges.iter().map(|r| r.path.as_str()).collect();
    assert_eq!(paths, vec!["g1"]);

    let g1 = |plc: &Plc| i32::from_le_bytes(plc.read_retain()[0..4].try_into().unwrap());
    let path = temp_path("global_split");
    {
        let mut plc = Plc::load(
            &wasm,
            Config {
                entry: None,
                retain_path: Some(path.clone()),
            },
        )
        .expect("boot 1");
        plc.run(3).expect("scans");
        assert_eq!(g1(&plc), 3);
        plc.snapshot_retain().expect("snapshot");
    }
    {
        let plc = Plc::load(
            &wasm,
            Config {
                entry: None,
                retain_path: Some(path.clone()),
            },
        )
        .expect("boot 2");
        assert_eq!(g1(&plc), 3, "retained global restored");
    }
    std::fs::remove_file(&path).ok();
}
