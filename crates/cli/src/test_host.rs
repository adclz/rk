// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! Running a component's tests in-process on a wasmtime host, as `rk test`
//! does: through its control interface, `init` and then `run-test` with the
//! test's place in the manifest, with WASI's clocks and standard output and
//! error. A test FUNCTION runs in one call. A test FUNCTION_BLOCK runs a
//! cycle per call, until a cycle leaves its DONE TRUE or the test's budget
//! is spent.
//! Each test gets a fresh instance, since a test mutates the same statics a
//! program does.
//!
//! A test that fails is reported with the calls in progress where it did,
//! named by the line tables of the core module the component wraps. A trap
//! comes back with wasmtime's backtrace. A raise brings none, and the test's
//! wrapper catches it inside the module, so the module `rk test` builds
//! calls `rk:test/raise`'s `raised` just before each raise
//! ([`mir::raise_hook`]), and the host takes the stack there: a test that
//! fails on a raise failed at the last one. Either way a frame's offset
//! counts from the start of the component, and is brought back to the
//! module's. An optimized build has no line tables: its failures are
//! reported without the calls.
//!
//! What a test writes to the standard output and error is kept, and
//! reported with the test when it fails.

use std::ops::Range;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::control::{TestError, TestState};
use anyhow::{Context, Result};
use debug_format::DebugInfo;
use debug_format::test_manifest::{TEST_MANIFEST_SECTION, TestEntry, TestManifest};
use debug_format::test_report::{Frame, Status, TestRecord};
use wasmtime::component::{Component, InstancePre};
use wasmtime::{Caller, Engine, Extern, Instance, Linker, Memory, Module, Store, StoreContextMut};
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

/// Run the tests of `component`, optionally filtered by a substring of
/// their path, calling `on_result` as each finishes. `budget` bounds each
/// test (`None` = [`DEFAULT_BUDGET`]).
pub fn run_each(
    component: &[u8],
    filter: Option<&str>,
    budget: Option<Duration>,
    mut on_result: impl FnMut(&TestRecord),
) -> Result<Vec<TestRecord>> {
    let budget = budget.unwrap_or(DEFAULT_BUDGET);
    let engine = engine()?;
    // Compiled once: instantiation is per test, compilation is not.
    let runner = Runner::new(&engine, component)?;
    let core = runner.core();
    let info = DebugInfo::from_wasm(core);
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
            .run(place, budget, info.has_lines())
            .with_context(|| format!("loading the module to run `{}`", entry.path))?;
        let duration_us = start.elapsed().as_micros() as u64;
        let failed = outcome.reason.is_some();
        // Without line tables, nothing names the calls.
        let backtrace = match failed && info.has_lines() {
            true => frames(&info, &outcome.stack),
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
/// assertions), the component model's async built-ins WASI 0.3 is written
/// with, and epoch interruption, the watchdog's mechanism; a ticker thread
/// advances the epoch.
fn engine() -> Result<Engine> {
    let mut config = wasmtime::Config::new();
    config.wasm_exceptions(true);
    // wasmtime-wasi's WASI 0.3 clocks: `wait-for` and `wait-until` are
    // async, though a component that only reads `now` runs without them.
    config.wasm_component_model_async(true);
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

/// How one test went: `reason` is `None` on a pass, and `stack` the calls
/// in progress where it failed, as `(function index, module offset)` pairs,
/// innermost first: a trap's backtrace, or the stack at the last raise.
/// `output` is what the test wrote, [`Stdio::text`].
struct Outcome {
    reason: Option<String>,
    stack: Vec<(u32, u32)>,
    output: String,
}

impl Outcome {
    fn passed() -> Self {
        Outcome {
            reason: None,
            stack: Vec::new(),
            output: String::new(),
        }
    }

    /// A failure with no calls in progress to show.
    fn failed(reason: String) -> Self {
        Outcome {
            reason: Some(reason),
            stack: Vec::new(),
            output: String::new(),
        }
    }

    /// A failure on a raise, `stack` the one the host took when the module
    /// said it raised, counted from the start of the component whose core
    /// module is at `module` in its bytes.
    fn raised(reason: String, stack: Option<Vec<(u32, u32)>>, module: &Range<usize>) -> Self {
        Outcome {
            reason: Some(reason),
            stack: frames_of_module(stack.unwrap_or_default(), module),
            output: String::new(),
        }
    }

    /// A trap of the component whose core module is at `module` in its
    /// bytes. A frame's offset counts from the start of the component: the
    /// frames of the module are kept, at their offsets in it. Another
    /// module's, a shim of the wrapper's, has nothing the tables name.
    fn trapped_in(reason: String, err: &wasmtime::Error, module: &Range<usize>) -> Self {
        let stack = err
            .downcast_ref::<wasmtime::WasmBacktrace>()
            .map(stack_of)
            .unwrap_or_default();
        Outcome {
            reason: Some(reason),
            stack: frames_of_module(stack, module),
            output: String::new(),
        }
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

/// The memory a module defines and exports as `memory`.
pub fn memory_of<T>(store: &mut Store<T>, instance: &Instance) -> Result<Memory> {
    instance
        .get_memory(&mut *store, "memory")
        .context("the module must export its memory as `memory`")
}

/// Wires the clocks a core module imports, by the names WASI gives them and
/// in their core forms, for a host that runs one alone, as the codegen tests
/// and the fuzz host do: the monotonic clock, which starts at zero here;
/// the system clock's `now` ([`wire_wall`]), from the system's clock; and
/// the zone's `utc-offset` at an instant ([`wire_zone`]), from the system's
/// zone. A module reads them through `Std.Timers`, and a debug module
/// imports every `{extern}` of the libraries it reads, so each is wired
/// whether the program reads it or not.
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
            wire_wall(linker, module_name, system_now)?;
        } else if name == "utc-offset" && module_name.contains("timezone") {
            wire_zone(linker, module_name, system_zone)?;
        }
    }
    Ok(())
}

/// Wires `now` as the `now` of `module_name`, a system clock. It gives an
/// instant as WASI's `instant` counts it: the whole seconds since
/// 1970-01-01, negative before, and the nanoseconds after them. The core
/// form writes that record where the module points, as the canonical ABI
/// lays it out: the `s64`, then the `u32` 8 bytes on.
pub fn wire_wall<T: 'static>(
    linker: &mut Linker<T>,
    module_name: &str,
    now: impl Fn() -> (i64, u32) + Send + Sync + 'static,
) -> Result<()> {
    linker
        .func_wrap(
            module_name,
            "now",
            move |mut caller: Caller<'_, T>, ret: i32| -> wasmtime::Result<()> {
                let (seconds, nanoseconds) = now();
                let mut instant = [0u8; 12];
                instant[..8].copy_from_slice(&seconds.to_le_bytes());
                instant[8..].copy_from_slice(&nanoseconds.to_le_bytes());
                write_at(&mut caller, ret as u32, &instant)
            },
        )
        .ctx("wiring the system clock")?;
    Ok(())
}

