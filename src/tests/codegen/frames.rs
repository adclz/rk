// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! The locals of a recursive call, read in the frame of each call.
//!
//! A recursive function's arrays and structs are in a frame each call
//! pushes, whose start a wasm local holds. A debugger stopped in one reads
//! that local and places the locals from it: here with wasmtime's debug
//! API, stopped at a raise four calls deep, as a debug adapter would be.
//!
//! An `ARRAY[*]` parameter is read the same way, stopped in the copy of
//! the function a call runs: its bounds are that copy's.

use crate::tests::codegen::{compile_to_wasm, with_db};
use debug_format::{DebugInfo, Live, Place, VarValue};
use rstest::rstest;
use std::sync::Arc;
use wasmtime::{DebugEvent, DebugHandler, Memory, StoreContextMut};

/// What the handler records: per `Depth` frame, innermost first, its
/// `trail[0]`, `pt.x` and `link^.x`.
#[derive(Default)]
struct Captured {
    memory: Option<Memory>,
    frames: Vec<(VarValue, VarValue, VarValue)>,
    /// What the second frame's `link` holds.
    link_of_second: Option<Link>,
    /// Per frame with an `ARRAY[*]` parameter `values`, innermost first.
    copies: Vec<Copy>,
}

/// What `values` reads as in the frame of one copy: the copy's name, the
/// bounds the frame's table gives the array, and its elements -2, 0 and 2.
#[derive(Debug, PartialEq)]
struct Copy {
    function: String,
    bounds: Vec<(i64, i64)>,
    elements: [Option<VarValue>; 3],
}

/// The address a `link` holds, whether a static table names it, the
/// stack's region, and the live frames' name for it.
#[derive(Debug, PartialEq)]
struct Link {
    held: u32,
    named: Option<String>,
    stack: Option<(u32, u32)>,
    in_frame: Option<(usize, String)>,
}

#[derive(Clone)]
struct AtRaise(Arc<DebugInfo>);

/// A frame's wasm locals and the memory, for the reader.
struct Frame<'a> {
    memory: &'a [u8],
    locals: Vec<(u32, u32)>,
}

impl Live for Frame<'_> {
    fn read(&mut self, address: u32, size: u32) -> Option<Vec<u8>> {
        self.memory
            .get(address as usize..(address as usize).checked_add(size as usize)?)
            .map(<[u8]>::to_vec)
    }

    fn local(&mut self, index: u32) -> Option<u32> {
        self.locals
            .iter()
            .find(|(i, _)| *i == index)
            .map(|(_, v)| *v)
    }
}

impl DebugHandler for AtRaise {
    type Data = Captured;

    async fn handle(&self, mut store: StoreContextMut<'_, Captured>, event: DebugEvent<'_>) {
        if !matches!(event, DebugEvent::UncaughtExceptionThrown(_)) {
            return;
        }
        let info = &self.0;
        // The frames first, with the store borrowed by each handle; then the
        // memory, borrowed whole.
        let mut depth_frames = Vec::new();
        let exits: Vec<_> = store.debug_exit_frames().collect();
        for exit in exits {
            let mut frame = Some(exit);
            while let Some(handle) = frame {
                if let Ok(Some((index, _))) = handle.wasm_function_index_and_pc(&mut store)
                    && info.function_name(index.as_u32()) == Some("Depth")
                {
                    let table = info.frame_locals(index.as_u32()).expect("Depth's table");
                    let slot = table.frame.as_ref().expect("Depth's frame").slot;
                    let mut locals = Vec::new();
                    for i in 0..handle.num_locals(&mut store).unwrap_or(0) {
                        if let Ok(wasmtime::Val::I32(v)) = handle.local(&mut store, i) {
                            locals.push((i, v as u32));
                        }
                    }
                    let base = locals
                        .iter()
                        .find(|(i, _)| *i == slot)
                        .map(|(_, v)| *v)
                        .expect("the frame's start");
                    depth_frames.push((index.as_u32(), base, locals));
                }
                frame = handle.parent(&mut store).ok().flatten();
            }
        }
        let memory = store.data().memory.expect("the memory");
        let bytes = memory.data(&store).to_vec();
        // The second call's `link`, a `REF_TO` in a wasm local, holds an
        // address in its own frame: no static table names it, the live
        // frames do.
        let live_frames: Vec<(u32, u32)> = depth_frames
            .iter()
            .map(|(i, base, _)| (*i, *base))
            .collect();
        let (index, _, locals) = &depth_frames[1];
        let link = info
            .frame_locals(*index)
            .and_then(|t| t.local_pointers.iter().find(|p| p.name == "link"));
        let held = link
            .and_then(|p| locals.iter().find(|(i, _)| *i == p.wasm_index))
            .map(|(_, v)| *v);
        // What `link` points at is a Pt: named as the STRUCT, not its first
        // field, which shares its address.
        let target = link
            .and_then(|p| info.local_type(p.target?))
            .and_then(|t| match t {
                debug_format::TypeDesc::Struct { name, .. } => Some(name.clone()),
                _ => None,
            });
        store.data_mut().link_of_second = held.map(|held| Link {
            held,
            named: info.variable_at(held),
            stack: info.stack(),
            in_frame: info.frame_variable_at(held, &live_frames, target.as_deref()),
        });
        let mut frames = Vec::new();
        for (index, base, locals) in depth_frames {
            let live = &mut Frame {
                memory: &bytes,
                locals,
            };
            let read = |place: Option<Place>, live: &mut Frame| match place {
                Some(Place::Value(loc)) => loc.decode(&live.read(loc.address, loc.size).unwrap()),
                other => panic!("not a value: {other:?}"),
            };
            let trail = info
                .resolve_in_frame(index, "trail[0]", base)
                .expect("trail[0]");
            let trail = trail.decode(&live.read(trail.address, trail.size).unwrap());
            let x = read(info.follow_local(index, "pt.x", Some(base), live), live);
            let through = read(info.follow_local(index, "link^.x", Some(base), live), live);
            frames.push((trail, x, through));
        }
        store.data_mut().frames = frames;
    }
}

