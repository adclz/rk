//! Helpers for the codegen tests: compile a source, run the module.
//!
//! [`TestPlc`] instantiates a compiled module the way a host does — it
//! provides `env.memory`, runs `__init`, reads the `rk.schedule` section and
//! drives the scan cycle — so a test asserts what a program DOES against the
//! same conventions the WASM ABI documents, with nothing but wasmtime.

use anyhow::{Context, Result, bail};
use auto_lsp::default::db::file::File;
use db::RootDatabase;
use debug_format::test_manifest::{TEST_MANIFEST_SECTION, TestEntry, TestManifest};
use debug_format::{DebugInfo, StackFrame, VarValue};
use hir::{check::diagnostics_for_file, hir_def::semantic_index::semantic_index};
use wasmtime::{ExternType, Instance, Linker, Memory, MemoryType, Module, Store, TypedFunc};

use crate::tests::utils::add_source;

/// `wasmtime::Error` is not a `std::error::Error`, so anyhow's `.context()`
/// does not apply to it; this folds the wasmtime chain into a message.
trait WasmtimeCtx<T> {
    fn ctx(self, msg: impl std::fmt::Display) -> Result<T>;
}

impl<T> WasmtimeCtx<T> for std::result::Result<T, wasmtime::Error> {
    fn ctx(self, msg: impl std::fmt::Display) -> Result<T> {
        self.map_err(|e| anyhow::anyhow!("{msg}: {e:#}"))
    }
}

/// Compile IEC source to WASM bytes, refusing a source the compiler rejects.
///
/// A codegen test asserts what a program DOES, which is only a question about
/// programs the compiler accepts. Lowering an already-rejected source measures
/// the behaviour of code no user can run, and pins it — which is how a test
/// came to assert that an over-long string literal truncates, that a variable
/// named like its own FUNCTION emits a module, and that `[1,2,3,4,5,6]` was
/// too many for a 2x3. To compile one on purpose, name the diagnostics with
/// [`compile_to_wasm_expecting`].
///
/// Note: WASM validation is not performed here - use wasmtime's Module::new()
/// or wasmparser::validate() on the returned bytes to validate.
pub fn compile_to_wasm(db: &mut RootDatabase, source: &str) -> Vec<u8> {
    compile_to_wasm_impl(db, source, Expectation::Clean)
}

/// Lower IEC source to MIR and WASM in a single pass, returning both. Use this
/// when a test needs to inspect the MIR layout (e.g. the retain band bounds or
/// a variable's storage) and run the emitted module against it. Lowering only
/// once avoids registering the same source twice (which would duplicate POUs).
pub fn compile_to_mir_and_wasm(db: &mut RootDatabase, source: &str) -> (mir::MirModule, Vec<u8>) {
    compile_to_mir_and_wasm_impl(db, source, Expectation::Clean)
}

/// [`compile_to_mir_and_wasm`] for a source the compiler rejects: `codes` must
/// match the diagnostics exactly, so the reason the source is invalid stays
/// pinned — a test that silently starts failing for a second reason is not
/// testing what it says.
pub fn compile_to_mir_and_wasm_expecting(
    db: &mut RootDatabase,
    source: &str,
    codes: &[&str],
) -> (mir::MirModule, Vec<u8>) {
    compile_to_mir_and_wasm_impl(db, source, Expectation::Exactly(codes))
}

fn compile_to_mir_and_wasm_impl(
    db: &mut RootDatabase,
    source: &str,
    expectation: Expectation,
) -> (mir::MirModule, Vec<u8>) {
    let file = add_source(db, source);
    let sem_idx = semantic_index(db, file);
    check_diagnostics(db, file, expectation);
    let mir_module =
        mir::lower::lower_module::lower_module(db, sem_idx).expect("MIR lowering failed");
    let wasm = wasm_codegen::generate_wasm(db, &export_everything(&mir_module)).finish();
    (mir_module, wasm)
}

