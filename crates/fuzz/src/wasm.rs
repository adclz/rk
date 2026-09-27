//! What must hold of a module the compiler emitted: it validates, its custom
//! sections decode and agree with it, and it runs the way a host runs it.

use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, LazyLock};

use debug_format::{LocatedArea, LocatedMap, RetainMap, ScheduleManifest};
use wasmparser::{Payload, Validator, WasmFeatures};
use wasmtime::{
    Engine, ExternType, Func, Instance, Linker, Memory, MemoryType, Module, Store, Trap, Val,
    ValType,
};

use crate::Finding;

/// The proposals rk's output may use, and no others: `WASM_FEATURES` in
/// `crates/cli/src/compiler.rs` is what `rk compile -O` enables in wasm-opt,
/// which refuses a module using anything it was not told about. A module
/// that needs more validates in wasmtime and fails the optimized build.
fn features() -> WasmFeatures {
    WasmFeatures::WASM1
        | WasmFeatures::BULK_MEMORY
        | WasmFeatures::BULK_MEMORY_OPT
        | WasmFeatures::SIGN_EXTENSION
        | WasmFeatures::EXCEPTIONS
        | WasmFeatures::MULTI_VALUE
        | WasmFeatures::SATURATING_FLOAT_TO_INT
}

pub(crate) fn validate(wasm: &[u8], profile: &str) -> Result<(), Finding> {
    Validator::new_with_features(features())
        .validate_all(wasm)
        .map(|_| ())
        .map_err(|e| Finding::new("wasm-validate", format!("{profile} module: {e}")))
}

/// The bytes of the named custom section, if the module has one.
fn custom_section<'a>(wasm: &'a [u8], name: &str) -> Option<&'a [u8]> {
    wasmparser::Parser::new(0)
        .parse_all(wasm)
        .find_map(|payload| match payload {
            Ok(Payload::CustomSection(c)) if c.name() == name => Some(c.data()),
            _ => None,
        })
}

/// A section's content with the section itself, for comparing two modules.
/// Custom sections are left out: they are what the profiles differ by.
fn code_and_data(wasm: &[u8]) -> Vec<(u8, Vec<u8>)> {
    wasmparser::Parser::new(0)
        .parse_all(wasm)
        .filter_map(|payload| match payload.ok()? {
            Payload::CodeSectionStart { range, .. } => Some((10, wasm[range].to_vec())),
            Payload::DataSection(r) => Some((11, wasm[r.range()].to_vec())),
            Payload::GlobalSection(r) => Some((6, wasm[r.range()].to_vec())),
            Payload::ImportSection(r) => Some((2, wasm[r.range()].to_vec())),
            _ => None,
        })
        .collect()
}

/// "One loader, two profiles: only the sections differ" (`Profile` in
/// wasm_codegen). The release keeps every function, test wrappers included,
/// so the memory layout, and therefore every address a monitor or a retain
/// file holds, is the debug build's.
pub(crate) fn same_program(debug: &[u8], release: &[u8]) -> Result<(), Finding> {
    let (d, r) = (code_and_data(debug), code_and_data(release));
    for (id, name) in [(2, "import"), (6, "global"), (10, "code"), (11, "data")] {
        let pick = |s: &[(u8, Vec<u8>)]| s.iter().find(|(i, _)| *i == id).map(|(_, b)| b.clone());
        if pick(&d) != pick(&r) {
            return Err(Finding::new(
                "profiles-diverge",
                format!("the debug and release modules have different {name} sections"),
            ));
        }
    }
    Ok(())
}

