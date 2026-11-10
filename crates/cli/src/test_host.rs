// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! Running a module's `{test}` functions in-process on a wasmtime host.
//! `rk test` runs the component it builds, as a host runs it: through its
//! control interface, `init` and then `run-test` with the test's place in
//! the manifest, each a `result<_, string>`, with WASI's clocks. The codegen
//! tests run a core module alone, whose `{test}` export is `() -> i32`, the
//! address of a 12-byte canonical-ABI `result<_, string>` the host decodes
//! itself. Each test gets a fresh instance, since a test mutates the same
//! statics a program does.
//!
//! A test that fails is reported with the calls in progress where it did,
//! from the core module: the one given, or the one the component wraps. A
//! trap comes back with wasmtime's backtrace, whose offsets count from the
//! start of the file that ran: a component's are brought back to its
//! module's. A `RAISE` brings none, and one a test's wrapper catches never
//! reaches the host at all, so that test alone runs once more on the core
//! module in a debug store, whose handler is called at the throw with the
//! frames still there.
//!
//! What a test writes to the standard output and error is kept, and
//! reported with the test when it fails.

use std::cell::OnceCell;
use std::ops::Range;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use debug_format::DebugInfo;
use debug_format::test_manifest::{TEST_MANIFEST_SECTION, TestEntry, TestManifest};
use debug_format::test_report::{Frame, Status, TestRecord};
use wasmtime::component::{Component, InstancePre};
use wasmtime::{
    Caller, DebugEvent, DebugHandler, Engine, Extern, Instance, Linker, Memory, Module, Store,
    StoreContextMut,
};
use wasmtime_wasi::p3::bindings::clocks::system_clock::Instant as WasiInstant;
use wasmtime_wasi::{ResourceTable, WasiCtx, WasiCtxBuilder, WasiCtxView, WasiView};

/// How long one test may run before the watchdog stops it, unless
/// `--timeout` says otherwise.
pub const DEFAULT_BUDGET: Duration = Duration::from_secs(5);

/// The most a test's output keeps, in bytes. What it writes past that is
/// counted, so that a test writing in a loop until the watchdog stops it
/// does not fill the host's memory.
pub const OUTPUT_LIMIT: usize = 64 * 1024;

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

