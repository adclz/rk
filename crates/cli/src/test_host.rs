// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! Running a module's `{test}` functions in-process on a wasmtime host. A
//! `{test}` export is `() -> i32`, the address of a 12-byte canonical-ABI
//! `result<_, string>` the host decodes itself. Each test gets a fresh
//! instance, since a test mutates the same statics a program does.
//!
//! A test that fails is reported with the calls in progress where it did. A
//! trap comes back with wasmtime's backtrace. A `RAISE` does not, and one a
//! test's wrapper catches never reaches the host at all, so that test alone
//! runs once more in a debug store, whose handler is called at the throw
//! with the frames still there.

use std::cell::OnceCell;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use debug_format::DebugInfo;
use debug_format::test_manifest::{TEST_MANIFEST_SECTION, TestEntry, TestManifest};
use debug_format::test_report::{Frame, Status, TestRecord};
use wasmtime::{
    DebugEvent, DebugHandler, Engine, ExternType, Linker, Memory, MemoryType, Module, Store,
    StoreContextMut,
};

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
    on_result: impl FnMut(&TestRecord),
) -> Result<Vec<TestRecord>> {
    run_each_located(wasm, None, filter, budget, on_result)
}

/// [`run_each`] for an optimized `wasm`, which has lost its line tables:
/// `unoptimized` is the build it was made from, where a test that failed
/// runs once more to say where.
pub fn run_each_located(
    wasm: &[u8],
    unoptimized: Option<&[u8]>,
    filter: Option<&str>,
    budget: Option<Duration>,
    mut on_result: impl FnMut(&TestRecord),
) -> Result<Vec<TestRecord>> {
    let budget = budget.unwrap_or(DEFAULT_BUDGET);
    let engine = engine(false)?;
    let locator = Locator::new(wasm, unoptimized);
    // Compiled once: instantiation is per test, compilation is not.
    let module = Module::new(&engine, wasm).ctx("compiling the module")?;
    // A test's name is an identifier: `T_ADD` finds `t_add` (IEC 61131-3
    // §6.1.2), with the same Unicode fold the compiler matches names in.
    let filter = filter.map(str::to_lowercase);
    let tests: Vec<TestEntry> = discover(wasm)
        .into_iter()
        .filter(|t| {
            filter
                .as_deref()
                .is_none_or(|f| t.path.to_lowercase().contains(f))
        })
        .collect();

    let mut results = Vec::with_capacity(tests.len());
    for entry in tests {
        let start = Instant::now();
        let outcome = run_one(&engine, &module, &entry, budget)
            .with_context(|| format!("loading the module to run `{}`", entry.path))?;
        // Timed before the failure is located: that run is the report's, not
        // the test's.
        let duration_us = start.elapsed().as_micros() as u64;
        let backtrace = match outcome.reason {
            Some(_) => locator.locate(&entry, &outcome.trap, budget),
            None => Vec::new(),
        };
        let record = TestRecord {
            name: entry.path.clone(),
            status: if outcome.reason.is_none() {
                Status::Pass
            } else {
                Status::Fail
            },
            reason: outcome.reason,
            duration_us,
            file: (!entry.file.is_empty()).then(|| entry.file.clone()),
            line: (entry.line > 0).then_some(entry.line),
            backtrace,
        };
        on_result(&record);
        results.push(record);
    }
    Ok(results)
}