/// Every custom section this compiler writes decodes, at the version this
/// toolchain reads, and is internally consistent. A section that fails here
/// fails in the runtime, the debugger or `rk test`, on a module that built.
pub(crate) fn decode_sections(wasm: &[u8]) -> Result<Sections, Finding> {
    let bad =
        |section: &str, why: String| Finding::new("custom-section", format!("`{section}`: {why}"));

    let schedule = match custom_section(wasm, debug_format::SCHEDULE_SECTION) {
        None => None,
        Some(data) => {
            let m = ScheduleManifest::from_msgpack(data)
                .map_err(|e| bad(debug_format::SCHEDULE_SECTION, e.to_string()))?;
            if m.version != debug_format::SCHEDULE_VERSION {
                return Err(bad(
                    debug_format::SCHEDULE_SECTION,
                    format!("version {}", m.version),
                ));
            }
            if let Some(t) = m.tasks.iter().find(|t| t.period_ticks == 0) {
                return Err(bad(
                    debug_format::SCHEDULE_SECTION,
                    format!("task `{}` has a period of 0 ticks", t.name),
                ));
            }
            Some(m)
        }
    };

    let retain = match custom_section(wasm, debug_format::RETAIN_MAP_SECTION) {
        None => None,
        Some(data) => {
            let m = RetainMap::from_msgpack(data)
                .map_err(|e| bad(debug_format::RETAIN_MAP_SECTION, e.to_string()))?;
            if m.version != debug_format::RETAIN_MAP_VERSION {
                return Err(bad(
                    debug_format::RETAIN_MAP_SECTION,
                    format!("version {}", m.version),
                ));
            }
            // The runtime compares the hash to decide whether a retain file
            // still fits: a stale one silently discards retained values.
            if RetainMap::new(m.ranges.clone()).layout_hash != m.layout_hash {
                return Err(bad(
                    debug_format::RETAIN_MAP_SECTION,
                    "layout_hash is not the hash of its own ranges".to_string(),
                ));
            }
            Some(m)
        }
    };

    let located = match custom_section(wasm, debug_format::LOCATED_MAP_SECTION) {
        None => None,
        Some(data) => {
            let m = LocatedMap::from_msgpack(data)
                .map_err(|e| bad(debug_format::LOCATED_MAP_SECTION, e.to_string()))?;
            if m.version != debug_format::LOCATED_MAP_VERSION {
                return Err(bad(
                    debug_format::LOCATED_MAP_SECTION,
                    format!("version {}", m.version),
                ));
            }
            if LocatedMap::new(m.entries.clone()).layout_hash != m.layout_hash {
                return Err(bad(
                    debug_format::LOCATED_MAP_SECTION,
                    "layout_hash is not the hash of its own entries".to_string(),
                ));
            }
            Some(m)
        }
    };

    macro_rules! decodes {
        ($section:expr, $ty:ty) => {
            if let Some(data) = custom_section(wasm, $section) {
                <$ty>::from_msgpack(data).map_err(|e| bad($section, e.to_string()))?;
            }
        };
    }
    decodes!(
        debug_format::DEBUG_FUNCTIONS_SECTION,
        debug_format::DebugFunctions
    );
    decodes!(debug_format::DEBUG_LINES_SECTION, debug_format::DebugLines);
    decodes!(
        debug_format::DEBUG_LOCALS_SECTION,
        debug_format::DebugLocals
    );
    decodes!(
        debug_format::DEBUG_SYMBOLS_SECTION,
        debug_format::DebugSymbols
    );
    decodes!(
        debug_format::test_manifest::TEST_MANIFEST_SECTION,
        debug_format::test_manifest::TestManifest
    );

    // What the debugger and the monitor load: a module this toolchain just
    // built has nothing for it to complain about.
    let info = debug_format::DebugInfo::from_wasm(wasm);
    if let Some(problem) = info.problems().first() {
        return Err(bad("debug-*", problem.clone()));
    }

    Ok(Sections {
        schedule,
        retain,
        located,
        info,
    })
}

pub(crate) struct Sections {
    schedule: Option<ScheduleManifest>,
    retain: Option<RetainMap>,
    located: Option<LocatedMap>,
    info: debug_format::DebugInfo,
}

/// One engine for every iteration: building one costs more than most runs.
static ENGINE: LazyLock<Engine> = LazyLock::new(|| {
    let mut config = wasmtime::Config::new();
    // Every module that raises, asserts or subscripts an array carries the
    // `$rk_exception` tag, which the default config refuses to parse.
    config.wasm_exceptions(true);
    // A fuzzed program loops forever as easily as not.
    config.consume_fuel(true);
    Engine::new(&config).expect("wasmtime engine")
});

/// Per call. Enough for a few thousand loop iterations, little enough that
/// an endless WHILE costs the fuzzer milliseconds.
const FUEL: u64 = 2_000_000;

/// Larger modules are declared valid and not run: the host is not what is
/// under test, and a 1 GiB array is a legal program.
const MAX_PAGES: u64 = 1024;

/// How many scans a program gets. More than one, so a second scan starts
/// from the state the first left, which is where RETAIN and FB instances
/// keep their state.
const SCANS: u64 = 3;

