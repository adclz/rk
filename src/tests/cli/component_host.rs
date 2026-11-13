// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! A component on a wasmtime host, called through the async API as `rk test`
//! calls one, with the WASI context a test gives it: its standard input,
//! output and error. Each call runs the host's side of the streams, which a
//! synchronous call does only for what the host takes within it.

use std::sync::Mutex;
use std::task::{Context, Poll};

use wasmtime::component::{Component, Linker};
use wasmtime_wasi::{ResourceTable, WasiCtx, WasiCtxView, WasiView};

const CONTROL: &str = debug_format::wit::CONTROL_INTERFACE;

/// What lets a gated stream move: closed until a test opens it, it wakes
/// the host's side of the stream then.
#[derive(Default)]
pub(crate) struct Gate {
    pub(crate) open: std::sync::atomic::AtomicBool,
    pub(crate) waker: Mutex<Option<std::task::Waker>>,
}

impl Gate {
    pub(crate) fn open(&self) {
        self.open.store(true, std::sync::atomic::Ordering::SeqCst);
        if let Some(waker) = self.waker.lock().unwrap().take() {
            waker.wake();
        }
    }

    /// Whether the gate is open; when it is not, `cx`'s waker is kept, to
    /// wake when it opens.
    pub(crate) fn passes(&self, cx: &Context<'_>) -> bool {
        if self.open.load(std::sync::atomic::Ordering::SeqCst) {
            return true;
        }
        *self.waker.lock().unwrap() = Some(cx.waker().clone());
        false
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

/// A component on a host with WASI's command line, `ctx` giving its
/// streams, called through wasmtime's async API.
pub(crate) struct Plc {
    store: wasmtime::Store<Host>,
    instance: wasmtime::component::Instance,
}

impl Plc {
    pub(crate) fn load(bytes: &[u8], ctx: WasiCtx) -> Self {
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
                ctx,
                table: ResourceTable::new(),
            },
        );
        let instance = block_on(linker.instantiate_async(&mut store, &component))
            .expect("the component instantiates");
        let mut plc = Plc { store, instance };
        assert_eq!(plc.call::<(), (), String>("init", ()), Ok(()));
        plc
    }

    pub(crate) fn call<P, T: 'static, E: 'static>(&mut self, name: &str, params: P) -> Result<T, E>
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
    pub(crate) fn scan(&mut self, input: i32) -> i32 {
        let outputs = self
            .call::<_, Vec<u8>, rk::control::ScanError>(
                "run-task",
                (0u32, &input.to_le_bytes()[..]),
            )
            .expect("the scan runs");
        i32::from_le_bytes(outputs[..4].try_into().expect("four bytes"))
    }
}