/// The system's clock, as [`wire_wall`] hands it over. A clock set before
/// 1970 reads 1970-01-01.
fn system_now() -> (i64, u32) {
    let since = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    (since.as_secs() as i64, since.subsec_nanos())
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

/// A component compiled once, ready to run each test on a fresh instance.
struct Runner<'a> {
    /// The component's bytes, and where the core module it wraps is in
    /// them.
    bytes: &'a [u8],
    module: Range<usize>,
    pre: InstancePre<ComponentHost>,
    /// What each call runs on: wasmtime calls a component that imports a
    /// host function that waits, as WASI's `wait-for` does, through its
    /// async API only, and wasmtime-wasi waits on tokio's clock.
    runtime: tokio::runtime::Runtime,
}

impl<'a> Runner<'a> {
    fn new(engine: &Engine, bytes: &'a [u8]) -> Result<Self> {
        let component = Component::new(engine, bytes).ctx("compiling the component")?;
        let pre = link_component(engine, &component)?
            .instantiate_pre(&component)
            .ctx("linking the component")?;
        Ok(Runner {
            bytes,
            module: wrapped_module(bytes)?,
            pre,
            runtime: tokio::runtime::Builder::new_current_thread()
                .enable_time()
                .build()
                .context("starting the runtime the tests' calls run on")?,
        })
    }

    /// The core module the component wraps, which carries the test manifest
    /// and the line tables.
    fn core(&self) -> &'a [u8] {
        &self.bytes[self.module.clone()]
    }

    /// The test at `place` in the manifest. `capture`: whether the stack is
    /// taken at each raise, which only a module with line tables can name.
    fn run(&self, place: u32, budget: Duration, capture: bool) -> Result<Outcome> {
        let stdio = Arc::new(Mutex::new(Stdio::keeping(OUTPUT_LIMIT)));
        let mut outcome = self.runtime.block_on(run_test(
            &self.pre,
            &self.module,
            place,
            budget,
            capture,
            &stdio,
        ))?;
        outcome.output = lock(&stdio).text();
        Ok(outcome)
    }
}

