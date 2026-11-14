// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! The body of an async import's function block ([`mir::async_call`]),
//! `(this)`, which never waits: each call takes the block's call one step.
//!
//! - A call running: the block's waitable set is polled. The call returned:
//!   the result is copied from where the host wrote it into the
//!   VAR_OUTPUTs, the call is let go, and DONE is TRUE. Otherwise DONE is
//!   FALSE.
//! - None running: the call is made with the VAR_INPUTs, lowered async, so
//!   it returns at once. Returned already, the result is copied and DONE is
//!   TRUE; otherwise the call joins the set and DONE is FALSE.
//!
//! A stream's write or read, `[stream-write-N]f` and `[stream-read-N]f`, and
//! a future's read, `[future-read-N]f`, are made the same way, on the end
//! they copy on instead of a subtask: the copy is started when none is
//! pending and checked otherwise.
//!
//! A warm start restores no call: its handle went with the instance that
//! made it, and is kept apart from the variables.

use hir::hir_ty::async_import::AsyncKind;
use mir::async_call::{
    Builtin, MirAsyncCall, STATE_EVENT, STATE_HANDLE, STATE_SET, builtin_at, import_at,
};
use mir::function::{MirFunction, MirLinkage};
use mir::types::{MirElementary, MirType};
use wasm_encoder::{BlockType, Function, Instruction as I, ValType};

use crate::WasmGen;
use crate::canonical::{Canonical, canonical, record_layout, store};
use crate::emit_expr::{emit_typed_mem_load, mem_arg};
use crate::emit_stmt::emit_typed_mem_store_pub as emit_typed_mem_store;

/// A subtask's event, and the state that ends it.
const EVENT_SUBTASK: i32 = 1;
const RETURNED: i32 = 2;

/// A copy's events, what a copy returns when it has not ended yet, and the
/// result of a copy whose other end left.
const EVENT_STREAM_READ: i32 = 2;
const EVENT_STREAM_WRITE: i32 = 3;
const EVENT_FUTURE_READ: i32 = 4;
const BLOCKED: i32 = -1;
const DROPPED: i32 = 1;

const THIS: u32 = 0;
const STATUS: u32 = 1;
/// The element a stream read spreads its next byte into, in an ARRAY OF
/// BYTE.
const INDEX: u32 = 2;

const BOOL: MirType = MirType::Elementary(MirElementary::Bool);

