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
//!
//! The components of the libraries the program calls run beside it, as a
//! runtime runs them: each in a store of its own, fresh for each test, with
//! the program's WASI 0.3, driven by a task of its own that runs the
//! store's event loop for the whole test ([`drive`], after wasmCloud's
//! driver), as a runtime runs each on a thread of its own. Each function the
//! program imports from one is the host's, which sends the call to that
//! task, where it runs beside the calls under way. Values cross through
//! [`relocate`](crate::relocate): plain ones as they are, a handle of the
//! component's resource as the [`resource_bridge`](crate::resource_bridge)'s
//! proxy, a stream or a future of numbers, bools, chars or strings through a
//! channel ([`stream_pump`](crate::stream_pump)), which moves while both
//! stores run.
//! A handle the program drops is sent to the task, which drops the
//! component's own in its store. A trap of the component fails the
//! program's call, named after its library.

use std::future::Future;
use std::ops::Range;
use std::pin::Pin;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::control::{TestError, TestState};
use crate::relocate;
use crate::resource_bridge::{self, ProxyResource, ResourceRegistry};
use anyhow::{Context, Result};
use debug_format::DebugInfo;
use debug_format::test_manifest::{TEST_MANIFEST_SECTION, TestEntry, TestManifest};
use debug_format::test_report::{Frame, Status, TestRecord};
use tokio::sync::{mpsc, oneshot, watch};
use wasmtime::component::types::{ComponentFunc, ComponentItem};
use wasmtime::component::{Accessor, Component, Func, InstancePre, Resource, Val};
use wasmtime::{
    AsContextMut, Caller, Engine, Extern, Instance, Linker, Memory, Module, Store, StoreContextMut,
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

/// A library's component, which runs beside the program.
pub struct Beside {
    /// The library that carries it, which names it when it fails.
    pub library: String,
    pub bytes: Vec<u8>,
}

/// Run the tests of `component`, with the components `beside` it, optionally
/// filtered by a substring of their path, calling `on_result` as each
/// finishes. `budget` bounds each test (`None` = [`DEFAULT_BUDGET`]).
pub fn run_each(
    component: &[u8],
    beside: &[Beside],
    filter: Option<&str>,
    budget: Option<Duration>,
    mut on_result: impl FnMut(&TestRecord),
) -> Result<Vec<TestRecord>> {
    let budget = budget.unwrap_or(DEFAULT_BUDGET);
    let engine = engine()?;
    // Compiled once: instantiation is per test, compilation is not.
    let runner = Runner::new(&engine, component, beside)?;
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
    /// The components beside it, each compiled and linked once too.
    beside: Vec<BesidePre>,
    /// What it imports from them.
    wires: Wires,
    /// What each call runs on: wasmtime calls a component that imports a
    /// host function that waits, as WASI's `wait-for` does, through its
    /// async API only, and wasmtime-wasi waits on tokio's clock.
    runtime: tokio::runtime::Runtime,
}

/// A component beside the program, linked: the library that carries it.
struct BesidePre {
    library: String,
    pre: InstancePre<ComponentHost>,
}

/// What the program imports from the components beside it: each function,
/// and each resource type, whose handles cross between the stores through
/// the [`resource_bridge`].
#[derive(Default)]
struct Wires {
    functions: Vec<Wire>,
    /// The component that owns each resource, by its place in
    /// [`ComponentHost::beside`], the interface that declares it, and its
    /// name.
    resources: Vec<(usize, String, String)>,
}

/// A function the program imports from a component beside it: which
/// component, the interface it exports the function in, and whether WIT
/// declares it an `async func`.
struct Wire {
    beside: usize,
    interface: String,
    function: String,
    is_async: bool,
}

impl<'a> Runner<'a> {
    fn new(engine: &Engine, bytes: &'a [u8], beside: &[Beside]) -> Result<Self> {
        let beside = beside
            .iter()
            .map(|b| {
                let component = Component::new(engine, &b.bytes)
                    .ctx(format!("compiling the component of '{}'", b.library))?;
                Ok((b.library.clone(), component))
            })
            .collect::<Result<Vec<_>>>()?;
        let component = Component::new(engine, bytes).ctx("compiling the component")?;
        let wires = wires(engine, &component, &beside);
        let pre = link_component(engine, &component, &wires)?
            .instantiate_pre(&component)
            .ctx("linking the component")?;
        let beside = beside
            .into_iter()
            .map(|(library, component)| {
                let pre = link_component(engine, &component, &Wires::default())?
                    .instantiate_pre(&component)
                    .ctx(format!("linking the component of '{library}'"))?;
                Ok(BesidePre { library, pre })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Runner {
            bytes,
            module: wrapped_module(bytes)?,
            pre,
            beside,
            wires,
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
            (&self.beside, &self.wires),
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
/// The program's store also holds the components beside it.
struct ComponentHost {
    ctx: WasiCtx,
    table: ResourceTable,
    /// Whether the stack is taken at each raise.
    capture: bool,
    /// The stack when the module last said it raised, as [`stack_of`]
    /// gives it.
    raised: Option<Vec<(u32, u32)>>,
    /// What a call may take before the watchdog stops it, a call into a
    /// component beside the program as much as one of the program's.
    budget: Duration,
    /// The components beside the program, each in a store of its own.
    beside: Vec<Started>,
    /// Each function the program imports from them, by [`Wire`].
    forwards: Vec<Forward>,
    /// A component's store: the handles of its resources the program
    /// holds ([`resource_bridge`]).
    resource_registry: Option<ResourceRegistry>,
}

impl resource_bridge::Bridged for ComponentHost {
    fn table(&mut self) -> &mut ResourceTable {
        &mut self.table
    }

    fn resource_registry(&mut self) -> Option<&mut ResourceRegistry> {
        self.resource_registry.as_mut()
    }
}

/// A value on its way between the program's store and a component's, both
/// holding a [`ComponentHost`].
type Relocated = relocate::Relocated<ComponentHost>;

/// A component beside the program, started for one test and driven by a
/// task of its own ([`drive`]), as a runtime runs each component on a
/// thread of its own: the program sends it the calls it makes and the
/// handles it drops, and the task runs the component's event loop all the
/// while, so that what crosses as a stream or a future moves between the
/// program's calls.
struct Started {
    library: String,
    jobs: mpsc::UnboundedSender<Job>,
    /// How the component trapped, once it has: its task is gone, and each
    /// call of the program's into it fails with it.
    trapped: watch::Receiver<Option<String>>,
}

/// What the program sends the task of a component beside it.
enum Job {
    /// A call of `func`: its parameters put into the component's store, its
    /// results taken out of it into `reply`.
    Call {
        func: Func,
        params: Vec<Relocated>,
        reply: oneshot::Sender<wasmtime::Result<Vec<Relocated>>>,
    },
    /// The program let go of the handle `proxy_id` names: the component's
    /// own goes, in the component's store.
    Drop { proxy_id: u64 },
}

/// Where a function the program imports runs: the component beside it, by
/// its place in [`ComponentHost::beside`], and the function it exports.
#[derive(Clone, Copy)]
struct Forward {
    beside: usize,
    func: Func,
}

/// A trap in a component beside the program, which fails the program's
/// call into it: the library that carries the component, and the trap in
/// a reader's words.
#[derive(Debug)]
struct InComponent {
    library: String,
    words: String,
}

impl std::fmt::Display for InComponent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "the component of '{}' trapped: {}",
            self.library, self.words
        )
    }
}

impl std::error::Error for InComponent {}

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
/// has none; `rk:test/raise`'s `raised`, which takes the stack; each
/// function of `wires`, made in the component beside it that exports it
/// ([`forward`]); and every other import a trap: a user can declare any
/// `{extern}` host function, so imports cannot be whitelisted, and an
/// unwired one fails only if the test calls it. A component beside the
/// program is linked the same way, with no wires.
fn link_component(
    engine: &Engine,
    component: &Component,
    wires: &Wires,
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
    // Every resource the program imports from a component is the bridge's
    // one host type: the program holds proxies, the component the handles.
    for (beside, interface, resource) in &wires.resources {
        let beside = *beside;
        linker
            .instance(interface)
            .and_then(|mut instance| {
                instance.resource(
                    resource,
                    resource_bridge::proxy_resource_type(),
                    move |mut store, rep| {
                        let proxy = store
                            .data_mut()
                            .table
                            .delete(Resource::<ProxyResource>::new_own(rep))?;
                        // Sent to the component's task, which drops its own
                        // handle in its store; gone with the task once the
                        // component trapped.
                        let _ = store.data().beside[beside].jobs.send(Job::Drop {
                            proxy_id: proxy.proxy_id,
                        });
                        Ok(())
                    },
                )
            })
            .ctx(format!("wiring the resource `{resource}` of `{interface}`"))?;
    }
    for (wire, w) in wires.functions.iter().enumerate() {
        // An `async func` is the host's to run concurrently, as the program
        // calls it: the call is under way while the program scans.
        linker
            .instance(&w.interface)
            .and_then(|mut instance| match w.is_async {
                true => instance.func_new_concurrent(
                    &w.function,
                    move |accessor, ty, params, results| {
                        forward_async(accessor, wire, ty, params, results)
                    },
                ),
                false => instance.func_new_async(&w.function, move |store, ty, params, results| {
                    forward(store, wire, ty, params, results)
                }),
            })
            .ctx(format!("wiring `{}` of `{}`", w.function, w.interface))?;
    }
    linker
        .define_unknown_imports_as_traps(component)
        .ctx("stubbing unwired host imports as traps")?;
    Ok(linker)
}

/// What `program` imports from the components `beside` it: each function
/// and each resource of each interface it imports that one of them
/// exports. The build let no two of them export one interface.
fn wires(engine: &Engine, program: &Component, beside: &[(String, Component)]) -> Wires {
    let mut exporter = std::collections::HashMap::new();
    for (index, (_, component)) in beside.iter().enumerate() {
        for (name, item) in component.component_type().exports(engine) {
            if let ComponentItem::ComponentInstance(_) = item.ty {
                exporter.entry(name.to_string()).or_insert(index);
            }
        }
    }
    let mut wires = Wires::default();
    for (interface, item) in program.component_type().imports(engine) {
        let (Some(&beside), ComponentItem::ComponentInstance(instance)) =
            (exporter.get(interface), &item.ty)
        else {
            continue;
        };
        for (name, item) in instance.exports(engine) {
            match item.ty {
                ComponentItem::ComponentFunc(ty) => wires.functions.push(Wire {
                    beside,
                    interface: interface.to_string(),
                    function: name.to_string(),
                    is_async: ty.async_(),
                }),
                ComponentItem::Resource(_) => {
                    wires
                        .resources
                        .push((beside, interface.to_string(), name.to_string()))
                }
                _ => {}
            }
        }
    }
    wires
}

/// The program's call of the function at `wire`, a `func`, sent to the
/// task of the component beside it that exports it ([`send_call`]). The
/// program waits for it; the component runs it beside the calls it has
/// under way.
fn forward<'a>(
    mut store: StoreContextMut<'a, ComponentHost>,
    wire: usize,
    ty: ComponentFunc,
    params: &'a [Val],
    results: &'a mut [Val],
) -> Box<dyn Future<Output = wasmtime::Result<()>> + Send + 'a> {
    Box::new(async move {
        let Forward { beside, func } = store.data().forwards[wire];
        let params = taken_out(&mut store, &ty, params)?;
        let got = send_call(store.data(), beside, func, params).await?;
        put_in(&mut store, got, results)
    })
}

/// [`forward`] for an `async func`, under way while the program scans.
fn forward_async<'a>(
    accessor: &'a Accessor<ComponentHost>,
    wire: usize,
    ty: ComponentFunc,
    params: &'a [Val],
    results: &'a mut [Val],
) -> Pin<Box<dyn Future<Output = wasmtime::Result<()>> + Send + 'a>> {
    Box::pin(async move {
        let call = accessor.with(|mut access| -> wasmtime::Result<_> {
            let Forward { beside, func } = access.data_mut().forwards[wire];
            let params = taken_out(&mut access, &ty, params)?;
            Ok(send_call(access.data_mut(), beside, func, params))
        })?;
        let got = call.await?;
        accessor.with(|mut access| put_in(&mut access, got, results))
    })
}