/// An engine with exception handling on (`RAISE`, the stdlib's
/// assertions) and epoch interruption, the watchdog's mechanism; a ticker
/// thread advances the epoch. `guest_debug` compiles the module with the
/// hooks a [`DebugHandler`] is called from, which costs every instruction:
/// it is for the one test being located, never for the run.
fn engine(guest_debug: bool) -> Result<Engine> {
    let mut config = wasmtime::Config::new();
    config.wasm_exceptions(true);
    config.epoch_interruption(true);
    config.guest_debug(guest_debug);
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

/// How one test went: `reason` is `None` on a pass, and `trap` the stack a
/// trap came back with, as `(function index, module offset)` pairs, innermost
/// first. A failure that is not a trap has none.
struct Outcome {
    reason: Option<String>,
    trap: Vec<(u32, u32)>,
}

impl Outcome {
    fn passed_or(reason: Option<String>) -> Self {
        Outcome {
            reason,
            trap: Vec::new(),
        }
    }

    fn trapped(reason: String, err: &wasmtime::Error) -> Self {
        let trap = err
            .downcast_ref::<wasmtime::WasmBacktrace>()
            .map(stack_of)
            .unwrap_or_default();
        Outcome {
            reason: Some(reason),
            trap,
        }
    }
}

/// A backtrace as `(function index, module offset)` pairs, innermost first.
/// A caller's offset is its call instruction's, so it is in the calling
/// statement as it stands.
fn stack_of(backtrace: &wasmtime::WasmBacktrace) -> Vec<(u32, u32)> {
    backtrace
        .frames()
        .iter()
        .filter_map(|frame| Some((frame.func_index(), frame.module_offset()? as u32)))
        .collect()
}

/// A linear memory for the module's `env.memory` import, and a linker with
/// it, the monotonic clock, and every other import stubbed as a trap: a
/// user can declare any `{extern}` host function, so imports cannot be
/// whitelisted, and an unwired one fails only if the test calls it.
fn link<T: 'static>(store: &mut Store<T>, module: &Module) -> Result<(Linker<T>, Memory)> {
    let min_pages = module
        .imports()
        .find_map(|i| match i.ty() {
            ExternType::Memory(mt) => Some(mt.minimum()),
            _ => None,
        })
        .context("the module must import a linear memory from `env`")?;
    let memory = Memory::new(&mut *store, MemoryType::new(min_pages as u32, None))
        .ctx("allocating linear memory")?;

    let mut linker = Linker::new(module.engine());
    linker
        .define(&*store, "env", "memory", memory)
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
    linker
        .define_unknown_imports_as_traps(module)
        .ctx("stubbing unwired host imports as traps")?;
    Ok((linker, memory))
}

/// One test on a fresh instance: a pass, a failure or a trap. `Err` is a
/// module this host could not load at all.
fn run_one(
    engine: &Engine,
    module: &Module,
    entry: &TestEntry,
    budget: Duration,
) -> Result<Outcome> {
    let mut store = Store::new(engine, ());
    // A store with epoch interruption enabled starts at a deadline that has
    // always elapsed, so arm it before ANY wasm runs — instantiation included.
    store.set_epoch_deadline(ticks(budget));
    let (linker, memory) = link(&mut store, module)?;
    let instance = linker
        .instantiate(&mut store, module)
        .ctx("instantiating the module")?;

    if let Ok(init) = instance.get_typed_func::<(), ()>(&mut store, "__init") {
        store.set_epoch_deadline(ticks(budget));
        if let Err(e) = init.call(&mut store, ()) {
            let reason = format!(
                "trapped in initialization: {}",
                explain(&mut store, memory, &e, budget)
            );
            return Ok(Outcome::trapped(reason, &e));
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
        Ok(addr) => Outcome::passed_or(decode_result(&store, memory, addr)),
        Err(e) => {
            let reason = format!("trapped: {}", explain(&mut store, memory, &e, budget));
            Outcome::trapped(reason, &e)
        }
    })
}

/// Where a failed test failed, for a report.
///
/// `info` reads the line tables of the module that can tell: the one that
/// ran, or the unoptimized build of an optimized one. A trap of that same
/// module is decoded as it came. Anything else runs the test once more in a
/// debug store, built on the first failure that needs one.
struct Locator<'a> {
    /// The module with the line tables.
    wasm: &'a [u8],
    /// Whether `wasm` is the module the tests ran on, so that a trap's
    /// offsets are offsets into it.
    ran: bool,
    info: OnceCell<DebugInfo>,
    debug: OnceCell<Option<(Engine, Module)>>,
}

impl<'a> Locator<'a> {
    fn new(ran: &'a [u8], unoptimized: Option<&'a [u8]>) -> Self {
        Locator {
            wasm: unoptimized.unwrap_or(ran),
            ran: unoptimized.is_none(),
            info: OnceCell::new(),
            debug: OnceCell::new(),
        }
    }

    fn locate(&self, entry: &TestEntry, trap: &[(u32, u32)], budget: Duration) -> Vec<Frame> {
        let info = self.info.get_or_init(|| DebugInfo::from_wasm(self.wasm));
        if !info.has_lines() {
            return Vec::new();
        }
        if self.ran && !trap.is_empty() {
            return frames(info, trap);
        }
        let debug = self.debug.get_or_init(|| {
            let engine = engine(true).ok()?;
            let module = Module::new(&engine, self.wasm).ok()?;
            Some((engine, module))
        });
        match debug {
            Some((_, module)) => frames(info, &trace(module, entry, budget)),
            None => Vec::new(),
        }
    }
}