impl WasmGen<'_> {
    /// Emits `func`, the body of the block that makes `call`.
    pub(crate) fn emit_async_body(&mut self, func: &MirFunction, call: &MirAsyncCall) {
        if call.kind.copies() {
            let f = self.copy_body(call);
            return self.place_body(func, &f);
        }
        let builtin = |builtin| {
            builtin_at(self.module, builtin).expect(
                "internal compiler error: a module that makes an async call imports the built-ins",
            )
        };
        let import = import_at(self.module, call)
            .expect("internal compiler error: an async call's import is among the imports");
        let check = |name: &str| {
            self.builtin_indices
                .get(name)
                .copied()
                .unwrap_or_else(|| panic!("internal compiler error: `{name}` was not grafted"))
        };
        let mut f = Function::new([(1, ValType::I32)]);
        store_bool(&mut f, call.done, false);

        // A call running: did it return?
        word(&mut f, call.state + STATE_HANDLE);
        f.instruction(&I::If(BlockType::Empty));
        word(&mut f, call.state + STATE_SET);
        address(&mut f, call.state + STATE_EVENT);
        f.instruction(&I::Call(builtin(Builtin::SetPoll)));
        f.instruction(&I::I32Const(EVENT_SUBTASK));
        f.instruction(&I::I32Eq);
        word(&mut f, call.state + STATE_EVENT);
        word(&mut f, call.state + STATE_HANDLE);
        f.instruction(&I::I32Eq);
        f.instruction(&I::I32And);
        word(&mut f, call.state + STATE_EVENT + 4);
        f.instruction(&I::I32Const(RETURNED));
        f.instruction(&I::I32Eq);
        f.instruction(&I::I32And);
        f.instruction(&I::If(BlockType::Empty));
        copy_result(&mut f, call);
        word(&mut f, call.state + STATE_HANDLE);
        f.instruction(&I::Call(builtin(Builtin::SubtaskDrop)));
        set_word(&mut f, call.state + STATE_HANDLE, |f| {
            f.instruction(&I::I32Const(0));
        });
        store_bool(&mut f, call.done, true);
        f.instruction(&I::End);

        // None running: the call, made now.
        f.instruction(&I::Else);
        // What the component model would trap on raises first.
        for (offset, ty) in &call.params {
            match ty {
                MirType::String { .. } => {
                    address(&mut f, offset + 4);
                    word(&mut f, *offset);
                    f.instruction(&I::Call(check("rk.utf8_check")));
                }
                MirType::Elementary(MirElementary::Char) => {
                    load(&mut f, *offset, ty);
                    f.instruction(&I::Call(check("rk.char_check")));
                }
                _ => {}
            }
        }
        lower_params(&mut f, call);
        // The result's area, cleared: a case the host writes in fewer bytes
        // than its slot, as an enum's in a `result`, reads whole.
        if let Some(area) = call.result_area {
            for at in 0..call.results.len() as u64 {
                f.instruction(&I::LocalGet(THIS));
                f.instruction(&I::I64Const(0));
                f.instruction(&I::I64Store(mem_arg(u64::from(area) + 8 * at, 3)));
            }
            address(&mut f, area);
        }
        f.instruction(&I::Call(import));
        f.instruction(&I::LocalSet(STATUS));
        f.instruction(&I::LocalGet(STATUS));
        f.instruction(&I::I32Const(0xf));
        f.instruction(&I::I32And);
        f.instruction(&I::I32Const(RETURNED));
        f.instruction(&I::I32Eq);
        f.instruction(&I::If(BlockType::Empty));
        copy_result(&mut f, call);
        store_bool(&mut f, call.done, true);
        f.instruction(&I::Else);
        set_word(&mut f, call.state + STATE_HANDLE, |f| {
            f.instruction(&I::LocalGet(STATUS));
            f.instruction(&I::I32Const(4));
            f.instruction(&I::I32ShrU);
        });
        // The set, made at the block's first call.
        word(&mut f, call.state + STATE_SET);
        f.instruction(&I::I32Eqz);
        f.instruction(&I::If(BlockType::Empty));
        set_word(&mut f, call.state + STATE_SET, |f| {
            f.instruction(&I::Call(builtin(Builtin::SetNew)));
        });
        f.instruction(&I::End);
        word(&mut f, call.state + STATE_HANDLE);
        word(&mut f, call.state + STATE_SET);
        f.instruction(&I::Call(builtin(Builtin::Join)));
        f.instruction(&I::End);
        f.instruction(&I::End);
        f.instruction(&I::End);
        self.place_body(func, &f);
    }

    /// The body of the block that copies into or out of a stream or a
    /// future, `call`: it starts the copy when none is pending and checks it
    /// otherwise, on the end it copies on, which a copy that did not end at
    /// once joins the block's set as a subtask does. A write passes the
    /// bytes as they are, unchecked: a stream carries bytes, not text.
    fn copy_body(&self, call: &MirAsyncCall) -> Function {
        let builtin = |builtin| {
            builtin_at(self.module, builtin).expect(
                "internal compiler error: a module that makes an async call imports the built-ins",
            )
        };
        let import = import_at(self.module, call)
            .expect("internal compiler error: a copy's import is among the imports");
        let (end, _) = call
            .params
            .first()
            .cloned()
            .expect("internal compiler error: a copy takes the end it copies on first (E1516)");
        let mut f = Function::new([(2, ValType::I32)]);
        store_bool(&mut f, call.done, false);

        // A copy pending: did it end?
        word(&mut f, call.state + STATE_HANDLE);
        f.instruction(&I::If(BlockType::Empty));
        copy_event(&mut f, call, builtin(Builtin::SetPoll));
        f.instruction(&I::If(BlockType::Empty));
        copy_ended(&mut f, call);
        f.instruction(&I::End);

        // None pending: the copy, started now.
        f.instruction(&I::Else);
        start_copy(&mut f, call);
        f.instruction(&I::Call(import));
        f.instruction(&I::LocalTee(STATUS));
        f.instruction(&I::I32Const(BLOCKED));
        f.instruction(&I::I32Ne);
        f.instruction(&I::If(BlockType::Empty));
        copied(&mut f, call, |f| {
            f.instruction(&I::LocalGet(STATUS));
        });
        f.instruction(&I::Else);
        // Pending: the stream's end is what the block checks.
        set_word(&mut f, call.state + STATE_HANDLE, |f| {
            word(f, end);
        });
        word(&mut f, call.state + STATE_SET);
        f.instruction(&I::I32Eqz);
        f.instruction(&I::If(BlockType::Empty));
        set_word(&mut f, call.state + STATE_SET, |f| {
            f.instruction(&I::Call(builtin(Builtin::SetNew)));
        });
        f.instruction(&I::End);
        word(&mut f, call.state + STATE_HANDLE);
        word(&mut f, call.state + STATE_SET);
        f.instruction(&I::Call(builtin(Builtin::Join)));
        f.instruction(&I::End);
        f.instruction(&I::End);
        f.instruction(&I::End);
        f
    }

    /// Places `f`, the body of `func`, and exports it when `func` is.
    fn place_body(&mut self, func: &MirFunction, f: &Function) {
        let ty = self.add_type(&[ValType::I32], &[]);
        self.fn_section.function(ty);
        self.code_section.function(f);
        if func.linkage == MirLinkage::Export {
            let index = self
                .index_remap
                .get(&func.index)
                .copied()
                .unwrap_or(func.index);
            self.export_section.export(
                func.name.text(self.db),
                wasm_encoder::ExportKind::Func,
                index,
            );
        }
    }
}

