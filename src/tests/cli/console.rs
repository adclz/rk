// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! The standard output and error of a component: WASI 0.3's, a stream of
//! bytes each, as wasmtime-wasi gives them to a host. The module writes
//! with `{wasm 'rk.stdout_write'}` and `{wasm 'rk.stderr_write'}`, which
//! `Std.Console` is written with.

use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use rstest::rstest;
use wasmtime::component::{Component, Linker};
use wasmtime_wasi::cli::{IsTerminal, StdoutStream};
use wasmtime_wasi::{ResourceTable, WasiCtx, WasiCtxBuilder, WasiCtxView, WasiView};

use crate::tests::codegen::{compile_to_mir_and_wasm_as_built, with_db};

const CONTROL: &str = debug_format::wit::CONTROL_INTERFACE;

/// The two writes, as `Std.Console` declares them, and a task that makes
/// both: its output says which the host took, 1 for the standard output
/// and 2 for the standard error.
const WRITER: &str = r#"
    FUNCTION Say : BOOL
    VAR_INPUT IN : STRING[4096]; END_VAR
        {wasm 'rk.stdout_write' (params IN) (result Say)}
    END_FUNCTION

    FUNCTION Warn : BOOL
    VAR_INPUT IN : STRING[4096]; END_VAR
        {wasm 'rk.stderr_write' (params IN) (result Warn)}
    END_FUNCTION

    PROGRAM P
    VAR_EXTERNAL input : DINT; output : DINT; END_VAR
        output := 0;
        IF Say('scan started$N') THEN
            output := 1;
        END_IF;
        IF Warn('pressure is high$N') THEN
            output := output + 2;
        END_IF;
        IF input = 7 THEN
            IF Say('seven$N') THEN
                output := output + 4;
            END_IF;
        END_IF;
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
    /// Lets go of its end at the first write.
    Closed,
}

impl tokio::io::AsyncWrite for Sink {
    fn poll_write(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
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

/// A component on a host with the two outputs given, called as `rk test`
/// and a plain host call it: synchronously.
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
        let instance = linker
            .instantiate(&mut store, &component)
            .expect("the component instantiates");
        let mut plc = Plc { store, instance };
        assert_eq!(plc.call::<(), (), String>("init", ()), Ok(()));
        plc
    }

    fn call<P, T, E>(&mut self, name: &str, params: P) -> Result<T, E>
    where
        P: wasmtime::component::ComponentNamedList + wasmtime::component::Lower,
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
        self.instance
            .get_typed_func::<P, (Result<T, E>,)>(&mut self.store, &function)
            .unwrap_or_else(|e| panic!("`{name}`: {e}"))
            .call(&mut self.store, params)
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

fn component(db: &mut db::RootDatabase, source: &str) -> rk::component::Component {
    let (mir, core) = compile_to_mir_and_wasm_as_built(db, source);
    let interfaces = rk::component::interfaces(db, &mir).expect("typed interfaces");
    rk::component::wrap(&core, "plant", &interfaces).expect("the module wraps")
}

fn text(written: &Written) -> Vec<(&'static str, String)> {
    written
        .lock()
        .unwrap()
        .iter()
        .map(|(name, bytes)| (*name, String::from_utf8_lossy(bytes).into_owned()))
        .collect()
}

/// A host that is ready takes each text within the call, in the order
/// written, scan after scan through the one stream of each output.
#[rstest]
fn a_scan_writes_to_the_host_s_outputs(mut with_db: db::RootDatabase) {
    let bytes = component(&mut with_db, WRITER).bytes;
    let written = Written::default();
    let mut plc = Plc::load(&bytes, ready("out", &written), ready("err", &written));
    assert_eq!(plc.scan(0), 3, "both writes taken");
    assert_eq!(plc.scan(7), 7, "and a third");
    assert_eq!(
        text(&written),
        [
            ("out", "scan started\n".to_string()),
            ("err", "pressure is high\n".to_string()),
            ("out", "scan started\n".to_string()),
            ("err", "pressure is high\n".to_string()),
            ("out", "seven\n".to_string()),
        ]
    );
}

/// A host may take a text a few bytes at a time: the write hands over the
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
    assert_eq!(plc.scan(0), 3, "taken, in pieces");
    let out: Vec<String> = text(&written)
        .into_iter()
        .filter(|(name, _)| *name == "out")
        .map(|(_, piece)| piece)
        .collect();
    assert_eq!(out, ["scan ", "start", "ed\n"]);
}

