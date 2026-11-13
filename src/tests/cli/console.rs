// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! The standard output and error of a component: WASI 0.3's, a stream of
//! bytes each, as wasmtime-wasi gives them to a host. `Std.Console` writes
//! to them in ST, over WASI's `write-via-stream` and the component model's
//! stream built-ins, which a scan never waits for.

use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use rstest::rstest;
use wasmtime::component::{Component, Linker};
use wasmtime_wasi::cli::{IsTerminal, StdoutStream};
use wasmtime_wasi::{ResourceTable, WasiCtx, WasiCtxBuilder, WasiCtxView, WasiView};

use crate::tests::codegen::{
    STD_CONSOLE, compile_to_mir_and_wasm_as_built, compile_to_mir_and_wasm_as_built_with_libraries,
    with_db,
};

const CONTROL: &str = debug_format::wit::CONTROL_INTERFACE;

/// A task that writes to both outputs with `Std.Console`: its input's bit
/// 0 is the standard output's REQ, bit 1 the standard error's, and its
/// output says how each block stands, in the bits below.
const WRITER: &str = r#"
    USING Std.Console;

    PROGRAM P
    VAR_EXTERNAL input : DINT; output : DINT; END_VAR
    VAR out : STDOUT; err : STDERR; END_VAR
        out(REQ := input = 1 OR input = 3, IN := 'pump started$N');
        err(REQ := input = 2 OR input = 3, IN := 'pressure is high$N');
        output := 0;
        IF out.DONE THEN output := output + 1; END_IF;
        IF out.BUSY THEN output := output + 2; END_IF;
        IF out.ERROR THEN output := output + 4; END_IF;
        IF err.DONE THEN output := output + 16; END_IF;
        IF err.BUSY THEN output := output + 32; END_IF;
        IF err.ERROR THEN output := output + 64; END_IF;
    END_PROGRAM

    CONFIGURATION Cfg
    VAR_GLOBAL
        input AT %ID0 : DINT;
        output AT %QD0 : DINT;
    END_VAR
        RESOURCE Res ON CPU
            TASK T(INTERVAL := T#10ms, PRIORITY := 1);
            PROGRAM Main WITH T : P;
        END_RESOURCE
    END_CONFIGURATION
"#;

/// How each block stands, in the task's output.
const OUT_DONE: i32 = 1;
const OUT_BUSY: i32 = 2;
const OUT_ERROR: i32 = 4;
const ERR_DONE: i32 = 16;

/// What a host's two outputs received, in the order written.
type Written = Arc<Mutex<Vec<(&'static str, Vec<u8>)>>>;

/// How a host's output behaves.
#[derive(Clone)]
enum Sink {
    /// Takes what it is given, `at_most` bytes at a time.
    Ready {
        name: &'static str,
        at_most: usize,
        written: Written,
    },
    /// Never ready: it takes nothing, and does not say when it will.
    Stalled,
    /// Takes nothing until its gate opens, which wakes the host's write.
    Gated { gate: Arc<Gate>, written: Written },
    /// Lets go of its end at the first write.
    Closed,
}

impl tokio::io::AsyncWrite for Sink {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        match &*self {
            Sink::Ready {
                name,
                at_most,
                written,
            } => {
                let taken = bytes.len().min(*at_most);
                written
                    .lock()
                    .unwrap()
                    .push((name, bytes[..taken].to_vec()));
                Poll::Ready(Ok(taken))
            }
            Sink::Stalled => Poll::Pending,
            Sink::Gated { gate, written } => {
                if !gate.open.load(std::sync::atomic::Ordering::SeqCst) {
                    *gate.waker.lock().unwrap() = Some(cx.waker().clone());
                    return Poll::Pending;
                }
                written.lock().unwrap().push(("out", bytes.to_vec()));
                Poll::Ready(Ok(bytes.len()))
            }
            Sink::Closed => Poll::Ready(Ok(0)),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

impl IsTerminal for Sink {
    fn is_terminal(&self) -> bool {
        false
    }
}

impl StdoutStream for Sink {
    fn async_stream(&self) -> Box<dyn tokio::io::AsyncWrite + Send + Sync> {
        Box::new(self.clone())
    }
}

/// What lets a gated sink take what it is given.
#[derive(Default)]
struct Gate {
    open: std::sync::atomic::AtomicBool,
    waker: Mutex<Option<std::task::Waker>>,
}

impl Gate {
    fn open(&self) {
        self.open.store(true, std::sync::atomic::Ordering::SeqCst);
        if let Some(waker) = self.waker.lock().unwrap().take() {
            waker.wake();
        }
    }
}

struct Host {
    ctx: WasiCtx,
    table: ResourceTable,
}

impl WasiView for Host {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.ctx,
            table: &mut self.table,
        }
    }
}

/// Polls `future` to its end on this thread, with a waker that does nothing.
fn block_on<F: std::future::Future>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    let mut cx = Context::from_waker(std::task::Waker::noop());
    loop {
        if let Poll::Ready(value) = future.as_mut().poll(&mut cx) {
            return value;
        }
        std::thread::yield_now();
    }
}

/// A component on a host with the two outputs given, called through
/// wasmtime's async API, as `rk test` calls it: each call runs the host's
/// side of the streams, which a synchronous call does only for what the
/// host takes within it.
struct Plc {
    store: wasmtime::Store<Host>,
    instance: wasmtime::component::Instance,
}

impl Plc {
    fn load(bytes: &[u8], stdout: Sink, stderr: Sink) -> Self {
        let mut config = wasmtime::Config::new();
        config.wasm_exceptions(true);
        config.wasm_component_model_async(true);
        let engine = wasmtime::Engine::new(&config).expect("an engine");
        let component = Component::new(&engine, bytes).expect("a valid component");
        let mut linker = Linker::<Host>::new(&engine);
        wasmtime_wasi::p3::cli::add_to_linker(&mut linker).expect("WASI's command line");
        linker
            .define_unknown_imports_as_traps(&component)
            .expect("its other imports stubbed");
        let mut store = wasmtime::Store::new(
            &engine,
            Host {
                ctx: WasiCtxBuilder::new().stdout(stdout).stderr(stderr).build(),
                table: ResourceTable::new(),
            },
        );
        let instance = block_on(linker.instantiate_async(&mut store, &component))
            .expect("the component instantiates");
        let mut plc = Plc { store, instance };
        assert_eq!(plc.call::<(), (), String>("init", ()), Ok(()));
        plc
    }

    fn call<P, T: 'static, E: 'static>(&mut self, name: &str, params: P) -> Result<T, E>
    where
        P: wasmtime::component::ComponentNamedList + wasmtime::component::Lower + Send + Sync,
        (Result<T, E>,): wasmtime::component::ComponentNamedList + wasmtime::component::Lift,
    {
        let control = self
            .instance
            .get_export_index(&mut self.store, None, CONTROL)
            .expect("the control interface");
        let function = self
            .instance
            .get_export_index(&mut self.store, Some(&control), name)
            .unwrap_or_else(|| panic!("the interface has `{name}`"));
        let typed = self
            .instance
            .get_typed_func::<P, (Result<T, E>,)>(&mut self.store, &function)
            .unwrap_or_else(|e| panic!("`{name}`: {e}"));
        block_on(typed.call_async(&mut self.store, params))
            .unwrap_or_else(|e| panic!("`{name}` trapped: {e:#}"))
            .0
    }

    /// A scan with `input`, and the task's output.
    fn scan(&mut self, input: i32) -> i32 {
        let outputs = self
            .call::<_, Vec<u8>, rk::control::ScanError>(
                "run-task",
                (0u32, &input.to_le_bytes()[..]),
            )
            .expect("the scan runs");
        i32::from_le_bytes(outputs[..4].try_into().expect("four bytes"))
    }
}

fn ready(name: &'static str, written: &Written) -> Sink {
    Sink::Ready {
        name,
        at_most: usize::MAX,
        written: written.clone(),
    }
}

/// `source` built with `Std.Console` as the library, as `rk` builds it.
fn component(db: &mut db::RootDatabase, source: &str) -> rk::component::Component {
    let (mir, core) = compile_to_mir_and_wasm_as_built_with_libraries(db, STD_CONSOLE, source);
    wrapped(db, &mir, &core)
}

/// `source` built alone.
fn component_alone(db: &mut db::RootDatabase, source: &str) -> rk::component::Component {
    let (mir, core) = compile_to_mir_and_wasm_as_built(db, source);
    wrapped(db, &mir, &core)
}

fn wrapped(db: &db::RootDatabase, mir: &mir::MirModule, core: &[u8]) -> rk::component::Component {
    let interfaces = rk::component::interfaces(db, mir).expect("typed interfaces");
    rk::component::wrap(core, "plant", &interfaces).expect("the module wraps")
}

fn text(written: &Written) -> Vec<(&'static str, String)> {
    written
        .lock()
        .unwrap()
        .iter()
        .map(|(name, bytes)| (*name, String::from_utf8_lossy(bytes).into_owned()))
        .collect()
}

/// A rising edge of REQ writes the text once, and a host that is ready
/// takes it within the scan: DONE in that scan, for one scan. Each block
/// writes through its own stream, in the order written, scan after scan.
#[rstest]
fn a_scan_writes_to_the_host_s_outputs(mut with_db: db::RootDatabase) {
    let bytes = component(&mut with_db, WRITER).bytes;
    let written = Written::default();
    let mut plc = Plc::load(&bytes, ready("out", &written), ready("err", &written));
    let seen: Vec<i32> = [0, 1, 1, 3, 0, 1]
        .into_iter()
        .map(|input| plc.scan(input))
        .collect();
    assert_eq!(seen, [0, OUT_DONE, 0, ERR_DONE, 0, OUT_DONE]);
    assert_eq!(
        text(&written),
        [
            ("out", "pump started\n".to_string()),
            ("err", "pressure is high\n".to_string()),
            ("out", "pump started\n".to_string()),
        ]
    );
}

/// A host may take a text a few bytes at a time: the block hands over the
/// rest until it has it all.
#[rstest]
fn a_host_may_take_a_text_in_pieces(mut with_db: db::RootDatabase) {
    let bytes = component(&mut with_db, WRITER).bytes;
    let written = Written::default();
    let pieces = Sink::Ready {
        name: "out",
        at_most: 5,
        written: written.clone(),
    };
    let mut plc = Plc::load(&bytes, pieces, ready("err", &written));
    let seen: Vec<i32> = (0..4).map(|_| plc.scan(1)).collect();
    assert_eq!(seen, [OUT_DONE, 0, 0, 0], "all of it within the scan");
    let out: Vec<String> = text(&written)
        .into_iter()
        .filter(|(name, _)| *name == "out")
        .map(|(_, piece)| piece)
        .collect();
    assert_eq!(out, ["pump ", "start", "ed\n"]);
}

/// A scan never waits. A host that takes none of the text leaves the block
/// BUSY, scan after scan, and nothing traps: the other output goes on.
#[rstest]
fn a_host_that_is_not_ready_leaves_the_block_busy(mut with_db: db::RootDatabase) {
    let bytes = component(&mut with_db, WRITER).bytes;
    let written = Written::default();
    let mut plc = Plc::load(&bytes, Sink::Stalled, ready("err", &written));
    let seen: Vec<i32> = [1, 1, 1, 3]
        .into_iter()
        .map(|input| plc.scan(input))
        .collect();
    assert_eq!(seen, [OUT_BUSY, OUT_BUSY, OUT_BUSY, OUT_BUSY + ERR_DONE]);
    assert_eq!(text(&written), [("err", "pressure is high\n".to_string())]);
}

/// A rising edge while the block is BUSY is ignored: the text being written
/// goes once, DONE comes when the host has taken it, and the edge asked for
/// nothing more.
#[rstest]
fn an_edge_while_busy_is_ignored(mut with_db: db::RootDatabase) {
    let bytes = component(&mut with_db, WRITER).bytes;
    let (gate, written) = (Arc::new(Gate::default()), Written::default());
    let gated = Sink::Gated {
        gate: gate.clone(),
        written: written.clone(),
    };
    let mut plc = Plc::load(&bytes, gated, ready("err", &written));
    let held: Vec<i32> = [1, 0, 1].into_iter().map(|input| plc.scan(input)).collect();
    assert_eq!(
        held, [OUT_BUSY; 3],
        "the edge at the third scan, while BUSY"
    );
    gate.open();
    let taken: Vec<i32> = (0..3).map(|_| plc.scan(1)).collect();
    assert_eq!(taken, [OUT_DONE, 0, 0], "DONE once, and no second write");
    assert_eq!(text(&written), [("out", "pump started\n".to_string())]);
}

/// A host that lets go of its end makes the write an ERROR, and every
/// later request one at once: the output stays closed.
#[rstest]
fn a_host_that_lets_go_is_an_error(mut with_db: db::RootDatabase) {
    let bytes = component(&mut with_db, WRITER).bytes;
    let written = Written::default();
    let mut plc = Plc::load(&bytes, Sink::Closed, ready("err", &written));
    let seen: Vec<i32> = [1, 1, 0, 1]
        .into_iter()
        .map(|input| plc.scan(input))
        .collect();
    assert_eq!(
        seen, [OUT_ERROR; 4],
        "the ERROR stays until the next request"
    );
    assert_eq!(text(&written), Vec::<(&str, String)>::new());
}

/// `IN` holds 4096 bytes: a longer STRING is cut there, as any input is,
/// and the 4096 go to the host.
#[rstest]
fn a_text_is_cut_at_4096_bytes(mut with_db: db::RootDatabase) {
    let source = format!(
        r#"
        USING Std.Console;

        PROGRAM P
        VAR_EXTERNAL input : DINT; output : DINT; END_VAR
        VAR out : STDOUT; long : STRING[5000] := '{}'; END_VAR
            out(REQ := input = 1, IN := long);
            output := 0;
            IF out.DONE THEN output := 1; END_IF;
        END_PROGRAM

        CONFIGURATION Cfg
        VAR_GLOBAL
            input AT %ID0 : DINT;
            output AT %QD0 : DINT;
        END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM Main WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#,
        "x".repeat(4500)
    );
    let bytes = component(&mut with_db, &source).bytes;
    let written = Written::default();
    let mut plc = Plc::load(&bytes, ready("out", &written), ready("err", &written));
    assert_eq!(plc.scan(1), OUT_DONE);
    let taken: usize = written.lock().unwrap().iter().map(|(_, b)| b.len()).sum();
    assert_eq!(taken, 4096, "what IN holds");
}

/// The world imports the interface of each output, as WASI declares it,
/// when `Std.Console` is built with the program, and the WIT folder holds
/// WASI's command line package with the ones its files name. A module
/// built alone imports neither.
#[rstest]
fn the_world_imports_the_outputs_std_console_writes_to(mut with_db: db::RootDatabase) {
    let component = component(&mut with_db, WRITER);
    assert!(
        component
            .world
            .contains("    import wasi:cli/stdout@0.3.0;\n")
            && component
                .world
                .contains("    import wasi:cli/stderr@0.3.0;\n"),
        "{}",
        component.world
    );
    assert!(
        component
            .wit
            .iter()
            .any(|(path, _)| path == "deps/wasi-cli/stdio.wit")
    );
}

#[rstest]
fn a_module_that_writes_nothing_imports_no_output(mut with_db: db::RootDatabase) {
    let quiet = r#"
        {export}
        FUNCTION Twice : INT
        VAR_INPUT n : INT; END_VAR
            Twice := n * 2;
        END_FUNCTION
    "#;
    let component = component_alone(&mut with_db, quiet);
    assert!(!component.world.contains("wasi:cli"), "{}", component.world);
    assert!(
        !component
            .wit
            .iter()
            .any(|(path, _)| path.contains("wasi-cli")),
        "no command line package beside it"
    );
}

/// A task with a stream of its own: its first scan makes the stream and
/// writes 'first', and the next ones pass 'second' to the same block.
const OWN_STREAM: &str = r#"
    {extern 'wasi:cli/stdout@0.3.0' '[stream-new-0]write-via-stream'}
    FUNCTION NEW_STREAM
    VAR_OUTPUT readable : UDINT; writable : UDINT; END_VAR
    END_FUNCTION

    {extern 'wasi:cli/stdout@0.3.0' 'write-via-stream'}
    FUNCTION READS : UDINT
    VAR_INPUT data : UDINT; END_VAR
    END_FUNCTION

    {extern 'wasi:cli/stdout@0.3.0' '[future-drop-readable-1]write-via-stream'}
    FUNCTION DROP_FUTURE
    VAR_INPUT future : UDINT; END_VAR
    END_FUNCTION

    {extern async 'wasi:cli/stdout@0.3.0' '[stream-write-0]write-via-stream'}
    FUNCTION_BLOCK STREAM_WRITE
    VAR_INPUT stream : UDINT; data : STRING[16]; END_VAR
    VAR_OUTPUT DONE : BOOL; written : UDINT; closed : BOOL; END_VAR
    END_FUNCTION_BLOCK

    PROGRAM P
    VAR_EXTERNAL output : DINT; END_VAR
    VAR open : BOOL; reading : UDINT; writing : UDINT; write : STREAM_WRITE; END_VAR
        IF NOT open THEN
            NEW_STREAM(readable => reading, writable => writing);
            DROP_FUTURE(future := READS(data := reading));
            open := TRUE;
            write(stream := writing, data := 'first');
        ELSE
            write(stream := writing, data := 'second');
        END_IF;
        IF write.DONE THEN
            output := output + 1;
        END_IF;
    END_PROGRAM

    CONFIGURATION Cfg
    VAR_GLOBAL output AT %QD0 : DINT; END_VAR
        RESOURCE Res ON CPU
            TASK T(INTERVAL := T#10ms, PRIORITY := 1);
            PROGRAM Main WITH T : P;
        END_RESOURCE
    END_CONFIGURATION
"#;

/// A new stream's two ends come back as the FUNCTION's two outputs, and a
/// stream write copies the bytes it started with: while the host holds the
/// copy, the calls of the block pass another text, which they leave in
/// their arguments, and the host reads the first.
#[rstest]
fn a_stream_write_copies_the_bytes_it_started_with(mut with_db: db::RootDatabase) {
    let bytes = component_alone(&mut with_db, OWN_STREAM).bytes;
    let gate = Arc::new(Gate::default());
    let written = Written::default();
    let out = Sink::Gated {
        gate: gate.clone(),
        written: written.clone(),
    };
    let mut plc = Plc::load(&bytes, out, ready("err", &written));
    let held: Vec<i32> = (0..3).map(|_| plc.scan(0)).collect();
    assert_eq!(held, [0, 0, 0], "the host holds the copy");
    gate.open();
    for _ in 0..3 {
        plc.scan(0);
    }
    let texts = text(&written);
    assert_eq!(
        texts.first(),
        Some(&("out", "first".to_string())),
        "{texts:?}"
    );
}