/// The program's `params`, typed by its import `ty`, taken out of its
/// store ([`relocate::extract`]).
fn taken_out(
    mut store: impl AsContextMut<Data = ComponentHost>,
    ty: &ComponentFunc,
    params: &[Val],
) -> wasmtime::Result<Vec<Relocated>> {
    ty.params()
        .zip(params)
        .map(|((_, ty), value)| relocate::extract(&mut store, value, &ty))
        .collect()
}

/// What the component gave back, `got`, put into the program's store as
/// the import's `results` ([`relocate::inject`]).
fn put_in(
    mut store: impl AsContextMut<Data = ComponentHost>,
    got: Vec<Relocated>,
    results: &mut [Val],
) -> wasmtime::Result<()> {
    for (slot, value) in results.iter_mut().zip(got) {
        *slot = relocate::inject(&mut store, value)?;
    }
    Ok(())
}

/// The call of `func` with `params`, sent at once to the task of the
/// component at `beside`, and what it gives back when it comes. A trap of
/// the component comes back as [`InComponent`], which traps the program's
/// call where it made it.
fn send_call(
    host: &ComponentHost,
    beside: usize,
    func: Func,
    params: Vec<Relocated>,
) -> impl Future<Output = wasmtime::Result<Vec<Relocated>>> + Send + use<> {
    let Started {
        library,
        jobs,
        trapped,
    } = &host.beside[beside];
    let (library, mut trapped, budget) = (library.clone(), trapped.clone(), host.budget);
    let (reply, answer) = oneshot::channel();
    let sent = jobs.send(Job::Call {
        func,
        params,
        reply,
    });
    async move {
        let in_component = |words| {
            wasmtime::Error::new(InComponent {
                library: library.clone(),
                words,
            })
        };
        if sent.is_ok() {
            match answer.await {
                Ok(Ok(results)) => return Ok(results),
                Ok(Err(e)) => return Err(in_component(stopped(&e, budget))),
                Err(_) => {}
            }
        }
        // The task is gone with the call: the component trapped.
        let words = trapped
            .wait_for(Option::is_some)
            .await
            .map(|words| words.clone().unwrap_or_default())
            .unwrap_or_else(|_| "its task ended".to_string());
        Err(in_component(words))
    }
}