/// What a run's tests run on.
#[derive(Clone, Copy)]
pub enum Target<'a> {
    /// The component `rk test` builds, run as a host runs it: through its
    /// control interface, with WASI's clocks.
    Component(&'a [u8]),
    /// A core module alone, its `{test}` exports called directly: what the
    /// codegen tests run.
    Core(&'a [u8]),
}

/// Run the target's tests, optionally filtered by a substring of their
/// path, calling `on_result` as each finishes. `budget` bounds each test
/// (`None` = [`DEFAULT_BUDGET`]).
pub fn run_each(
    target: Target<'_>,
    filter: Option<&str>,
    budget: Option<Duration>,
    on_result: impl FnMut(&TestRecord),
) -> Result<Vec<TestRecord>> {
    run_each_located(target, None, filter, budget, on_result)
}

/// [`run_each`] for an optimized build, which has lost its line tables:
/// `unoptimized` is the core module it was made from, where a test that
/// failed runs once more to say where.
pub fn run_each_located(
    target: Target<'_>,
    unoptimized: Option<&[u8]>,
    filter: Option<&str>,
    budget: Option<Duration>,
    mut on_result: impl FnMut(&TestRecord),
) -> Result<Vec<TestRecord>> {
    let budget = budget.unwrap_or(DEFAULT_BUDGET);
    let engine = engine(false)?;
    // Compiled once: instantiation is per test, compilation is not.
    let runner = Runner::new(&engine, target)?;
    let core = runner.core();
    let locator = Locator::new(core, unoptimized);
    // A test's name is an identifier: `T_ADD` finds `t_add` (IEC 61131-3
    // §6.1.2), with the same Unicode fold the compiler matches names in. A
    // test keeps its place in the manifest, which `run-test` takes.
    let filter = filter.map(str::to_lowercase);
    let tests: Vec<(u32, TestEntry)> = discover(core)
        .into_iter()
        .enumerate()
        .filter(|(_, t)| {
            filter
                .as_deref()
                .is_none_or(|f| t.path.to_lowercase().contains(f))
        })
        .map(|(place, entry)| (place as u32, entry))
        .collect();

    let mut results = Vec::with_capacity(tests.len());
    for (place, entry) in tests {
        let start = Instant::now();
        let outcome = runner
            .run(&entry, place, budget)
            .with_context(|| format!("loading the module to run `{}`", entry.path))?;
        // Timed before the failure is located: that run is the report's, not
        // the test's.
        let duration_us = start.elapsed().as_micros() as u64;
        let failed = outcome.reason.is_some();
        let backtrace = match failed {
            true => locator.locate(&entry, &outcome.trap, budget),
            false => Vec::new(),
        };
        let record = TestRecord {
            name: entry.path.clone(),
            status: if failed { Status::Fail } else { Status::Pass },
            reason: outcome.reason,
            duration_us,
            file: (!entry.file.is_empty()).then(|| entry.file.clone()),
            line: (entry.line > 0).then_some(entry.line),
            backtrace,
            // A pass keeps quiet, whatever it wrote.
            output: (failed && !outcome.output.is_empty()).then_some(outcome.output),
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
    // wasmtime-wasi's WASI 0.3 clocks: `wait-for` and `wait-until` are
    // async, though a component that only reads `now` runs without them.
    config.wasm_component_model_async(true);
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
/// first. A failure that is not a trap has none. `output` is what the test
/// wrote, [`Stdio::text`].
struct Outcome {
    reason: Option<String>,
    trap: Vec<(u32, u32)>,
    output: String,
}

impl Outcome {
    fn passed_or(reason: Option<String>) -> Self {
        Outcome {
            reason,
            trap: Vec::new(),
            output: String::new(),
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
            output: String::new(),
        }
    }

    /// [`Outcome::trapped`] for a trap of a component, whose core module
    /// is at `module` in the component's bytes. A frame's offset counts
    /// from the start of the component: the frames of the module are kept,
    /// at their offsets in it. Another module's, a shim of the wrapper's,
    /// has nothing the tables name.
    fn trapped_in(reason: String, err: &wasmtime::Error, module: &Range<usize>) -> Self {
        let mut outcome = Self::trapped(reason, err);
        outcome.trap = frames_of_module(outcome.trap, module);
        outcome
    }
}

/// The frames of `stack` that are in the module at `module` in a
/// component, their offsets counted from the module's start instead of the
/// component's.
fn frames_of_module(stack: Vec<(u32, u32)>, module: &Range<usize>) -> Vec<(u32, u32)> {
    stack
        .into_iter()
        .filter(|(_, offset)| module.contains(&(*offset as usize)))
        .map(|(function, offset)| (function, offset - module.start as u32))
        .collect()
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

/// A linker with the clocks, the standard output and error, which write to
/// `stdio`, and every other import stubbed as a trap: a user can declare
/// any `{extern}` host function, so imports cannot be whitelisted, and an
/// unwired one fails only if the test calls it. The memory is the module's
/// own ([`memory_of`]).
fn link<T: 'static>(module: &Module, stdio: &Arc<Mutex<Stdio>>) -> Result<Linker<T>> {
    let mut linker = Linker::new(module.engine());
    wire_clocks(&mut linker, module)?;
    wire_console(&mut linker, module, stdio)?;
    linker
        .define_unknown_imports_as_traps(module)
        .ctx("stubbing unwired host imports as traps")?;
    Ok(linker)
}

/// The memory a module defines and exports as `memory`.
pub fn memory_of<T>(store: &mut Store<T>, instance: &Instance) -> Result<Memory> {
    instance
        .get_memory(&mut *store, "memory")
        .context("the module must export its memory as `memory`")
}

/// Wires the clocks a module imports, by the names WASI gives them and in
/// their core forms: the monotonic clock, which starts at zero here; the
/// system clock, whose `now` writes its record, the seconds since 1970 and
/// the nanoseconds, where the module points; and the zone's `utc-offset`
/// at an instant ([`wire_zone`]), from the system's zone. A module reads
/// them through `Std.Timers`, and a debug module imports every `{extern}`
/// of the libraries it reads, so each is wired whether a test reads it or
/// not.
pub fn wire_clocks<T: 'static>(linker: &mut Linker<T>, module: &Module) -> Result<()> {
    let clock_start = Instant::now();
    for import in module.imports() {
        let (module_name, name) = (import.module(), import.name());
        if name == "now" && module_name.contains("monotonic-clock") {
            linker
                .func_wrap(module_name, name, move || {
                    clock_start.elapsed().as_nanos() as i64
                })
                .ctx("wiring the monotonic clock")?;
        } else if name == "now" && module_name.contains("system-clock") {
            linker
                .func_wrap(
                    module_name,
                    name,
                    move |mut caller: Caller<'_, T>, ret: i32| -> wasmtime::Result<()> {
                        let since = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default();
                        // The record `instant`: an `s64`, then a `u32` 8 bytes on.
                        let mut instant = [0u8; 12];
                        instant[..8].copy_from_slice(&(since.as_secs() as i64).to_le_bytes());
                        instant[8..].copy_from_slice(&since.subsec_nanos().to_le_bytes());
                        write_at(&mut caller, ret as u32, &instant)
                    },
                )
                .ctx("wiring the system clock")?;
        } else if name == "utc-offset" && module_name.contains("timezone") {
            wire_zone(linker, module_name, system_zone)?;
        }
    }
    Ok(())
}

/// Wires `zone` as the `utc-offset` of `module_name`. It is handed an
/// instant as its seconds and nanoseconds, and gives the nanoseconds the
/// zone is east of UTC there, or `None` for a host with no zone. The core
/// form writes that `option<s64>` where the module points, as the
/// canonical ABI lays it out and nothing more: a byte saying whether it
/// holds a value, and the value 8 bytes on.
pub fn wire_zone<T: 'static>(
    linker: &mut Linker<T>,
    module_name: &str,
    zone: impl Fn(i64, i32) -> Option<i64> + Send + Sync + 'static,
) -> Result<()> {
    linker
        .func_wrap(
            module_name,
            "utc-offset",
            move |mut caller: Caller<'_, T>,
                  seconds: i64,
                  nanoseconds: i32,
                  ret: i32|
                  -> wasmtime::Result<()> {
                let ret = ret as u32;
                match zone(seconds, nanoseconds) {
                    Some(offset) => {
                        write_at(&mut caller, ret, &[1])?;
                        write_at(&mut caller, ret.wrapping_add(8), &offset.to_le_bytes())
                    }
                    None => write_at(&mut caller, ret, &[0]),
                }
            },
        )
        .ctx("wiring the time zone")?;
    Ok(())
}

/// Writes `bytes` in the calling module's memory at `address`.
fn write_at<T>(caller: &mut Caller<'_, T>, address: u32, bytes: &[u8]) -> wasmtime::Result<()> {
    let memory = caller
        .get_export("memory")
        .and_then(Extern::into_memory)
        .ok_or_else(|| wasmtime::Error::msg("the module exports no memory"))?;
    memory
        .write(&mut *caller, address as usize, bytes)
        .map_err(|e| wasmtime::Error::msg(format!("writing where the module points: {e}")))
}

/// How far east of UTC the system's zone is at an instant, in nanoseconds,
/// daylight saving included, or `None` when the system has no zone. An
/// instant past what the zone's rules cover takes the nearest they do.
fn system_zone(seconds: i64, nanoseconds: i32) -> Option<i64> {
    let zone = jiff::tz::TimeZone::try_system().ok()?;
    let at = jiff::Timestamp::new(seconds, nanoseconds).unwrap_or(if seconds < 0 {
        jiff::Timestamp::MIN
    } else {
        jiff::Timestamp::MAX
    });
    Some(i64::from(zone.to_offset(at).seconds()) * 1_000_000_000)
}

/// One of the host's two outputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stream {
    Stdout,
    Stderr,
}