/// The frames a report shows: each call the tables can name, with its
/// position when they have one. What they cannot name is the compiler's own
/// (a bundled check, a wrapper), and says nothing to a reader.
fn frames(info: &DebugInfo, stack: &[(u32, u32)]) -> Vec<Frame> {
    stack
        .iter()
        .filter_map(|&(func_index, pc)| {
            // A backtrace counts the module's imports first, the tables do
            // not.
            let defined_index = info.defined_index(func_index)?;
            let function = shown_name(info.function_name(defined_index)?);
            let at = info.source_position(defined_index, pc);
            Some(Frame {
                function,
                file: at
                    .as_ref()
                    .and_then(|at| info.source_files().get(at.file as usize).cloned()),
                line: at.as_ref().map(|at| at.line + 1),
                column: at.as_ref().map(|at| at.col + 1),
            })
        })
        .collect()
}

/// A function's name as its source spells it: `Motor#Start` is the method
/// `Motor.Start`, and `Motor$__body__` the body of `Motor`.
fn shown_name(name: &str) -> String {
    match name.split_once('$') {
        Some((pou, _)) => pou.to_string(),
        None => name.replace('#', "."),
    }
}

/// What the debug store keeps: the stack at the first throw or trap, as
/// [`stack_of`] gives it.
#[derive(Default)]
struct Trace {
    stack: Option<Vec<(u32, u32)>>,
}

/// Records the stack where a test fails. wasmtime calls it at the throw of
/// an exception, caught or not, at a trap and at a hostcall's error, with
/// the frames still there.
#[derive(Clone)]
struct AtFailure;

impl DebugHandler for AtFailure {
    type Data = Trace;

    async fn handle(&self, mut store: StoreContextMut<'_, Trace>, event: DebugEvent<'_>) {
        // A hostcall's error is a failure too: the watchdog's interrupt
        // comes from one, and so does a call to an import nothing wired.
        let failed = matches!(
            event,
            DebugEvent::CaughtExceptionThrown(_)
                | DebugEvent::UncaughtExceptionThrown(_)
                | DebugEvent::Trap(_)
                | DebugEvent::HostcallError(_)
        );
        // The first is the failure. Nothing a test runs catches and goes on.
        if !failed || store.data().stack.is_some() {
            return;
        }
        // The backtrace a trap would come back with, taken here. The debug
        // frames are not read: a caller's pc there is where it resumes, the
        // next statement's when its call ends one, and one byte back is the
        // statement before when its call starts one.
        let stack = stack_of(&wasmtime::WasmBacktrace::force_capture(&store));
        store.data_mut().stack = Some(stack);
    }
}

/// Run one test in a debug store and return the stack where it failed;
/// empty when it did not fail this time, or could not be loaded.
fn trace(module: &Module, entry: &TestEntry, budget: Duration) -> Vec<(u32, u32)> {
    let mut store = Store::new(module.engine(), Trace::default());
    store.set_debug_handler(AtFailure);
    store.set_epoch_deadline(ticks(budget));
    block_on(async {
        let Ok((linker, _memory)) = link(&mut store, module) else {
            return;
        };
        let Ok(instance) = linker.instantiate_async(&mut store, module).await else {
            return;
        };
        if let Ok(init) = instance.get_typed_func::<(), ()>(&mut store, "__init") {
            store.set_epoch_deadline(ticks(budget));
            if init.call_async(&mut store, ()).await.is_err() {
                return;
            }
        }
        if let Ok(test) = instance.get_typed_func::<(), i32>(&mut store, &entry.export) {
            store.set_epoch_deadline(ticks(budget));
            let _ = test.call_async(&mut store, ()).await;
        }
    });
    store.into_data().stack.unwrap_or_default()
}

/// Drive a future that never waits on anything outside itself: wasm on a
/// debug store runs on a fiber, and the handler above returns at once.
fn block_on<F: std::future::Future>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    loop {
        if let std::task::Poll::Ready(output) = future.as_mut().poll(&mut cx) {
            return output;
        }
        std::thread::yield_now();
    }
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
