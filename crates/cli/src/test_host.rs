//! Running a module's `{test}` functions in-process on a wasmtime host. A
//! `{test}` export is `() -> i32`, the address of a 12-byte canonical-ABI
//! `result<_, string>` the host decodes itself. Each test gets a fresh
//! instance, since a test mutates the same statics a program does.

use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use debug_format::test_manifest::{TEST_MANIFEST_SECTION, TestEntry, TestManifest};
use debug_format::test_report::{Status, TestRecord};
use wasmtime::{Engine, ExternType, Linker, Memory, MemoryType, Module, Store};

/// How long one test may run before the watchdog stops it, unless
/// `--timeout` says otherwise.
pub const DEFAULT_BUDGET: Duration = Duration::from_secs(5);

/// The watchdog's resolution: the engine's epoch advances this often.
const TICK: Duration = Duration::from_millis(10);

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

/// The `{test}` functions a compiled module carries, from its `test-manifest`
/// custom section. An empty list means the module has none.
pub fn discover(wasm: &[u8]) -> Vec<TestEntry> {
    for payload in wasmparser::Parser::new(0).parse_all(wasm) {
        if let Ok(wasmparser::Payload::CustomSection(c)) = payload
            && c.name() == TEST_MANIFEST_SECTION
            && let Ok(manifest) = TestManifest::from_msgpack(c.data())
        {
            return manifest.tests;
        }
    }
    Vec::new()
}

/// Run the module's tests, optionally filtered by a substring of their
/// path, calling `on_result` as each finishes. `budget` bounds each test
/// (`None` = [`DEFAULT_BUDGET`]).
pub fn run_each(
    wasm: &[u8],
    filter: Option<&str>,
    budget: Option<Duration>,
    mut on_result: impl FnMut(&TestRecord),
) -> Result<Vec<TestRecord>> {
    let budget = budget.unwrap_or(DEFAULT_BUDGET);
    let engine = engine()?;
    // Compiled once: instantiation is per test, compilation is not.
    let module = Module::new(&engine, wasm).ctx("compiling the module")?;
    let tests: Vec<TestEntry> = discover(wasm)
        .into_iter()
        .filter(|t| filter.is_none_or(|f| t.path.contains(f)))
        .collect();

    let mut results = Vec::with_capacity(tests.len());
    for entry in tests {
        let start = Instant::now();
        let outcome = run_one(&engine, &module, &entry, budget)
            .with_context(|| format!("loading the module to run `{}`", entry.path))?;
        let record = TestRecord {
            name: entry.path.clone(),
            status: if outcome.is_none() {
                Status::Pass
            } else {
                Status::Fail
            },
            reason: outcome,
            duration_us: start.elapsed().as_micros() as u64,
            file: (!entry.file.is_empty()).then(|| entry.file.clone()),
            line: (entry.line > 0).then_some(entry.line),
        };
        on_result(&record);
        results.push(record);
    }
    Ok(results)
}

/// An engine with exception handling on (`RAISE`, the stdlib's
/// assertions) and epoch interruption, the watchdog's mechanism; a ticker
/// thread advances the epoch.
fn engine() -> Result<Engine> {
    let mut config = wasmtime::Config::new();
    config.wasm_exceptions(true);
    config.epoch_interruption(true);
    let engine = Engine::new(&config).ctx("creating the wasm engine")?;
    let weak = engine.weak();
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(TICK);
            match weak.upgrade() {
                Some(engine) => engine.increment_epoch(),
                None => return,
            }
        }
    });
    Ok(engine)
}

fn ticks(budget: Duration) -> u64 {
    ((budget.as_nanos() / TICK.as_nanos()) as u64).max(1)
}