/// The task of a component beside the program, which runs its store's
/// event loop until the program lets go of `jobs`, the way wasmCloud's
/// driver serves a component: each call runs in the store beside the
/// others under way, with the program's budget, and each handle the program
/// drops is staged in the store's registry and dropped there, in the order
/// the program sent them. A trap ends the task: `trapped` says how, for
/// the calls that come after.
async fn drive(
    mut store: Store<ComponentHost>,
    mut jobs: mpsc::UnboundedReceiver<Job>,
    budget: Duration,
    trapped: watch::Sender<Option<String>>,
) {
    let served = store
        .run_concurrent(async |accessor| -> wasmtime::Result<()> {
            while let Some(job) = jobs.recv().await {
                match job {
                    Job::Call {
                        func,
                        params,
                        reply,
                    } => {
                        accessor.spawn(async move |accessor: &Accessor<ComponentHost>| {
                            let _ = reply.send(serve_call(accessor, func, params, budget).await);
                            Ok(())
                        })?;
                    }
                    Job::Drop { proxy_id } => {
                        let pending = accessor.with(|mut access| {
                            match access.data_mut().resource_registry.as_mut() {
                                Some(registry) => {
                                    registry.stage_drop(proxy_id);
                                    registry.take_pending_drops()
                                }
                                None => Vec::new(),
                            }
                        });
                        for real in pending {
                            real.resource_drop_concurrent(accessor).await?;
                        }
                    }
                }
            }
            Ok(())
        })
        .await;
    let error = match served {
        Ok(Ok(())) => return,
        Ok(Err(e)) | Err(e) => e,
    };
    let _ = trapped.send(Some(stopped(&error, budget)));
}