/// Whether the set's next event, from `waitable-set.poll`, is the end of
/// the block's copy. The set holds the end it copies on alone.
fn copy_event(f: &mut Function, call: &MirAsyncCall, poll: u32) {
    let event = match call.kind {
        AsyncKind::StreamRead => EVENT_STREAM_READ,
        AsyncKind::StreamWrite => EVENT_STREAM_WRITE,
        AsyncKind::FutureRead => EVENT_FUTURE_READ,
        AsyncKind::Call => unreachable!("internal compiler error: a call is no copy"),
    };
    word(f, call.state + STATE_SET);
    address(f, call.state + STATE_EVENT);
    f.instruction(&I::Call(poll));
    f.instruction(&I::I32Const(event));
    f.instruction(&I::I32Eq);
    word(f, call.state + STATE_EVENT);
    word(f, call.state + STATE_HANDLE);
    f.instruction(&I::I32Eq);
    f.instruction(&I::I32And);
}

/// Pushes what the copy's built-in takes: a write's end and bytes; a stream
/// read's end, where the bytes go and how many fit; a future read's end and
/// where the value goes, that area cleared first, as a call's result's is.
fn start_copy(f: &mut Function, call: &MirAsyncCall) {
    let end = |f: &mut Function| {
        let (end, _) = call.params[0];
        word(f, end);
    };
    match call.kind {
        AsyncKind::StreamWrite => lower_params(f, call),
        AsyncKind::StreamRead => {
            end(f);
            match call.results.first() {
                Some((data, MirType::String { capacity })) => {
                    address(f, data + 4);
                    f.instruction(&I::I32Const(*capacity as i32));
                }
                Some((data, MirType::Array(array))) => {
                    address(f, *data);
                    f.instruction(&I::I32Const(array.total_elements as i32));
                }
                _ => panic!(
                    "internal compiler error: a stream read's bytes land in a STRING or an \
                     ARRAY OF BYTE (E1516)"
                ),
            }
        }
        AsyncKind::FutureRead => {
            end(f);
            match call.result_area {
                Some(area) => {
                    for at in 0..call.results.len() as u64 {
                        f.instruction(&I::LocalGet(THIS));
                        f.instruction(&I::I64Const(0));
                        f.instruction(&I::I64Store(mem_arg(u64::from(area) + 8 * at, 3)));
                    }
                    address(f, area);
                }
                // A value with no field: the host writes nothing.
                None => address(f, call.state),
            }
        }
        AsyncKind::Call => unreachable!("internal compiler error: a call is no copy"),
    }
}