/// What a module wrote to the host's standard output and error.
#[derive(Debug)]
pub struct Stdio {
    /// Each write, with its stream, in the order the module made them, as
    /// far as `limit` keeps them.
    writes: Vec<(Stream, Vec<u8>)>,
    /// The most bytes kept. A write past it is only counted.
    limit: usize,
    kept: usize,
    /// The bytes written past `limit`.
    dropped: usize,
}

impl Stdio {
    /// Keeps the first `limit` bytes written: 0 keeps none.
    pub fn keeping(limit: usize) -> Self {
        Stdio {
            writes: Vec::new(),
            limit,
            kept: 0,
            dropped: 0,
        }
    }

    /// Each write kept, with its stream, in the order the module made them.
    pub fn writes(&self) -> &[(Stream, Vec<u8>)] {
        &self.writes
    }

    /// Both outputs as one text, in the order they were written. Bytes that
    /// are not UTF-8 read as U+FFFD, and a last line says how many were
    /// written past the limit. Empty when nothing was written.
    pub fn text(&self) -> String {
        let bytes: Vec<u8> = self
            .writes
            .iter()
            .flat_map(|(_, bytes)| bytes.iter().copied())
            .collect();
        let mut text = String::from_utf8_lossy(&bytes).into_owned();
        if self.dropped > 0 {
            if !text.is_empty() && !text.ends_with('\n') {
                text.push('\n');
            }
            text.push_str(&format!("[{} more bytes not kept]", self.dropped));
        }
        text
    }