/// The module with every function exported, imports included, which is what
/// the compiler did before `{export}`. The tests here call a FUNCTION, an FB
/// body or an import by name to look at what it computes, and a host may not:
/// a real module exports only what says so. An export is a root for an
/// optimizer and nothing else, so the code under test is the same.
///
/// The MIR a test gets back is the untouched one.
pub fn export_everything(module: &mir::MirModule) -> mir::MirModule {
    let mut module = module.clone();
    for func in &mut module.functions {
        func.linkage = mir::function::MirLinkage::Export;
    }
    for ext in &mut module.extern_functions {
        ext.linkage = mir::function::MirLinkage::Export;
    }
    module
}

#[derive(Clone, Copy)]
enum Expectation<'a> {
    /// The compiler must accept the source.
    Clean,
    /// The compiler must reject it with exactly these codes, in order.
    Exactly(&'a [&'a str]),
}

fn diagnostic_codes(db: &RootDatabase, file: File) -> Vec<String> {
    diagnostics_for_file(db, file)
        .iter()
        .map(|diag| match &diag.diagnostic.code {
            Some(auto_lsp::lsp_types::NumberOrString::String(code)) => code.clone(),
            Some(auto_lsp::lsp_types::NumberOrString::Number(code)) => code.to_string(),
            None => "?".to_string(),
        })
        .collect()
}

fn check_diagnostics(db: &RootDatabase, file: File, expectation: Expectation) {
    let diagnostics = diagnostics_for_file(db, file);
    let found = diagnostic_codes(db, file);
    if let Expectation::Exactly(expected) = expectation
        && found == expected
    {
        return;
    }
    if matches!(expectation, Expectation::Clean) && diagnostics.is_empty() {
        return;
    }

    let mut message = match expectation {
        Expectation::Clean => format!(
            "Source has {} diagnostic(s), cannot compile:\n",
            diagnostics.len()
        ),
        Expectation::Exactly(expected) => {
            format!("Source was expected to report {expected:?}, but reports:\n")
        }
    };
    for (diag, code) in diagnostics.iter().zip(&found).take(10) {
        message.push_str(&format!("  [{code}] {}\n", diag.diagnostic.message));
    }
    if diagnostics.len() > 10 {
        message.push_str(&format!(
            "  ... and {} more diagnostics\n",
            diagnostics.len() - 10
        ));
    }
    panic!("{}", message);
}

fn compile_to_wasm_impl(db: &mut RootDatabase, source: &str, expectation: Expectation) -> Vec<u8> {
    let file = add_source(db, source);
    let sem_idx = semantic_index(db, file);

    check_diagnostics(db, file, expectation);

    // MIR pipeline: HIR → MIR → WASM
    let mir_module =
        mir::lower::lower_module::lower_module(db, sem_idx).expect("MIR lowering failed");

    let wasm_module = wasm_codegen::generate_wasm(db, &export_everything(&mir_module));
    wasm_module.finish()
}

/// Helper to validate WASM bytes using wasmtime.
///
/// Wasmtime's Module::new() performs full validation, so we don't need
/// wasmparser for validation anymore. This returns an error if the WASM
/// is invalid, or Ok(()) if it's valid.
pub fn validate_wasm(wasm_bytes: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    let engine = test_engine();
    let _ = wasmtime::Module::new(&engine, wasm_bytes)?;
    Ok(())
}

/// Helper to execute a WASM function and return its result.
///
/// This is a generic helper that works with any function signature that
/// implements wasmtime's WasmParams and WasmResults traits.
///
/// # Examples
///
/// ```ignore
/// // No parameters, returns i32
/// let result: i32 = execute_wasm(&wasm_bytes, "my_func", ());
///
/// // Two i32 parameters, returns i32
/// let result: i32 = execute_wasm(&wasm_bytes, "add", (5, 3));
///
/// // One i32 parameter, returns f32
/// let result: f32 = execute_wasm(&wasm_bytes, "to_float", 42);
/// ```
/// Instantiate a core module produced by `generate_wasm`, providing the
/// `env.memory` import that the module now requires (see `WasmGen::new`).
///
/// Use in tests that build their own engine/store rather than going through
/// `execute_wasm` / `execute_wasm_with_imports`.
/// An engine configured the way the real runtime's is: exception handling on.
/// Any module whose lowering pulled in `rk.idx_check` (every runtime array
/// subscript), `RAISE`, or an assertion carries the `$rk_exception` tag, and
/// wasmtime's DEFAULT config refuses to even parse it.
pub fn test_engine() -> wasmtime::Engine {
    let mut config = wasmtime::Config::new();
    config.wasm_exceptions(true);
    wasmtime::Engine::new(&config).expect("engine with exceptions")
}