/// One test on a fresh instance. `Ok(None)` is a pass; `Ok(Some(reason))` a
/// failure or a trap; `Err` a module this host could not load at all.
fn run_one(
    engine: &Engine,
    module: &Module,
    entry: &TestEntry,
    budget: Duration,
) -> Result<Option<String>> {
    let mut store = Store::new(engine, ());
    // A store with epoch interruption enabled starts at a deadline that has
    // always elapsed, so arm it before ANY wasm runs — instantiation included.
    store.set_epoch_deadline(ticks(budget));

    let min_pages = module
        .imports()
        .find_map(|i| match i.ty() {
            ExternType::Memory(mt) => Some(mt.minimum()),
            _ => None,
        })
        .context("the module must import a linear memory from `env`")?;
    let memory = Memory::new(&mut store, MemoryType::new(min_pages as u32, None))
        .ctx("allocating linear memory")?;

    let mut linker = Linker::new(engine);
    linker
        .define(&store, "env", "memory", memory)
        .ctx("defining env.memory")?;
    let clock_start = Instant::now();
    for import in module.imports() {
        if import.name() == "now" && import.module().contains("monotonic-clock") {
            linker
                .func_wrap(import.module(), import.name(), move || {
                    clock_start.elapsed().as_nanos() as i64
                })
                .ctx("wiring the monotonic clock")?;
        }
    }
    // A user can declare any `{extern}` host function, so imports cannot be
    // whitelisted: an unwired one fails only if the test actually calls it.
    linker
        .define_unknown_imports_as_traps(module)
        .ctx("stubbing unwired host imports as traps")?;
    let instance = linker
        .instantiate(&mut store, module)
        .ctx("instantiating the module")?;

    if let Ok(init) = instance.get_typed_func::<(), ()>(&mut store, "__init") {
        store.set_epoch_deadline(ticks(budget));
        if let Err(e) = init.call(&mut store, ()) {
            return Ok(Some(format!(
                "trapped in initialization: {}",
                explain(&mut store, memory, &e, budget)
            )));
        }
    }

    let test = instance
        .get_typed_func::<(), i32>(&mut store, &entry.export)
        .ctx(format!(
            "test export `{}` must be a () -> i32 function",
            entry.export
        ))?;
    store.set_epoch_deadline(ticks(budget));
    Ok(match test.call(&mut store, ()) {
        Ok(addr) => decode_result(&store, memory, addr),
        Err(e) => {
            let reason = explain(&mut store, memory, &e, budget);
            Some(match place(entry) {
                Some(at) => format!("trapped at {at}: {reason}"),
                None => format!("trapped: {reason}"),
            })
        }
    })
}

/// Where a failing test was declared, as the module's manifest records it —
/// so a report names a source line rather than only a wasm trap.
fn place(entry: &TestEntry) -> Option<String> {
    if entry.file.is_empty() || entry.line == 0 {
        return None;
    }
    Some(format!("{}:{}", entry.file, entry.line))
}

/// A failed call in the words a reader needs: the watchdog when it fired, the
/// `$rk_exception` payload when one is pending (a `RAISE` or a failed runtime
/// check), else the trap's own words.
fn explain(
    store: &mut Store<()>,
    memory: Memory,
    err: &wasmtime::Error,
    budget: Duration,
) -> String {
    if matches!(
        err.downcast_ref::<wasmtime::Trap>(),
        Some(wasmtime::Trap::Interrupt)
    ) {
        return format!(
            "the watchdog stopped it after {}: it never returned",
            describe(budget)
        );
    }
    if let Some(exn) = store.take_pending_exception()
        && let (Ok(wasmtime::Val::I32(ptr)), Ok(wasmtime::Val::I32(len))) =
            (exn.field(&mut *store, 0), exn.field(&mut *store, 1))
    {
        let data = memory.data(&*store);
        let start = ptr as u32 as usize;
        let msg = data
            .get(start..start.saturating_add(len as u32 as usize))
            .map(|b| String::from_utf8_lossy(b).into_owned())
            .unwrap_or_else(|| "<exception payload out of bounds>".to_string());
        return format!("uncaught IEC exception: {msg}");
    }
    trap_words(err)
}

/// A trap in plain words: the trap itself when there is one, otherwise the
/// outermost message, never wasmtime's backtrace preamble.
pub fn trap_words(err: &wasmtime::Error) -> String {
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

fn describe(budget: Duration) -> String {
    if budget.subsec_millis() == 0 && budget.as_secs() > 0 {
        format!("{}s", budget.as_secs())
    } else {
        format!("{}ms", budget.as_millis())
    }
}

/// Decode the 12-byte canonical-ABI `result<_, string>` a test returns: a
/// pass is `None`, a failure carries the program's own message.
fn decode_result(store: &Store<()>, memory: Memory, addr: i32) -> Option<String> {
    let data = memory.data(store);
    let at = |ptr: u32, len: usize| data.get(ptr as usize..(ptr as usize).saturating_add(len));
    let Some(head) = at(addr as u32, 12) else {
        return Some("test result area is outside linear memory".to_string());
    };
    let disc = i32::from_le_bytes(head[0..4].try_into().unwrap());
    if disc == 0 {
        return None;
    }
    let ptr = u32::from_le_bytes(head[4..8].try_into().unwrap());
    let len = u32::from_le_bytes(head[8..12].try_into().unwrap()) as usize;
    // An empty message is the assertion macro's "no detail given" case, not a
    // decoding failure — say so rather than reporting a blank line.
    if len == 0 {
        return Some("assertion failed".to_string());
    }
    Some(match at(ptr, len) {
        Some(bytes) => String::from_utf8_lossy(bytes).into_owned(),
        None => "test failure message is outside linear memory".to_string(),
    })
}