    fn record(&mut self, stream: Stream, bytes: &[u8]) {
        let kept = bytes.len().min(self.limit - self.kept);
        if kept > 0 {
            self.writes.push((stream, bytes[..kept].to_vec()));
            self.kept += kept;
        }
        self.dropped += bytes.len() - kept;
    }
}

fn lock(stdio: &Mutex<Stdio>) -> MutexGuard<'_, Stdio> {
    stdio.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Wires the standard output and error a core module writes to, to `stdio`:
/// the four functions [`mir::console`] imports for each, in their core
/// forms. `stream.new` answers the two ends of a stream, `write-via-stream`
/// takes the reading end and answers a future, `future.drop-readable` gives
/// that up, and `stream.write` takes all the bytes it is handed, as a host
/// that is ready does. A component gets these from its host's WASI and the
/// component model; a core module run alone gets them here.
pub fn wire_console<T: 'static>(
    linker: &mut Linker<T>,
    module: &Module,
    stdio: &Arc<Mutex<Stdio>>,
) -> Result<()> {
    use mir::console::{Import, Output};
    for (output, stream) in [
        (Output::Stdout, Stream::Stdout),
        (Output::Stderr, Stream::Stderr),
    ] {
        let interface = output.interface();
        if !module.imports().any(|import| import.module() == interface) {
            continue;
        }
        for import in Import::ALL {
            let stdio = stdio.clone();
            match import {
                // The reading end is 1 and the writing end 2: no table here.
                Import::NewStream => {
                    linker.func_wrap(&interface, import.name(), || 1i64 | (2i64 << 32))
                }
                Import::WriteViaStream => {
                    linker.func_wrap(&interface, import.name(), |_reading_end: i32| 1i32)
                }
                Import::DropFuture => {
                    linker.func_wrap(&interface, import.name(), |_future: i32| {})
                }
                Import::Write => linker.func_wrap(
                    &interface,
                    import.name(),
                    move |mut caller: Caller<'_, T>,
                          _writing_end: i32,
                          ptr: i32,
                          len: i32|
                          -> wasmtime::Result<i32> {
                        let memory = caller
                            .get_export("memory")
                            .and_then(Extern::into_memory)
                            .ok_or_else(|| wasmtime::Error::msg("the module exports no memory"))?;
                        let (start, len) = (ptr as u32 as usize, len as u32 as usize);
                        let bytes =
                            memory
                                .data(&caller)
                                .get(start..start + len)
                                .ok_or_else(|| {
                                    wasmtime::Error::msg(
                                        "a write of bytes outside the module's memory",
                                    )
                                })?;
                        lock(&stdio).record(stream, bytes);
                        // All of them taken, and the stream still open.
                        Ok((len as i32) << 4)
                    },
                ),
            }
            .ctx("wiring the standard output and error")?;
        }
    }
    Ok(())
}