/// Instantiate `wasm` as a host does and drive it: `__init`, then a few
/// scans (the task schedule, or every program body), then each `{test}` and
/// `{export}` function once. A trap the program can ask for (a division by
/// zero, a RAISE, running out of fuel) ends the run and is not a finding.
/// One no well-typed program can cause is.
pub(crate) fn execute(wasm: &[u8], sections: &Sections) -> Result<(), Finding> {
    let engine = &*ENGINE;
    let module = Module::new(engine, wasm)
        .map_err(|e| Finding::new("wasmtime-compile", format!("{e:#}")))?;
    let mut store = Store::new(engine, ());

    let Some(min_pages) = module.imports().find_map(|i| match i.ty() {
        ExternType::Memory(m) => Some(m.minimum()),
        _ => None,
    }) else {
        return Err(Finding::new(
            "module-shape",
            "the module does not import its linear memory from `env`",
        ));
    };
    if min_pages > MAX_PAGES {
        return Ok(());
    }
    let memory = Memory::new(&mut store, MemoryType::new(min_pages as u32, None))
        .map_err(|e| Finding::new("module-shape", format!("allocating memory: {e:#}")))?;

    let mut linker = Linker::new(engine);
    linker
        .define(&store, "env", "memory", memory)
        .map_err(|e| Finding::new("module-shape", format!("{e:#}")))?;
    // The clock moves a millisecond a call, so timers run and the run
    // reproduces.
    let clock = Arc::new(AtomicI64::new(0));
    for import in module.imports() {
        if import.name() == "now" && import.module().contains("monotonic-clock") {
            let clock = clock.clone();
            linker
                .func_wrap(import.module(), import.name(), move || {
                    clock.fetch_add(1_000_000, Ordering::Relaxed)
                })
                .map_err(|e| Finding::new("module-shape", format!("{e:#}")))?;
        }
    }
    linker
        .define_unknown_imports_as_traps(&module)
        .map_err(|e| Finding::new("module-shape", format!("{e:#}")))?;

    // Nothing runs during instantiation (there is no start function), so a
    // failure here is the module's own shape: typically a data segment
    // beyond the memory it asked for.
    let instance = linker
        .instantiate(&mut store, &module)
        .map_err(|e| Finding::new("instantiate", format!("{e:#}")))?;

    check_bands(&mut store, &instance, memory, sections)?;

    if let Ok(init) = instance.get_typed_func::<(), ()>(&mut store, "__init") {
        store.set_fuel(FUEL).expect("fuel is enabled");
        if !survived(init.call(&mut store, ()), "__init")? {
            return Ok(());
        }
    }

    let scans = scan_entries(&mut store, &instance, &module, sections)?;
    for tick in 0..SCANS {
        for (name, func, this, period) in &scans {
            if tick % period != 0 {
                continue;
            }
            store.set_fuel(FUEL).expect("fuel is enabled");
            let args: Vec<Val> = this.iter().map(|a| Val::I32(*a)).collect();
            if !survived(func.call(&mut store, &args, &mut []), name)? {
                return Ok(());
            }
        }
    }

    // What the monitor does after a scan: read every symbol back.
    let _ = sections.info.read_all_bytes(memory.data(&store));

    // The `{test}` functions and the `{export}` FUNCTIONs, with zero
    // arguments: the callers a module has besides its schedule.
    let scanned: Vec<&str> = scans.iter().map(|(n, ..)| n.as_str()).collect();
    let others: Vec<(String, Func)> = module
        .exports()
        .filter(|e| matches!(e.ty(), ExternType::Func(_)))
        .map(|e| e.name().to_string())
        .filter(|n| n != "__init" && !scanned.contains(&n.as_str()))
        .filter_map(|n| instance.get_func(&mut store, &n).map(|f| (n, f)))
        .collect();
    for (name, func) in others {
        let ty = func.ty(&store);
        let Some(args) = ty.params().map(|t| zero(&t)).collect::<Option<Vec<Val>>>() else {
            continue;
        };
        let Some(mut results) = ty.results().map(|t| zero(&t)).collect::<Option<Vec<Val>>>() else {
            continue;
        };
        store.set_fuel(FUEL).expect("fuel is enabled");
        if !survived(func.call(&mut store, &args, &mut results), &name)? {
            return Ok(());
        }
    }
    Ok(())
}

fn zero(ty: &ValType) -> Option<Val> {
    Some(match ty {
        ValType::I32 => Val::I32(0),
        ValType::I64 => Val::I64(0),
        ValType::F32 => Val::F32(0),
        ValType::F64 => Val::F64(0),
        _ => return None,
    })
}

/// A scan entry: its name, the function, its `this` argument when it is a
/// program instance, and the period it runs at.
type ScanEntry = (String, Func, Option<i32>, u64);

/// What a scan runs: the tasks of the `rk.schedule` section, whose every
/// binding must resolve, or without one each `() -> ()` export, as a
/// module without a CONFIGURATION is run.
fn scan_entries(
    store: &mut Store<()>,
    instance: &Instance,
    module: &Module,
    sections: &Sections,
) -> Result<Vec<ScanEntry>, Finding> {
    let mut entries = Vec::new();
    if let Some(schedule) = &sections.schedule {
        for task in &schedule.tasks {
            for program in &task.programs {
                let func = instance
                    .get_typed_func::<i32, ()>(&mut *store, &program.export)
                    .map_err(|_| {
                        Finding::new(
                            "custom-section",
                            format!(
                                "`rk.schedule` binds `{}` to `{}`, which is not an (i32) -> () export",
                                program.instance, program.export
                            ),
                        )
                    })?;
                entries.push((
                    program.export.clone(),
                    func.func().clone(),
                    Some(program.instance_addr as i32),
                    task.period_ticks,
                ));
            }
        }
        return Ok(entries);
    }
    for export in module.exports() {
        if export.name() == "__init" {
            continue;
        }
        if let Ok(f) = instance.get_typed_func::<(), ()>(&mut *store, export.name()) {
            entries.push((export.name().to_string(), f.func().clone(), None, 1));
        }
    }
    Ok(entries)
}