/// Stopped at a raise, reads `values` in every frame that has one.
#[derive(Clone)]
struct AtRaiseInACopy(Arc<DebugInfo>);

impl DebugHandler for AtRaiseInACopy {
    type Data = Captured;

    async fn handle(&self, mut store: StoreContextMut<'_, Captured>, event: DebugEvent<'_>) {
        if !matches!(event, DebugEvent::UncaughtExceptionThrown(_)) {
            return;
        }
        let info = &self.0;
        let mut frames = Vec::new();
        let exits: Vec<_> = store.debug_exit_frames().collect();
        for exit in exits {
            let mut frame = Some(exit);
            while let Some(handle) = frame {
                if let Ok(Some((index, _))) = handle.wasm_function_index_and_pc(&mut store) {
                    let index = index.as_u32();
                    let takes_values = info
                        .frame_locals(index)
                        .is_some_and(|t| t.local_pointers.iter().any(|p| p.name == "values"));
                    if takes_values {
                        let mut locals = Vec::new();
                        for i in 0..handle.num_locals(&mut store).unwrap_or(0) {
                            if let Ok(wasmtime::Val::I32(v)) = handle.local(&mut store, i) {
                                locals.push((i, v as u32));
                            }
                        }
                        frames.push((index, locals));
                    }
                }
                frame = handle.parent(&mut store).ok().flatten();
            }
        }
        let memory = store.data().memory.expect("the memory");
        let bytes = memory.data(&store).to_vec();
        let mut copies = Vec::new();
        for (index, locals) in frames {
            let live = &mut Frame {
                memory: &bytes,
                locals,
            };
            // The array itself: its place carries the type the copy gives
            // it, where a debugger reads the bounds to list the elements.
            let bounds = match info.follow_local(index, "values", None, live) {
                Some(Place::Aggregate { ty, .. }) => match info.local_type(ty) {
                    Some(debug_format::TypeDesc::Array { dimensions, .. }) => dimensions.clone(),
                    other => panic!("not an array: {other:?}"),
                },
                other => panic!("not an aggregate: {other:?}"),
            };
            let elements = [-2, 0, 2].map(|at| {
                match info.follow_local(index, &format!("values[{at}]"), None, live) {
                    Some(Place::Value(loc)) => {
                        Some(loc.decode(&live.read(loc.address, loc.size).unwrap()))
                    }
                    None => None,
                    other => panic!("not a value: {other:?}"),
                }
            });
            copies.push(Copy {
                function: info.function_name(index).unwrap_or_default().to_string(),
                bounds,
                elements,
            });
        }
        store.data_mut().copies = copies;
    }
}

/// A store with the memory `wasm` imports and `handler` called on its debug
/// events, a linker with that memory defined, and the module.
fn debugged(
    wasm: &[u8],
    handler: impl DebugHandler<Data = Captured>,
) -> (
    wasmtime::Store<Captured>,
    wasmtime::Linker<Captured>,
    wasmtime::Module,
) {
    let mut config = wasmtime::Config::new();
    config.wasm_exceptions(true);
    config.guest_debug(true);
    let engine = wasmtime::Engine::new(&config).expect("engine");
    let module = wasmtime::Module::new(&engine, wasm).expect("module");
    let mut store = wasmtime::Store::new(&engine, Captured::default());
    store.set_debug_handler(handler);
    let min_pages = module
        .imports()
        .find_map(|i| match i.ty() {
            wasmtime::ExternType::Memory(mt) => Some(mt.minimum()),
            _ => None,
        })
        .expect("an imported memory");
    let memory = Memory::new(
        &mut store,
        wasmtime::MemoryType::new(min_pages as u32, None),
    )
    .expect("memory");
    store.data_mut().memory = Some(memory);
    let mut linker = wasmtime::Linker::new(&engine);
    linker
        .define(&store, "env", "memory", memory)
        .expect("define");
    linker
        .define_unknown_imports_as_traps(&module)
        .expect("stubs");
    (store, linker, module)
}