pub fn instantiate_with_memory(
    store: &mut wasmtime::Store<()>,
    module: &wasmtime::Module,
) -> wasmtime::Instance {
    instantiate_returning_memory(store, module).0
}

/// As [`instantiate_with_memory`], keeping the memory handle — a fault test
/// needs it to read the `$rk_exception` payload out of linear memory.
pub fn instantiate_returning_memory(
    store: &mut wasmtime::Store<()>,
    module: &wasmtime::Module,
) -> (wasmtime::Instance, wasmtime::Memory) {
    let memory =
        wasmtime::Memory::new(&mut *store, wasmtime::MemoryType::new(1, None)).expect("memory");
    let instance = wasmtime::Instance::new(store, module, &[memory.into()])
        .expect("Failed to instantiate with memory");
    (instance, memory)
}

/// What a failed call says to whoever reads the fault: the `$rk_exception`
/// payload when one is pending (the same decode the runtime does, so a test
/// asserts the message a user would see), else the trap's own words. Asserting
/// only `is_err()` lets a named fault silently degrade into "thrown Wasm
/// exception".
pub fn fault_message(
    store: &mut wasmtime::Store<()>,
    memory: wasmtime::Memory,
    err: wasmtime::Error,
) -> String {
    let Some(exn) = store.take_pending_exception() else {
        return format!("{err:?}");
    };
    let (Ok(wasmtime::Val::I32(ptr)), Ok(wasmtime::Val::I32(len))) =
        (exn.field(&mut *store, 0), exn.field(&mut *store, 1))
    else {
        return format!("{err:?}");
    };
    let data = memory.data(&*store);
    data.get(ptr as u32 as usize..(ptr as u32 as usize).saturating_add(len as u32 as usize))
        .map(|b| String::from_utf8_lossy(b).into_owned())
        .unwrap_or_else(|| "<exception payload out of bounds>".to_string())
}

pub fn execute_wasm<P, R>(wasm_bytes: &[u8], func_name: &str, params: P) -> R
where
    P: wasmtime::WasmParams,
    R: wasmtime::WasmResults,
{
    execute_wasm_with_imports(wasm_bytes, func_name, params, |_| {})
}

/// Helper to execute a WASM function that requires imports (extern pragmas).
///
/// Uses a wasmtime Linker to provide the import functions before instantiation.
/// The `define_imports` closure receives a `&mut Linker<()>` to register host functions.
pub fn execute_wasm_with_imports<P, R, F>(
    wasm_bytes: &[u8],
    func_name: &str,
    params: P,
    define_imports: F,
) -> R
where
    P: wasmtime::WasmParams,
    R: wasmtime::WasmResults,
    F: FnOnce(&mut wasmtime::Linker<()>),
{
    let engine = test_engine();
    let module = wasmtime::Module::new(&engine, wasm_bytes).expect("Failed to create module");
    let mut store = wasmtime::Store::new(&engine, ());
    let mut linker = wasmtime::Linker::new(&engine);

    // The core module imports its memory from `env`. Provide a host-owned memory
    // that the module can load/store into during the test.
    let memory =
        wasmtime::Memory::new(&mut store, wasmtime::MemoryType::new(1, None)).expect("memory");
    linker
        .define(&store, "env", "memory", memory)
        .expect("define env.memory");

    define_imports(&mut linker);

    let instance = linker
        .instantiate(&mut store, &module)
        .expect("Failed to instantiate with imports");

    let func = instance
        .get_typed_func::<P, R>(&mut store, func_name)
        .unwrap_or_else(|_| panic!("Failed to get function '{}'", func_name));

    func.call(&mut store, params)
        .unwrap_or_else(|e| panic!("Failed to call function '{}': {}", func_name, e))
}

/// A band of linear memory located from the module's exported globals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Region {
    pub base: u32,
    pub size: u32,
}