/// A call the program sent, run in the component's store: its parameters
/// put into the store, `func` called beside the calls under way, its
/// results taken out ([`relocate`]).
async fn serve_call(
    accessor: &Accessor<ComponentHost>,
    func: Func,
    params: Vec<Relocated>,
    budget: Duration,
) -> wasmtime::Result<Vec<Relocated>> {
    let (params, mut results) = accessor.with(|mut access| -> wasmtime::Result<_> {
        access.as_context_mut().set_epoch_deadline(ticks(budget));
        let params = params
            .into_iter()
            .map(|param| relocate::inject(&mut access, param))
            .collect::<wasmtime::Result<Vec<_>>>()?;
        let results = vec![Val::Bool(false); func.ty(&access).results().len()];
        Ok((params, results))
    })?;
    func.call_concurrent(accessor, &params, &mut results)
        .await?;
    accessor.with(|mut access| {
        let ty = func.ty(&access);
        results
            .iter()
            .zip(ty.results())
            .map(|(value, ty)| relocate::extract(&mut access, value, &ty))
            .collect()
    })
}

/// The tasks of the components beside the program, stopped when the test
/// is over, whatever its end: none outlives its test.
struct Drivers(Vec<tokio::task::JoinHandle<()>>);

impl Drop for Drivers {
    fn drop(&mut self) {
        for driver in &self.0 {
            driver.abort();
        }
    }
}