/// The copy's end, its outcome in the event: what [`copied`] writes, and no
/// copy pending. The stream's end stays in the block's set: the next copy
/// that does not end at once joins it there again, which moves nothing.
fn copy_ended(f: &mut Function, call: &MirAsyncCall) {
    copied(f, call, |f| {
        word(f, call.state + STATE_EVENT + 4);
    });
    set_word(f, call.state + STATE_HANDLE, |f| {
        f.instruction(&I::I32Const(0));
    });
}

/// A copy's outcome, which `outcome` pushes, `count << 4 | result`, written
/// into the VAR_OUTPUTs after DONE, and DONE. A stream write gives how many
/// bytes went and whether the reader left. A stream read gives the bytes,
/// how many came and whether the writer left: a STRING's length becomes the
/// count, and an ARRAY OF BYTE's bytes, which came a byte after the other,
/// are spread over its elements. A future read gives its value's fields.
fn copied(f: &mut Function, call: &MirAsyncCall, outcome: impl Fn(&mut Function)) {
    let count = |f: &mut Function| {
        outcome(f);
        f.instruction(&I::I32Const(4));
        f.instruction(&I::I32ShrU);
    };
    let left = |f: &mut Function, closed: u32, ty: &MirType| {
        address(f, closed);
        outcome(f);
        f.instruction(&I::I32Const(0xf));
        f.instruction(&I::I32And);
        f.instruction(&I::I32Const(DROPPED));
        f.instruction(&I::I32Eq);
        emit_typed_mem_store(f, ty);
    };
    match (call.kind, call.results.as_slice()) {
        (AsyncKind::StreamWrite, [(written, written_ty), (closed, closed_ty)]) => {
            address(f, *written);
            count(f);
            emit_typed_mem_store(f, written_ty);
            left(f, *closed, closed_ty);
        }
        (AsyncKind::StreamRead, [(data, data_ty), (read, read_ty), (closed, closed_ty)]) => {
            address(f, *read);
            count(f);
            emit_typed_mem_store(f, read_ty);
            match data_ty {
                MirType::String { .. } => set_word(f, *data, count),
                MirType::Array(array) => spread(f, *data, array, count),
                _ => panic!(
                    "internal compiler error: a stream read's bytes land in a STRING or an \
                     ARRAY OF BYTE (E1516)"
                ),
            }
            left(f, *closed, closed_ty);
        }
        (AsyncKind::FutureRead, _) => copy_result(f, call),
        _ => panic!(
            "internal compiler error: a copy declares its built-in's VAR_OUTPUTs after DONE \
             (E1516)"
        ),
    }
    store_bool(f, call.done, true);
}

/// Spreads the bytes a stream read left one after the other at the start of
/// `array`, at `data`, how many `count` pushes, over its elements, each in
/// its own place: backwards, so that each byte is read before an element is
/// written over it, since the element of a byte is never before it.
fn spread(
    f: &mut Function,
    data: u32,
    array: &mir::types::MirArrayType,
    count: impl Fn(&mut Function),
) {
    count(f);
    f.instruction(&I::LocalSet(INDEX));
    f.instruction(&I::Block(BlockType::Empty));
    f.instruction(&I::Loop(BlockType::Empty));
    f.instruction(&I::LocalGet(INDEX));
    f.instruction(&I::I32Eqz);
    f.instruction(&I::BrIf(1));
    f.instruction(&I::LocalGet(INDEX));
    f.instruction(&I::I32Const(1));
    f.instruction(&I::I32Sub);
    f.instruction(&I::LocalSet(INDEX));
    // The element: `this + data + index * size`.
    address(f, data);
    f.instruction(&I::LocalGet(INDEX));
    f.instruction(&I::I32Const(array.element_size as i32));
    f.instruction(&I::I32Mul);
    f.instruction(&I::I32Add);
    // The byte: `this + data + index`.
    address(f, data);
    f.instruction(&I::LocalGet(INDEX));
    f.instruction(&I::I32Add);
    f.instruction(&I::I32Load8U(mem_arg(0, 0)));
    emit_typed_mem_store(f, &array.element_type);
    f.instruction(&I::Br(0));
    f.instruction(&I::End);
    f.instruction(&I::End);
}