/// One test on a fresh instance: a pass, a failure or a trap, with what it
/// wrote. `Err` is a module this host could not load at all.
fn run_one(
    engine: &Engine,
    module: &Module,
    entry: &TestEntry,
    budget: Duration,
) -> Result<Outcome> {
    let stdio = Arc::new(Mutex::new(Stdio::keeping(OUTPUT_LIMIT)));
    let mut outcome = run_one_with(engine, module, entry, budget, &stdio)?;
    outcome.output = lock(&stdio).text();
    Ok(outcome)
}

/// [`run_one`], its outputs written to `stdio`.
fn run_one_with(
    engine: &Engine,
    module: &Module,
    entry: &TestEntry,
    budget: Duration,
    stdio: &Arc<Mutex<Stdio>>,
) -> Result<Outcome> {
    let mut store = Store::new(engine, ());
    // A store with epoch interruption enabled starts at a deadline that has
    // always elapsed, so arm it before ANY wasm runs — instantiation included.
    store.set_epoch_deadline(ticks(budget));
    let linker = link(module, stdio)?;
    let instance = linker
        .instantiate(&mut store, module)
        .ctx("instantiating the module")?;
    let memory = memory_of(&mut store, &instance)?;

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

/// A target compiled once, ready to run each test on a fresh instance.
enum Runner<'a> {
    Core(&'a [u8], Module),
    Component {
        /// The component's bytes, and where the core module it wraps is in
        /// them.
        bytes: &'a [u8],
        module: Range<usize>,
        pre: InstancePre<ComponentHost>,
    },
}

impl<'a> Runner<'a> {
    fn new(engine: &Engine, target: Target<'a>) -> Result<Self> {
        Ok(match target {
            Target::Core(wasm) => {
                Runner::Core(wasm, Module::new(engine, wasm).ctx("compiling the module")?)
            }
            Target::Component(bytes) => {
                let component = Component::new(engine, bytes).ctx("compiling the component")?;
                let pre = link_component(engine, &component)?
                    .instantiate_pre(&component)
                    .ctx("linking the component")?;
                Runner::Component {
                    bytes,
                    module: wrapped_module(bytes)?,
                    pre,
                }
            }
        })
    }

    /// The core module: the one given, or the one the component wraps,
    /// which carries the test manifest and the line tables.
    fn core(&self) -> &'a [u8] {
        match self {
            Runner::Core(wasm, _) => wasm,
            Runner::Component { bytes, module, .. } => &bytes[module.clone()],
        }
    }

    /// The test `entry`, at `place` in the manifest.
    fn run(&self, entry: &TestEntry, place: u32, budget: Duration) -> Result<Outcome> {
        match self {
            Runner::Core(_, module) => run_one(module.engine(), module, entry, budget),
            Runner::Component { module, pre, .. } => run_on_component(pre, module, place, budget),
        }
    }
}

/// What a component's store holds: what wasmtime-wasi gives it, its clocks
/// and its standard output and error.
struct ComponentHost {
    ctx: WasiCtx,
    table: ResourceTable,
}

impl WasiView for ComponentHost {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.ctx,
            table: &mut self.table,
        }
    }
}

/// One of a component's two outputs, as wasmtime-wasi asks a host for it:
/// what is written to it is kept in `stdio`, and always taken at once.
#[derive(Clone)]
struct Sink {
    stream: Stream,
    stdio: Arc<Mutex<Stdio>>,
}

impl tokio::io::AsyncWrite for Sink {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
        bytes: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        lock(&self.stdio).record(self.stream, bytes);
        std::task::Poll::Ready(Ok(bytes.len()))
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::task::Poll::Ready(Ok(()))
    }
}