/// What a component's store holds: what wasmtime-wasi gives it, its clocks
/// and its standard output and error, and the stack at the last raise.
struct ComponentHost {
    ctx: WasiCtx,
    table: ResourceTable,
    /// Whether the stack is taken at each raise.
    capture: bool,
    /// The stack when the module last said it raised, as [`stack_of`]
    /// gives it.
    raised: Option<Vec<(u32, u32)>>,
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
/// has none; `rk:test/raise`'s `raised`, which takes the stack; and every
/// other import a trap: a user can declare any `{extern}` host function, so
/// imports cannot be whitelisted, and an unwired one fails only if the test
/// calls it.
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
        .instance(debug_format::wit::TEST_INTERFACE)
        .and_then(|mut test| {
            test.func_wrap(
                mir::raise_hook::FUNCTION,
                |mut store: StoreContextMut<'_, ComponentHost>, (): ()| {
                    // The exception has not unwound anything yet: the calls
                    // are all there, the innermost at its call to here.
                    if store.data().capture {
                        let stack = stack_of(&wasmtime::WasmBacktrace::force_capture(&store));
                        store.data_mut().raised = Some(stack);
                    }
                    Ok(())
                },
            )
        })
        .ctx("wiring the raise hook")?;
    linker
        .define_unknown_imports_as_traps(component)
        .ctx("stubbing unwired host imports as traps")?;
    Ok(linker)
}

/// One test on a fresh instance of the component: `init`, then `run-test`
/// with the test's `place` in the manifest, once for a test FUNCTION and
/// once per cycle for a test FUNCTION_BLOCK, its outputs written to `stdio`.
/// A raise comes back as the error of a result, placed where the module
/// last said it raised, a trap as the call's error, with its backtrace:
/// both in the core module at `module` in the component.
async fn run_test(
    pre: &InstancePre<ComponentHost>,
    module: &Range<usize>,
    place: u32,
    budget: Duration,
    capture: bool,
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
            capture,
            raised: None,
        },
    );
    // A store with epoch interruption enabled starts at a deadline that has
    // always elapsed, so arm it before ANY wasm runs, instantiation included.
    store.set_epoch_deadline(ticks(budget));
    let instance = pre
        .instantiate_async(&mut store)
        .await
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
        .get_typed_func::<(u32,), (std::result::Result<TestState, TestError>,)>(
            &mut store, &run_test,
        )
        .ctx("`run-test` must be a func(u32) -> result<test-state, test-error>")?;

    store.set_epoch_deadline(ticks(budget));
    match init.call_async(&mut store, ()).await {
        Ok((Ok(()),)) => {}
        Ok((Err(message),)) => {
            let stack = store.data_mut().raised.take();
            return Ok(Outcome::raised(
                format!("raised in initialization: {message}"),
                stack,
                module,
            ));
        }
        Err(e) => {
            let reason = format!("trapped in initialization: {}", stopped(&e, budget));
            return Ok(Outcome::trapped_in(reason, &e, module));
        }
    }
    // A test FUNCTION passes in one call. A test FUNCTION_BLOCK runs a cycle
    // a call, `running` until one leaves its DONE TRUE: each call has the
    // budget to return, and the test has it to pass.
    let start = Instant::now();
    let mut cycles: u64 = 0;
    let outcome = loop {
        store.set_epoch_deadline(ticks(budget));
        match run_test.call_async(&mut store, (place,)).await {
            Ok((Ok(TestState::Passed),)) => break Outcome::passed(),
            Ok((Ok(TestState::Running),)) => {
                cycles += 1;
                if start.elapsed() >= budget {
                    break Outcome::failed(format!(
                        "the watchdog stopped it after {}: DONE was still FALSE after {cycles} cycles",
                        describe(budget)
                    ));
                }
            }
            Ok((Err(TestError::Failed(message)),)) => {
                let stack = store.data_mut().raised.take();
                break Outcome::raised(message, stack, module);
            }
            // The manifest's own place: only a release build answers this.
            Ok((Err(TestError::NoSuchTest),)) => {
                break Outcome::raised("no such test".to_string(), None, module);
            }
            Err(e) => {
                let reason = format!("trapped: {}", stopped(&e, budget));
                break Outcome::trapped_in(reason, &e, module);
            }
        }
    };
    Ok(outcome)
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