/// How a loaded module runs one scan.
enum Driver {
    /// No scan entry: the module is still worth loading for its `{test}`
    /// exports or its memory, and only [`TestPlc::scan`] refuses.
    None(String),
    /// No CONFIGURATION: the single program export is the whole scan.
    Single(TypedFunc<(), ()>),
    /// A CONFIGURATION's task schedule, from the `rk.schedule` section.
    Scheduled(Vec<ScheduledTask>),
}

#[derive(Clone)]
struct ScheduledTask {
    name: String,
    period_ticks: u64,
    /// Program instances in declaration order: (instance name, body, `this`).
    programs: Vec<(String, TypedFunc<i32, ()>, i32)>,
}

/// A compiled module instantiated the way a host does it, driven scan by scan.
///
/// The host owns the linear memory (`env.memory`), calls `__init` once, and
/// finds the retained and global bands through the exported `retain_base`,
/// `retain_size`, `globals_base` and `globals_size`. A module with an
/// `rk.schedule` section runs every task whose period divides the tick, in
/// manifest order; a module without one runs its sole function export.
pub struct TestPlc {
    store: Store<()>,
    memory: Memory,
    instance: Instance,
    driver: Driver,
    retain: Region,
    globals: Region,
    tick: u64,
}

impl TestPlc {
    /// Instantiate `wasm`, run `__init`, and locate the memory bands.
    pub fn load(wasm: &[u8]) -> Result<Self> {
        let engine = test_engine();
        let module = Module::new(&engine, wasm).ctx("compiling the module")?;
        let mut store = Store::new(&engine, ());

        let min_pages = module
            .imports()
            .find_map(|i| match i.ty() {
                ExternType::Memory(mt) => Some(mt.minimum()),
                _ => None,
            })
            .context("the module must import a linear memory from `env`")?;
        let memory = Memory::new(&mut store, MemoryType::new(min_pages as u32, None))
            .ctx("allocating linear memory")?;

        let mut linker = Linker::new(&engine);
        linker
            .define(&store, "env", "memory", memory)
            .ctx("defining env.memory")?;
        let clock_start = std::time::Instant::now();
        for import in module.imports() {
            if import.name() == "now" && import.module().contains("monotonic-clock") {
                linker
                    .func_wrap(import.module(), import.name(), move || {
                        clock_start.elapsed().as_nanos() as i64
                    })
                    .ctx("wiring the monotonic clock")?;
            }
        }
        // An `{extern}` a test never calls must not stop the module loading.
        linker
            .define_unknown_imports_as_traps(&module)
            .ctx("stubbing unwired host imports as traps")?;
        let instance = linker
            .instantiate(&mut store, &module)
            .ctx("instantiating the module")?;

        if let Ok(init) = instance.get_typed_func::<(), ()>(&mut store, "__init") {
            init.call(&mut store, ())
                .map_err(|e| anyhow::anyhow!("running __init: {}", trap_words(&e)))?;
        }

        let retain = Region {
            base: global_i32(&mut store, &instance, "retain_base")? as u32,
            size: global_i32(&mut store, &instance, "retain_size")? as u32,
        };
        let globals = Region {
            base: global_i32_opt(&mut store, &instance, "globals_base").unwrap_or(0) as u32,
            size: global_i32_opt(&mut store, &instance, "globals_size").unwrap_or(0) as u32,
        };

        let driver = match schedule_manifest(wasm)? {
            Some(manifest) => Driver::Scheduled(resolve_tasks(&mut store, &instance, &manifest)?),
            None => match sole_function_export(&module) {
                Ok(entry) => match instance.get_typed_func::<(), ()>(&mut store, &entry) {
                    Ok(f) => Driver::Single(f),
                    Err(e) => Driver::None(format!(
                        "scan entry `{entry}` must be a () -> () export: {e}"
                    )),
                },
                Err(why) => Driver::None(format!("{why:#}")),
            },
        };

        Ok(Self {
            store,
            memory,
            instance,
            driver,
            retain,
            globals,
            tick: 0,
        })
    }

    /// Scans completed so far.
    #[allow(dead_code)]
    pub fn tick(&self) -> u64 {
        self.tick
    }