impl wasmtime_wasi::cli::IsTerminal for Sink {
    fn is_terminal(&self) -> bool {
        false
    }
}

impl wasmtime_wasi::cli::StdoutStream for Sink {
    fn async_stream(&self) -> Box<dyn tokio::io::AsyncWrite + Send + Sync> {
        Box::new(self.clone())
    }
}

/// The zone's interface, at the version the standard library imports.
const ZONE: &str = "wasi:clocks/timezone@0.3.0";

/// A component's linker: WASI's monotonic and system clocks and its
/// command line's interfaces, the standard output and error among them,
/// from wasmtime-wasi; the zone's `utc-offset` from the system's zone
/// ([`system_zone`]), since WASI's time zone is unstable and wasmtime-wasi
/// has none; and every other import a trap, as [`link`] stubs a module's.
fn link_component(
    engine: &Engine,
    component: &Component,
) -> Result<wasmtime::component::Linker<ComponentHost>> {
    let mut linker = wasmtime::component::Linker::new(engine);
    wasmtime_wasi::p3::clocks::add_to_linker(&mut linker).ctx("wiring WASI's clocks")?;
    wasmtime_wasi::p3::cli::add_to_linker(&mut linker)
        .ctx("wiring WASI's standard output and error")?;
    linker
        .instance(ZONE)
        .and_then(|mut zone| {
            zone.func_wrap("utc-offset", |_, (when,): (WasiInstant,)| {
                Ok((system_zone(when.seconds, when.nanoseconds as i32),))
            })
        })
        .ctx("wiring the zone")?;
    linker
        .define_unknown_imports_as_traps(component)
        .ctx("stubbing unwired host imports as traps")?;
    Ok(linker)
}

/// One test on a fresh instance of the component: `init`, then `run-test`
/// with the test's `place` in the manifest. A raise comes back as the
/// error of a result, a trap as the call's, placed in the core module at
/// `module` in the component.
fn run_on_component(
    pre: &InstancePre<ComponentHost>,
    module: &Range<usize>,
    place: u32,
    budget: Duration,
) -> Result<Outcome> {
    let stdio = Arc::new(Mutex::new(Stdio::keeping(OUTPUT_LIMIT)));
    let mut outcome = run_on_component_with(pre, module, place, budget, &stdio)?;
    outcome.output = lock(&stdio).text();
    Ok(outcome)
}

/// [`run_on_component`], its outputs written to `stdio`.
fn run_on_component_with(
    pre: &InstancePre<ComponentHost>,
    module: &Range<usize>,
    place: u32,
    budget: Duration,
    stdio: &Arc<Mutex<Stdio>>,
) -> Result<Outcome> {
    let sink = |stream| Sink {
        stream,
        stdio: stdio.clone(),
    };
    let mut store = Store::new(
        pre.engine(),
        ComponentHost {
            ctx: WasiCtxBuilder::new()
                .stdout(sink(Stream::Stdout))
                .stderr(sink(Stream::Stderr))
                .build(),
            table: ResourceTable::new(),
        },
    );
    // Armed before ANY wasm runs, instantiation included, as in `run_one`.
    store.set_epoch_deadline(ticks(budget));
    let instance = pre
        .instantiate(&mut store)
        .ctx("instantiating the component")?;
    let control = instance
        .get_export_index(&mut store, None, debug_format::wit::CONTROL_INTERFACE)
        .context("the component exports no control interface")?;
    let mut function = |name: &str| {
        instance
            .get_export_index(&mut store, Some(&control), name)
            .with_context(|| format!("the control interface has no `{name}`"))
    };
    let (init, run_test) = (function("init")?, function("run-test")?);
    let init = instance
        .get_typed_func::<(), (std::result::Result<(), String>,)>(&mut store, &init)
        .ctx("`init` must be a func() -> result<_, string>")?;
    let run_test = instance
        .get_typed_func::<(u32,), (std::result::Result<(), String>,)>(&mut store, &run_test)
        .ctx("`run-test` must be a func(u32) -> result<_, string>")?;

    store.set_epoch_deadline(ticks(budget));
    match init.call(&mut store, ()) {
        Ok((Ok(()),)) => {}
        Ok((Err(message),)) => {
            return Ok(Outcome::passed_or(Some(format!(
                "raised in initialization: {message}"
            ))));
        }
        Err(e) => {
            let reason = format!("trapped in initialization: {}", stopped(&e, budget));
            return Ok(Outcome::trapped_in(reason, &e, module));
        }
    }
    store.set_epoch_deadline(ticks(budget));
    Ok(match run_test.call(&mut store, (place,)) {
        Ok((result,)) => Outcome::passed_or(result.err()),
        Err(e) => {
            let reason = format!("trapped: {}", stopped(&e, budget));
            Outcome::trapped_in(reason, &e, module)
        }
    })
}