/// `this + offset`.
fn address(f: &mut Function, offset: u32) {
    f.instruction(&I::LocalGet(THIS));
    f.instruction(&I::I32Const(offset as i32));
    f.instruction(&I::I32Add);
}

/// The value of the instance's part of type `ty` at `offset`, as rk holds
/// it.
fn load(f: &mut Function, offset: u32, ty: &MirType) {
    address(f, offset);
    emit_typed_mem_load(f, ty);
}

/// A word of the call's state.
fn word(f: &mut Function, offset: u32) {
    f.instruction(&I::LocalGet(THIS));
    f.instruction(&I::I32Load(mem_arg(u64::from(offset), 2)));
}

/// Writes the word `value` pushes at `offset` of the call's state.
fn set_word(f: &mut Function, offset: u32, value: impl FnOnce(&mut Function)) {
    f.instruction(&I::LocalGet(THIS));
    value(f);
    f.instruction(&I::I32Store(mem_arg(u64::from(offset), 2)));
}

fn store_bool(f: &mut Function, offset: u32, value: bool) {
    address(f, offset);
    f.instruction(&I::I32Const(i32::from(value)));
    emit_typed_mem_store(f, &BOOL);
}

/// The canonical ABI's view of `types`: each a scalar or a STRING, as HIR
/// admits on an import.
fn fields(types: &[(u32, MirType)]) -> Vec<Canonical> {
    types
        .iter()
        .map(|(_, ty)| {
            canonical(ty).unwrap_or_else(|| {
                panic!("internal compiler error: an async call passes {ty:?}, no scalar or STRING")
            })
        })
        .collect()
}

/// Copies each field of the result, where the host wrote it, into its
/// VAR_OUTPUT.
fn copy_result(f: &mut Function, call: &MirAsyncCall) {
    let Some(area) = call.result_area else {
        return;
    };
    let canonical = fields(&call.results);
    let (offsets, size, _) = record_layout(&canonical);
    assert!(
        size as usize <= 8 * call.results.len(),
        "internal compiler error: an async result outgrows its area"
    );
    for (((output, ty), at), field) in call.results.iter().zip(offsets).zip(&canonical) {
        let elementary = field
            .elementary
            .expect("internal compiler error: an async call returns no STRING");
        address(f, *output);
        address(f, area);
        f.instruction(&crate::canonical::load(elementary, u64::from(at)));
        emit_typed_mem_store(f, ty);
    }
}

/// Pushes the call's parameters: each value, or past 4 values the address
/// of a tuple of them, laid out in the block's area.
fn lower_params(f: &mut Function, call: &MirAsyncCall) {
    let Some(area) = call.args_area else {
        for (offset, ty) in &call.params {
            match ty {
                MirType::String { .. } => {
                    address(f, offset + 4);
                    word(f, *offset);
                }
                _ => load(f, *offset, ty),
            }
        }
        return;
    };
    let canonical = fields(&call.params);
    let (offsets, size, _) = record_layout(&canonical);
    assert!(
        size as usize <= 8 * call.params.len(),
        "internal compiler error: an async call's parameters outgrow their area"
    );
    for (((offset, ty), at), field) in call.params.iter().zip(offsets).zip(&canonical) {
        let at = u64::from(at);
        match field.elementary {
            None => {
                // A STRING: its characters' address, then its length.
                address(f, area);
                address(f, offset + 4);
                f.instruction(&I::I32Store(mem_arg(at, 2)));
                address(f, area);
                word(f, *offset);
                f.instruction(&I::I32Store(mem_arg(at + 4, 2)));
            }
            Some(elementary) => {
                address(f, area);
                load(f, *offset, ty);
                f.instruction(&store(elementary, at));
            }
        }
    }
    address(f, area);
}