    /// One scan cycle: every task due at this tick in manifest order, or the
    /// single program. The tick advances however the cycle ended, so a fault
    /// does not replay the tasks that already ran.
    pub fn scan(&mut self) -> Result<()> {
        enum Due {
            Single(TypedFunc<(), ()>),
            Tasks(Vec<ScheduledTask>),
        }
        let tick = self.tick;
        let due = match &self.driver {
            Driver::None(why) => bail!("this module has no scan entry: {why}"),
            Driver::Single(f) => Due::Single(f.clone()),
            Driver::Scheduled(tasks) => Due::Tasks(
                tasks
                    .iter()
                    .filter(|t| tick.is_multiple_of(t.period_ticks))
                    .cloned()
                    .collect(),
            ),
        };
        let mut result = Ok(());
        match due {
            Due::Single(f) => {
                if let Err(e) = f.call(&mut self.store, ()) {
                    result = Err(self.fault(e, "the scan cycle"));
                }
            }
            Due::Tasks(tasks) => {
                'cycle: for task in tasks {
                    for (instance, body, this) in task.programs {
                        if let Err(e) = body.call(&mut self.store, this) {
                            let unit =
                                format!("task '{}': program instance '{instance}'", task.name);
                            result = Err(self.fault(e, &unit));
                            break 'cycle;
                        }
                    }
                }
            }
        }
        self.tick = tick + 1;
        result
    }

    /// `ticks` scan cycles in sequence.
    pub fn run(&mut self, ticks: usize) -> Result<()> {
        for _ in 0..ticks {
            self.scan()?;
        }
        Ok(())
    }

    /// Call a `{test}` export, returning the address of its 12-byte result.
    pub fn call_test(&mut self, export: &str) -> Result<i32> {
        let f = self
            .instance
            .get_typed_func::<(), i32>(&mut self.store, export)
            .ctx(format!(
                "test export `{export}` must be a () -> i32 function"
            ))?;
        f.call(&mut self.store, ())
            .map_err(|e| self.fault(e, &format!("test `{export}`")))
    }

    /// What a failed call says: the `$rk_exception` payload when one is
    /// pending, else the trap's own words, under the unit that faulted.
    fn fault(&mut self, err: wasmtime::Error, unit: &str) -> anyhow::Error {
        let words = match self.store.take_pending_exception() {
            Some(exn) => match (exn.field(&mut self.store, 0), exn.field(&mut self.store, 1)) {
                (Ok(wasmtime::Val::I32(ptr)), Ok(wasmtime::Val::I32(len))) => {
                    let data = self.memory.data(&self.store);
                    let start = ptr as u32 as usize;
                    let msg = data
                        .get(start..start.saturating_add(len as u32 as usize))
                        .map(|b| String::from_utf8_lossy(b).into_owned())
                        .unwrap_or_else(|| "<exception payload out of bounds>".to_string());
                    format!("uncaught IEC exception: {msg}")
                }
                _ => trap_words(&err),
            },
            None => trap_words(&err),
        };
        anyhow::anyhow!("{unit}: {words}")
    }

    pub fn retain_region(&self) -> Region {
        self.retain
    }

    pub fn globals_region(&self) -> Region {
        self.globals
    }

    /// Copy the retained band out of linear memory.
    pub fn read_retain(&self) -> Vec<u8> {
        self.read_region(self.retain)
    }

    /// Copy the globals band out of linear memory.
    pub fn read_globals(&self) -> Vec<u8> {
        self.read_region(self.globals)
    }

    fn read_region(&self, region: Region) -> Vec<u8> {
        self.read_bytes(region.base, region.size as usize)
            .expect("a band must lie within linear memory")
    }

    /// Overwrite bytes of the globals band, `offset` from its base.
    pub fn write_globals(&mut self, offset: usize, bytes: &[u8]) -> Result<()> {
        if offset + bytes.len() > self.globals.size as usize {
            bail!(
                "write of {} bytes at offset {offset} leaves the globals band ({} bytes)",
                bytes.len(),
                self.globals.size
            );
        }
        self.write_bytes(self.globals.base + offset as u32, bytes)
    }

    pub fn read_bytes(&self, addr: u32, len: usize) -> Result<Vec<u8>> {
        let mut buf = vec![0u8; len];
        self.memory
            .read(&self.store, addr as usize, &mut buf)
            .with_context(|| format!("reading {len} bytes at {addr}"))?;
        Ok(buf)
    }

    pub fn write_bytes(&mut self, addr: u32, bytes: &[u8]) -> Result<()> {
        self.memory
            .write(&mut self.store, addr as usize, bytes)
            .with_context(|| format!("writing {} bytes at {addr}", bytes.len()))
    }

    #[allow(dead_code)]
    pub fn memory_data(&self) -> &[u8] {
        self.memory.data(&self.store)
    }

    /// A variable's current value by qualified path, through the module's
    /// debug symbols; `None` if no such symbol.
    pub fn read_var(&self, info: &DebugInfo, path: &str) -> Option<VarValue> {
        let loc = info.resolve(path)?;
        let bytes = self.read_bytes(loc.address, loc.size as usize).ok()?;
        Some(debug_format::decode(loc.ty, &bytes))
    }

    /// The current value of every symbol, in symbol order.
    pub fn read_all<'a>(&self, info: &'a DebugInfo) -> Vec<(&'a str, VarValue)> {
        info.read_all_with(|address, size| self.read_bytes(address, size as usize).ok())
    }

    /// Write a value by qualified path: the debugger's "force".
    pub fn write_var(&mut self, info: &DebugInfo, path: &str, value: VarValue) -> Result<()> {
        let loc = info
            .resolve(path)
            .with_context(|| format!("unknown variable `{path}`"))?;
        let bytes = debug_format::encode(loc.ty, value)?;
        self.write_bytes(loc.address, &bytes)
    }
}