/// Where the core module `component` wraps is in its bytes: rk's own, the
/// one that exports its memory. The component's other modules are the
/// wrapper's (wit-component's shims), with neither the test manifest nor
/// the line tables.
fn wrapped_module(component: &[u8]) -> Result<Range<usize>> {
    for payload in wasmparser::Parser::new(0).parse_all(component) {
        if let wasmparser::Payload::ModuleSection {
            unchecked_range, ..
        } = payload.context("reading the component")?
            && exports_memory(&component[unchecked_range.clone()])
        {
            return Ok(unchecked_range);
        }
    }
    anyhow::bail!("the component wraps no module of rk's")
}

/// Whether `module` exports a memory named `memory`.
fn exports_memory(module: &[u8]) -> bool {
    for payload in wasmparser::Parser::new(0).parse_all(module) {
        if let Ok(wasmparser::Payload::ExportSection(exports)) = payload {
            return exports.into_iter().flatten().any(|export| {
                export.name == "memory" && export.kind == wasmparser::ExternalKind::Memory
            });
        }
    }
    false
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
/// the frames still there. The last one is the failure's: a `__CATCH` of
/// the test's own takes the ones before it and goes on.
#[derive(Clone)]
struct AtFailure;

impl DebugHandler for AtFailure {
    type Data = Trace;

    async fn handle(&self, mut store: StoreContextMut<'_, Trace>, event: DebugEvent<'_>) {
        // A hostcall's error is a failure too: the watchdog's interrupt
        // comes from one, and so does a call to an import nothing wired.
        // An exception is reported at its throw, before wasmtime looks for
        // a handler: a caught one too.
        let failed = matches!(
            event,
            DebugEvent::Exception(_) | DebugEvent::Trap(_) | DebugEvent::HostcallError(_)
        );
        if !failed {
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
    // What the test wrote was kept when it ran: this run is for its place.
    let quiet = Arc::new(Mutex::new(Stdio::keeping(0)));
    block_on(async {
        let Ok(linker) = link(module, &quiet) else {
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
    if let Some(words) = watchdog(err, budget) {
        return words;
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

/// A failed call to a component in a reader's words: the watchdog when it
/// fired, else the trap's own words. An exception never leaves a component:
/// its interface returns it as an error.
fn stopped(err: &wasmtime::Error, budget: Duration) -> String {
    watchdog(err, budget).unwrap_or_else(|| trap_words(err))
}

/// The watchdog's words, when it is what stopped the call.
fn watchdog(err: &wasmtime::Error, budget: Duration) -> Option<String> {
    matches!(
        err.downcast_ref::<wasmtime::Trap>(),
        Some(wasmtime::Trap::Interrupt)
    )
    .then(|| {
        format!(
            "the watchdog stopped it after {}: it never returned",
            describe(budget)
        )
    })
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A component's frames are brought back to its module's offsets, and
    /// a frame of another module, before or after it, is left out.
    #[test]
    fn a_component_s_frames_are_placed_in_its_module() {
        let stack = vec![(0, 406), (1, 418), (7, 10), (3, 1083), (2, 11)];
        assert_eq!(
            frames_of_module(stack, &(11..1083)),
            [(0, 395), (1, 407), (2, 0)]
        );
    }
}