/// A scan never waits. A host that takes none of the text leaves the
/// output closed: the write answers FALSE, and so does each later one,
/// while the other output goes on. Nothing traps, so the scans run.
#[rstest]
fn a_host_that_is_not_ready_closes_the_output(mut with_db: db::RootDatabase) {
    let bytes = component(&mut with_db, WRITER).bytes;
    let written = Written::default();
    let mut plc = Plc::load(&bytes, Sink::Stalled, ready("err", &written));
    for scan in 0..3 {
        assert_eq!(plc.scan(0), 2, "scan {scan}: only the standard error took");
    }
    assert_eq!(
        text(&written),
        [
            ("err", "pressure is high\n".to_string()),
            ("err", "pressure is high\n".to_string()),
            ("err", "pressure is high\n".to_string()),
        ]
    );
}

/// A host that lets go of its end closes the output as well.
#[rstest]
fn a_host_that_lets_go_closes_the_output(mut with_db: db::RootDatabase) {
    let bytes = component(&mut with_db, WRITER).bytes;
    let written = Written::default();
    let mut plc = Plc::load(&bytes, ready("out", &written), Sink::Closed);
    for scan in 0..3 {
        assert_eq!(plc.scan(0), 1, "scan {scan}: only the standard output took");
    }
    assert_eq!(text(&written).len(), 3);
}

/// A text longer than the buffer is cut to it, and the write answers
/// FALSE: `Std.Console` takes a STRING of that capacity, which a pragma
/// written by hand may exceed.
#[rstest]
fn a_text_longer_than_the_buffer_is_cut(mut with_db: db::RootDatabase) {
    let source = format!(
        r#"
        FUNCTION Say : BOOL
        VAR_INPUT IN : STRING[5000]; END_VAR
            {{wasm 'rk.stdout_write' (params IN) (result Say)}}
        END_FUNCTION

        PROGRAM P
        VAR_EXTERNAL input : DINT; output : DINT; END_VAR
        VAR long : STRING[5000] := '{}'; END_VAR
            output := 0;
            IF Say(long) THEN
                output := 1;
            END_IF;
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
    assert_eq!(plc.scan(0), 0, "not all of it");
    let taken: usize = written.lock().unwrap().iter().map(|(_, b)| b.len()).sum();
    assert_eq!(taken, 4096, "what the buffer holds");
}

/// The world imports the interface of each output the module writes to, as
/// WASI declares it, and the WIT folder holds WASI's command line package
/// with the ones its files name. A module that writes to none imports
/// neither.
#[rstest]
fn the_world_imports_the_outputs_the_module_writes_to(mut with_db: db::RootDatabase) {
    let only_stdout = r#"
        FUNCTION Say : BOOL
        VAR_INPUT IN : STRING[4096]; END_VAR
            {wasm 'rk.stdout_write' (params IN) (result Say)}
        END_FUNCTION

        {export}
        FUNCTION Greet : BOOL
            Greet := Say('hello$N');
        END_FUNCTION
    "#;
    let component = component(&mut with_db, only_stdout);
    assert!(
        component
            .world
            .contains("    import wasi:cli/stdout@0.3.0;\n")
            && !component.world.contains("stderr"),
        "{}",
        component.world
    );
    let packages: std::collections::BTreeSet<&str> = component
        .wit
        .iter()
        .filter_map(|(path, _)| path.strip_prefix("deps/")?.split('/').next())
        .collect();
    assert_eq!(
        packages.into_iter().collect::<Vec<_>>(),
        [
            "rk-plc",
            "wasi-cli",
            "wasi-clocks",
            "wasi-filesystem",
            "wasi-random",
            "wasi-sockets"
        ]
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
    let component = component(&mut with_db, quiet);
    assert!(!component.world.contains("wasi:cli"), "{}", component.world);
    assert!(
        !component
            .wit
            .iter()
            .any(|(path, _)| path.contains("wasi-cli")),
        "no command line package beside it"
    );
}