/// One test on a fresh instance of the component: `init`, then `run-test`
/// with the test's `place` in the manifest, once for a test FUNCTION and
/// once per cycle for a test FUNCTION_BLOCK, its outputs written to `stdio`.
/// A raise comes back as the error of a result, placed where the module
/// last said it raised, a trap as the call's error, with its backtrace:
/// both in the core module at `module` in the component. The components
/// beside it start first, fresh as it is, and write to `stdio` too.
async fn run_test(
    pre: &InstancePre<ComponentHost>,
    module: &Range<usize>,
    (beside, wires): (&[BesidePre], &Wires),
    place: u32,
    budget: Duration,
    capture: bool,
    stdio: &Arc<Mutex<Stdio>>,
) -> Result<Outcome> {
    let host = |capture| ComponentHost {
        ctx: WasiCtxBuilder::new()
            .stdout(Sink {
                stream: Stream::Stdout,
                stdio: stdio.clone(),
            })
            .stderr(Sink {
                stream: Stream::Stderr,
                stdio: stdio.clone(),
            })
            .build(),
        table: ResourceTable::new(),
        capture,
        raised: None,
        budget,
        beside: Vec::new(),
        forwards: Vec::new(),
        resource_registry: None,
    };
    let mut started = Vec::with_capacity(beside.len());
    for BesidePre { library, pre } in beside {
        let mut store = Store::new(
            pre.engine(),
            ComponentHost {
                resource_registry: Some(ResourceRegistry::default()),
                ..host(false)
            },
        );
        store.set_epoch_deadline(ticks(budget));
        match pre.instantiate_async(&mut store).await {
            Ok(instance) => started.push((library.clone(), store, instance)),
            Err(e) => {
                return Ok(Outcome::failed(format!(
                    "the component of '{library}' did not start: {}",
                    stopped(&e, budget)
                )));
            }
        }
    }
    let mut forwards = Vec::with_capacity(wires.functions.len());
    for wire in &wires.functions {
        let (library, store, instance) = &mut started[wire.beside];
        let func = instance
            .get_export_index(&mut *store, None, &wire.interface)
            .and_then(|interface| {
                instance.get_export_index(&mut *store, Some(&interface), &wire.function)
            })
            .and_then(|function| instance.get_func(&mut *store, function))
            .with_context(|| {
                format!(
                    "the component of '{library}' exports no `{}` in `{}`",
                    wire.function, wire.interface
                )
            })?;
        forwards.push(Forward {
            beside: wire.beside,
            func,
        });
    }
    let mut program = host(capture);
    let mut drivers = Drivers(Vec::with_capacity(started.len()));
    program.beside = started
        .into_iter()
        .map(|(library, store, _)| {
            let (jobs, inbox) = mpsc::unbounded_channel();
            let (trap, trapped) = watch::channel(None);
            drivers
                .0
                .push(tokio::spawn(drive(store, inbox, budget, trap)));
            Started {
                library,
                jobs,
                trapped,
            }
        })
        .collect();
    program.forwards = forwards;
    let mut store = Store::new(pre.engine(), program);
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
            let reason = trapped("trapped in initialization", &e, budget);
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
                // A cycle that waits on nothing never parks the runtime,
                // whose timers fire only when it does: a host's `wait-for`
                // would never end. The runtime has a turn between cycles,
                // as it has one between a task's scans.
                tokio::task::yield_now().await;
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
                let reason = trapped("trapped", &e, budget);
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

/// Why a call of the program's failed, after `what`: in a component beside
/// it, named after its library, or in the program itself.
fn trapped(what: &str, err: &wasmtime::Error, budget: Duration) -> String {
    match err.downcast_ref::<InComponent>() {
        Some(InComponent { library, words }) => {
            format!("{what} in the component of '{library}': {words}")
        }
        None => format!("{what}: {}", stopped(err, budget)),
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