/// Drive a future that never waits on anything outside itself.
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

/// Four calls of `Depth`, each with its own `trail`, `pt` and `link`, the
/// innermost raising: the values read in each frame are that call's, where
/// a static address would have shown the innermost call's in every frame.
#[rstest]
fn a_recursive_call_s_locals_are_read_in_its_frame(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Pt : STRUCT x : REAL; y : REAL; END_STRUCT; END_TYPE

        FUNCTION Depth : INT
        VAR_INPUT n : INT; END_VAR
        VAR
            trail : ARRAY[0..3] OF INT;
            pt : Pt;
            link : REF_TO Pt;
        END_VAR
            trail[0] := n;
            pt.x := n;
            link := REF(pt);
            IF n > 0 THEN
                Depth := Depth(n - 1) + 1;
            ELSE
                __RAISE('bottom');
            END_IF;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let info = Arc::new(DebugInfo::from_wasm(&wasm));
    assert!(info.problems().is_empty(), "{:?}", info.problems());

    let (mut store, linker, module) = debugged(&wasm, AtRaise(info.clone()));
    let frames = block_on(async {
        let instance = linker
            .instantiate_async(&mut store, &module)
            .await
            .expect("instantiate");
        let depth = instance
            .get_typed_func::<i32, i32>(&mut store, "Depth")
            .expect("Depth is exported");
        depth
            .call_async(&mut store, 3)
            .await
            .expect_err("the innermost call raises");
        let link = store
            .data_mut()
            .link_of_second
            .take()
            .expect("the second frame's link was read");
        assert_eq!(link.named, None, "no static name on the stack");
        let (base, end) = link.stack.expect("the stack's region");
        let held = link.held;
        assert!(
            held >= base && held < end,
            "{held} is on the stack [{base}, {end})"
        );
        assert_eq!(
            link.in_frame,
            Some((1, "pt".to_string())),
            "the second frame's own pt, the STRUCT"
        );
        std::mem::take(&mut store.data_mut().frames)
    });

    let expected: Vec<_> = (0..4)
        .map(|n| {
            (
                VarValue::I16(n),
                VarValue::F32(n as f32),
                VarValue::F32(n as f32),
            )
        })
        .collect();
    assert_eq!(frames, expected, "innermost first, each call's own");
}

/// An `ARRAY[*]` parameter is a pointer in a wasm local, and each call runs
/// the copy of the function compiled for the array it binds: the frame's
/// table gives the pointer that array's type. Stopped two copies deep, each
/// frame reads its own array within its own bounds, `Outer`'s copy the
/// caller's `[0..2]` and `Last`'s the `[-2..2]` it was handed.
#[rstest]
fn a_conformand_parameter_is_read_with_the_bounds_of_its_copy(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION Last : INT
        VAR_IN_OUT values : ARRAY[*] OF INT; END_VAR
            Last := values[0];
            __RAISE('here');
        END_FUNCTION

        FUNCTION Outer : INT
        VAR_IN_OUT values : ARRAY[*] OF INT; END_VAR
        VAR around : ARRAY[-2..2] OF INT := [100, 200, 300, 400, 500]; END_VAR
            Outer := Last(around) + values[0];
        END_FUNCTION

        FUNCTION run : INT
        VAR three : ARRAY[0..2] OF INT := [1, 2, 3]; END_VAR
            run := Outer(three);
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let info = Arc::new(DebugInfo::from_wasm(&wasm));
    assert!(info.problems().is_empty(), "{:?}", info.problems());

    let (mut store, linker, module) = debugged(&wasm, AtRaiseInACopy(info.clone()));
    let copies = block_on(async {
        let instance = linker
            .instantiate_async(&mut store, &module)
            .await
            .expect("instantiate");
        let run = instance
            .get_typed_func::<(), i32>(&mut store, "run")
            .expect("run is exported");
        run.call_async(&mut store, ())
            .await
            .expect_err("the innermost call raises");
        std::mem::take(&mut store.data_mut().copies)
    });

    let int = |v: i16| Some(VarValue::I16(v));
    assert_eq!(
        copies,
        [
            Copy {
                function: "Last$[-2..2]".to_string(),
                bounds: vec![(-2, 2)],
                elements: [int(100), int(300), int(500)],
            },
            Copy {
                function: "Outer$[0..2]".to_string(),
                bounds: vec![(0, 2)],
                elements: [None, int(1), int(3)],
            },
        ],
        "innermost first"
    );
}
