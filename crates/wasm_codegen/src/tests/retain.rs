//! Tests for persistent PROGRAM state: the RETAIN storage class and the
//! contiguous retain band the runtime snapshots, plus the Step 2a guarantee
//! that any persistent PROGRAM `VAR` survives across scan calls (lives in
//! linear memory, not a wasm local that resets every call).

use crate::tests::{compile_to_mir_and_wasm, with_db};
use mir::function::{MirLocal, MirStorage, MirVariableStorage};
use rstest::*;

/// Builtin shadow-stack + data live below this; no IEC variable may sit lower.
const BUILTIN_RESERVED_FLOOR: u32 = 16_384;

/// Locate a program's local variable in the lowered MIR.
fn find_local<'a>(
    mir: &'a mir::MirModule,
    db: &db::RootDatabase,
    program: &str,
    var: &str,
) -> &'a MirLocal {
    let prog = mir
        .functions
        .iter()
        .find(|f| f.origin_name.text(db).as_str() == program)
        .unwrap_or_else(|| panic!("program function '{program}' not found"));
    prog.locals
        .iter()
        .find(|l| l.name.text(db).as_str() == var)
        .unwrap_or_else(|| panic!("local '{var}' not found in '{program}'"))
}

fn memory_address(local: &MirLocal) -> u32 {
    match local.storage {
        MirStorage::Memory { address, .. } => address,
        other => panic!("expected memory storage, got {other:?}"),
    }
}

/// Instantiate the core module against one host-owned memory, call its single
/// `() -> ()` function export `scans` times, then read the i32 at `read_addr`.
/// Reusing one memory across calls models the host driving a scan loop.
fn run_scans(wasm: &[u8], scans: usize, read_addr: u32) -> i32 {
    let engine = wasmtime::Engine::default();
    let module = wasmtime::Module::new(&engine, wasm).expect("valid module");
    let export_name = module
        .exports()
        .find_map(|e| match e.ty() {
            wasmtime::ExternType::Func(_) => Some(e.name().to_string()),
            _ => None,
        })
        .expect("module has a function export");

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
    let scan = instance
        .get_typed_func::<(), ()>(&mut store, &export_name)
        .unwrap_or_else(|_| panic!("program export '{export_name}' not callable"));

    for _ in 0..scans {
        scan.call(&mut store, ()).expect("scan call failed");
    }

    let mut buf = [0u8; 4];
    memory
        .read(&store, read_addr as usize, &mut buf)
        .expect("read linear memory");
    i32::from_le_bytes(buf)
}

/// Read the `retain_base` / `retain_size` globals the module exports for the
/// host runtime.
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

/// A `VAR RETAIN` scalar lands in the exported retain band and keeps its value
/// across repeated scans against the same memory.
#[rstest]
fn retain_scalar_lives_in_band_and_persists(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM Main
        VAR RETAIN counter : INT; END_VAR
            counter := counter + 1;
        END_PROGRAM
    "#;

    let (mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);

    // One INT retained => a 4-byte band sitting above the builtin floor.
    assert_eq!(mir.retain_size, 4, "one INT => 4-byte retain band");
    assert!(
        mir.retain_base >= BUILTIN_RESERVED_FLOOR,
        "retain band must sit above the builtin reserved floor, got {}",
        mir.retain_base
    );

    // `counter` is classified Retain, is memory-resident, and (being the only
    // retained var) sits at the band base.
    let counter = find_local(&mir, &with_db, "Main", "counter");
    assert_eq!(counter.var_storage, MirVariableStorage::Retain);
    assert_eq!(
        memory_address(counter),
        mir.retain_base,
        "the lone retain var should be at the band base"
    );

    // Driving the scan 3 times must increment the persisted counter to 3 —
    // proving the var is real memory at the band base, not a resettable local.
    assert_eq!(run_scans(&wasm, 3, mir.retain_base), 3);

    // Step 3: the band bounds are exported as globals matching the MIR, so a
    // host runtime can locate the snapshot region from the binary alone.
    let (base, size) = read_retain_globals(&wasm);
    assert_eq!(base as u32, mir.retain_base, "exported retain_base == MIR");
    assert_eq!(size as u32, mir.retain_size, "exported retain_size == MIR");
    assert_eq!(size, 4, "single INT retained => 4-byte region");
}

/// Control: a plain PROGRAM `VAR` produces no retain band, yet (per Step 2a) is
/// still memory-resident Static state and persists across scans.
#[rstest]
fn plain_program_var_persists_without_a_retain_band(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM Main
        VAR counter : INT; END_VAR
            counter := counter + 1;
        END_PROGRAM
    "#;

    let (mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);

    // No RETAIN declared => empty band.
    assert_eq!(mir.retain_size, 0, "no RETAIN vars => empty retain band");

    // The plain VAR is Static and memory-resident, so it persists anyway.
    let counter = find_local(&mir, &with_db, "Main", "counter");
    assert_eq!(counter.var_storage, MirVariableStorage::Static);
    let addr = memory_address(counter);
    assert_eq!(
        run_scans(&wasm, 3, addr),
        3,
        "a Static PROGRAM var must persist across scans"
    );

    // The bounds globals still exist, reporting an empty region.
    let (_base, size) = read_retain_globals(&wasm);
    assert_eq!(size, 0, "no RETAIN vars => exported retain_size is 0");
}

/// Full pipeline: compile real IEC source to wasm, then drive it through the
/// `runtime` host across a simulated power cycle. The retained counter must
/// continue from its persisted value rather than reset — proving the whole
/// chain (compiler → retain band → exported bounds → host snapshot/restore).
#[rstest]
fn end_to_end_retain_survives_power_cycle(mut with_db: db::RootDatabase) {
    use runtime::{Config, Plc};

    let source = r#"
        PROGRAM Main
        VAR RETAIN counter : INT; END_VAR
            counter := counter + 1;
        END_PROGRAM
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);

    let path = std::env::temp_dir().join(format!("rk_e2e_retain_{}.bin", std::process::id()));
    let _ = std::fs::remove_file(&path);

    let read_counter = |plc: &Plc| i32::from_le_bytes(plc.read_retain()[..4].try_into().unwrap());

    // Boot 1: cold start, run 3 scans, persist to the retain file.
    {
        let cfg = Config {
            entry: None,
            retain_path: Some(path.clone()),
        };
        let mut plc = Plc::load(&wasm, cfg).expect("load (boot 1)");
        plc.run(3).expect("scans");
        assert_eq!(read_counter(&plc), 3);
        plc.snapshot_retain().expect("snapshot");
    }

    // Boot 2 (power cycle): a fresh instance restores and continues 3 -> 5.
    {
        let cfg = Config {
            entry: None,
            retain_path: Some(path.clone()),
        };
        let mut plc = Plc::load(&wasm, cfg).expect("load (boot 2)");
        assert_eq!(read_counter(&plc), 3, "counter restored from the previous boot");
        plc.run(2).expect("scans");
        assert_eq!(read_counter(&plc), 5);
    }

    std::fs::remove_file(&path).ok();
}