/// The retain and located maps name addresses in bands the module exports;
/// every one must lie inside its band, and every band inside the memory.
fn check_bands(
    store: &mut Store<()>,
    instance: &Instance,
    memory: Memory,
    sections: &Sections,
) -> Result<(), Finding> {
    let size = memory.data_size(&*store) as u64;
    let mut band = |base: &str, len: &str| -> Option<(u64, u64)> {
        let b = instance
            .get_global(&mut *store, base)?
            .get(&mut *store)
            .i32()? as u32 as u64;
        let l = instance
            .get_global(&mut *store, len)?
            .get(&mut *store)
            .i32()? as u32 as u64;
        Some((b, b + l))
    };
    let retain = band("retain_base", "retain_size");
    let input = band("input_base", "input_size");
    let output = band("output_base", "output_size");
    let marker = band("marker_base", "marker_size");
    let globals = band("globals_base", "globals_size");

    for (name, b) in [
        ("retain", retain),
        ("input", input),
        ("output", output),
        ("marker", marker),
        ("globals", globals),
    ] {
        if let Some((start, end)) = b
            && (start > end || end > size)
        {
            return Err(Finding::new(
                "memory-layout",
                format!("the {name} band [{start}, {end}) is outside the {size}-byte memory"),
            ));
        }
    }

    if let Some(map) = &sections.retain {
        let Some((start, end)) = retain else {
            return Err(Finding::new(
                "memory-layout",
                "the module has a retain map but exports no retain band",
            ));
        };
        for r in &map.ranges {
            let (a, z) = (r.addr as u64, r.addr as u64 + r.size as u64);
            if a < start || z > end {
                return Err(Finding::new(
                    "memory-layout",
                    format!(
                        "retained `{}` at [{a}, {z}) is outside the retain band [{start}, {end})",
                        r.path
                    ),
                ));
            }
        }
    }

    if let Some(map) = &sections.located {
        for v in &map.entries {
            let (area, b) = match v.area {
                LocatedArea::Input => ("input", input),
                LocatedArea::Output => ("output", output),
                LocatedArea::Marker => ("marker", marker),
            };
            let Some((start, end)) = b else {
                return Err(Finding::new(
                    "memory-layout",
                    format!(
                        "`{}` is located in the {area} area, which the module does not export",
                        v.address
                    ),
                ));
            };
            let (a, z) = (v.addr as u64, v.addr as u64 + v.size as u64);
            if a < start || z > end {
                return Err(Finding::new(
                    "memory-layout",
                    format!(
                        "`{}` ({}) at [{a}, {z}) is outside the {area} band [{start}, {end})",
                        v.address, v.name
                    ),
                ));
            }
        }
    }

    if let Some(schedule) = &sections.schedule {
        for task in &schedule.tasks {
            for p in &task.programs {
                if p.instance_addr as u64 >= size {
                    return Err(Finding::new(
                        "memory-layout",
                        format!(
                            "program instance `{}` is at {}, beyond the {size}-byte memory",
                            p.instance, p.instance_addr
                        ),
                    ));
                }
            }
        }
    }
    Ok(())
}

/// `Ok(true)` when the call returned, `Ok(false)` when it ended the way a
/// program may end (a trap it asked for, an IEC exception, no fuel left),
/// and a finding when it ended in a way no accepted program can.
fn survived(result: wasmtime::Result<()>, unit: &str) -> Result<bool, Finding> {
    let Err(err) = result else {
        return Ok(true);
    };
    let Some(trap) = err.downcast_ref::<Trap>() else {
        // An uncaught `$rk_exception`: a RAISE, an assertion, a subscript
        // out of range. The program's own fault.
        return Ok(false);
    };
    let broken = match trap {
        // IEC has no pointer arithmetic, and every subscript is checked
        // before the access, so no address an accepted program computes
        // leaves the memory.
        Trap::MemoryOutOfBounds => Some("memory-bounds"),
        // The code generator only emits `unreachable` behind a `throw`, and
        // the grafted builtins behind a Rust panic.
        Trap::UnreachableCodeReached => Some("unreachable"),
        // Recursion is refused (E13xx), and nothing else nests this deep.
        Trap::StackOverflow => Some("stack-overflow"),
        _ => None,
    };
    match broken {
        Some(oracle) => Err(Finding::new(oracle, format!("{unit}: {trap}"))),
        None => Ok(false),
    }
}