/// Resolve a trap's backtrace into source-level frames, innermost first.
/// `n_func_imports` turns a module-level function index into the defined
/// index the debug tables use; frames in imported functions are skipped.
pub fn resolve_backtrace(
    info: &DebugInfo,
    backtrace: &wasmtime::WasmBacktrace,
    n_func_imports: u32,
) -> Vec<StackFrame> {
    backtrace
        .frames()
        .iter()
        .filter_map(|f| {
            let defined = f.func_index().checked_sub(n_func_imports)?;
            Some(info.resolve_frame(defined, f.module_offset().map(|o| o as u32)))
        })
        .collect()
}

fn global_i32(store: &mut Store<()>, instance: &Instance, name: &str) -> Result<i32> {
    instance
        .get_global(&mut *store, name)
        .with_context(|| format!("the module must export global `{name}`"))?
        .get(&mut *store)
        .i32()
        .with_context(|| format!("global `{name}` must be i32"))
}

fn global_i32_opt(store: &mut Store<()>, instance: &Instance, name: &str) -> Option<i32> {
    instance
        .get_global(&mut *store, name)?
        .get(&mut *store)
        .i32()
}

fn sole_function_export(module: &Module) -> Result<String> {
    let mut funcs = module
        .exports()
        .filter(|e| matches!(e.ty(), ExternType::Func(_)))
        .map(|e| e.name().to_string());
    let first = funcs
        .next()
        .context("the module has no function export to use as the scan entry")?;
    if let Some(second) = funcs.next() {
        bail!("the module has several function exports (`{first}`, `{second}`) and no schedule");
    }
    Ok(first)
}

/// The `rk.schedule` section, decoded, when the module carries one.
fn schedule_manifest(wasm: &[u8]) -> Result<Option<debug_format::ScheduleManifest>> {
    let Some(data) = custom_section(wasm, debug_format::SCHEDULE_SECTION) else {
        return Ok(None);
    };
    let manifest = debug_format::ScheduleManifest::from_msgpack(data)
        .context("decoding the `rk.schedule` section")?;
    if manifest.version != debug_format::SCHEDULE_VERSION {
        bail!(
            "`rk.schedule` is version {}, this harness reads {}",
            manifest.version,
            debug_format::SCHEDULE_VERSION
        );
    }
    Ok(Some(manifest))
}

fn resolve_tasks(
    store: &mut Store<()>,
    instance: &Instance,
    manifest: &debug_format::ScheduleManifest,
) -> Result<Vec<ScheduledTask>> {
    let mut tasks = Vec::new();
    for t in &manifest.tasks {
        if t.period_ticks == 0 {
            bail!("task `{}` has a period of 0 ticks", t.name);
        }
        let mut programs = Vec::new();
        for prog in &t.programs {
            let body = instance
                .get_typed_func::<i32, ()>(&mut *store, &prog.export)
                .ctx(format!(
                    "the schedule binds instance `{}` to export `{}`, which is not an (i32) -> () export",
                    prog.instance, prog.export
                ))?;
            programs.push((prog.instance.clone(), body, prog.instance_addr as i32));
        }
        tasks.push(ScheduledTask {
            name: t.name.clone(),
            period_ticks: t.period_ticks,
            programs,
        });
    }
    Ok(tasks)
}

/// The bytes of the named custom section, if the module has one.
pub fn custom_section<'a>(wasm: &'a [u8], name: &str) -> Option<&'a [u8]> {
    wasmparser::Parser::new(0)
        .parse_all(wasm)
        .find_map(|payload| match payload {
            Ok(wasmparser::Payload::CustomSection(c)) if c.name() == name => Some(c.data()),
            _ => None,
        })
}

/// A trap in plain words: the trap itself when there is one, otherwise the
/// outermost message, never wasmtime's backtrace preamble.
fn trap_words(err: &wasmtime::Error) -> String {
    if let Some(trap) = err.downcast_ref::<wasmtime::Trap>() {
        let words = trap.to_string();
        return words
            .strip_prefix("wasm trap: ")
            .map(str::to_string)
            .unwrap_or(words);
    }
    let first = err.to_string();
    if !first.starts_with("error while executing") {
        return first;
    }
    err.root_cause().to_string()
}

/// What happened to one `{test}` function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Pass,
    /// A failed `ASSERT` or an explicit `RAISE`: the message the program gave.
    Fail(String),
    /// The test trapped, with the reason.
    Trap(String),
}

/// One `{test}` function and how it went.
#[derive(Debug, Clone)]
pub struct TestResult {
    pub entry: TestEntry,
    pub outcome: Outcome,
}

impl TestResult {
    pub fn passed(&self) -> bool {
        self.outcome == Outcome::Pass
    }
}

/// The `{test}` functions a module carries, from its `test-manifest` section.
pub fn discover_tests(wasm: &[u8]) -> Vec<TestEntry> {
    custom_section(wasm, TEST_MANIFEST_SECTION)
        .and_then(|data| TestManifest::from_msgpack(data).ok())
        .map(|m| m.tests)
        .unwrap_or_default()
}

/// Run the module's `{test}` functions, each on a fresh instance so no test
/// inherits another's state. `filter` selects by a substring of the path.
pub fn run_tests(wasm: &[u8], filter: Option<&str>) -> Result<Vec<TestResult>> {
    let mut results = Vec::new();
    for entry in discover_tests(wasm) {
        if filter.is_some_and(|f| !entry.path.contains(f)) {
            continue;
        }
        let mut plc = TestPlc::load(wasm)
            .with_context(|| format!("loading the module to run `{}`", entry.path))?;
        let outcome = match plc.call_test(&entry.export) {
            Ok(addr) => decode_test_result(&plc, addr),
            Err(e) => Outcome::Trap(format!("{e:#}")),
        };
        results.push(TestResult { entry, outcome });
    }
    Ok(results)
}

/// Decode the 12-byte `result<_, string>` a test returns: discriminant, then
/// for the error case a pointer and length into linear memory.
fn decode_test_result(plc: &TestPlc, addr: i32) -> Outcome {
    let Ok(head) = plc.read_bytes(addr as u32, 12) else {
        return Outcome::Fail("test result area is outside linear memory".to_string());
    };
    let disc = i32::from_le_bytes(head[0..4].try_into().unwrap());
    if disc == 0 {
        return Outcome::Pass;
    }
    let ptr = u32::from_le_bytes(head[4..8].try_into().unwrap());
    let len = u32::from_le_bytes(head[8..12].try_into().unwrap()) as usize;
    if len == 0 {
        return Outcome::Fail("assertion failed".to_string());
    }
    match plc.read_bytes(ptr, len) {
        Ok(bytes) => Outcome::Fail(String::from_utf8_lossy(&bytes).into_owned()),
        Err(_) => Outcome::Fail("test failure message is outside linear memory".to_string()),
    }
}
