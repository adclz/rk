// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! The writes to the host's standard output and error: the function behind
//! `{wasm 'rk.stdout_write'}` and `{wasm 'rk.stderr_write'}`, one per
//! output a module writes to, over the stream [`mir::console`] imports.
//!
//! An output's stream is made at the first write and kept: its reading end
//! goes to the host, the module holds the writing end in a global. A write
//! copies the text into the output's buffer and hands it to the stream,
//! which a host that is ready takes within the call.
//!
//! A function a host calls may not wait, and a write the host did not take
//! can be neither taken back nor dropped: both trap. So when the host takes
//! none of the bytes, they stay in the buffer for it, pending, and the
//! output is closed: this write and every later one answer FALSE, and the
//! buffer is not written again.

use mir::console::{Import, Output};
use wasm_encoder::{BlockType, ConstExpr, Function, GlobalType, Instruction, ValType};

use crate::WasmGen;

/// The bytes an output's buffer holds: the most one write hands over, the
/// capacity of the STRING `Std.Console` takes.
pub(crate) const CAPACITY: u32 = 4096;

/// An output with no stream yet, with one, and closed.
const NEW: i32 = 0;
const OPEN: i32 = 1;
const CLOSED: i32 = 2;

/// What `stream.write` answers when the host took nothing yet.
const BLOCKED: i32 = -1;

impl WasmGen<'_> {
    /// Emits the function that writes to `output`, `(ptr, len) -> i32`: 1
    /// when the host took all `len` bytes at `ptr`, 0 otherwise. `buffer`
    /// is the output's own, [`CAPACITY`] bytes.
    pub(crate) fn emit_console(&mut self, output: Output, buffer: u32) -> Function {
        let import = |import| {
            mir::console::import_at(self.module, output, import)
                .expect("a module that writes to an output imports its stream")
        };
        let mut global = |start: i32| {
            let index = self.global_section.len();
            self.global_section.global(
                GlobalType {
                    val_type: ValType::I32,
                    mutable: true,
                    shared: false,
                },
                &ConstExpr::i32_const(start),
            );
            index
        };
        let (state, end) = (global(NEW), global(0));

        // Params: the text 0 and its length 1; locals: the bytes to hand
        // over 2, those the host took 3, what a write answered 4, and the
        // stream's two ends 5.
        let mut f = Function::new([(3, ValType::I32), (1, ValType::I64)]);
        let (ptr, len, count, taken, answer, ends) = (0, 1, 2, 3, 4, 5);
        let closed = |f: &mut Function| {
            f.instruction(&Instruction::I32Const(CLOSED));
            f.instruction(&Instruction::GlobalSet(state));
        };

        f.instruction(&Instruction::GlobalGet(state));
        f.instruction(&Instruction::I32Const(CLOSED));
        f.instruction(&Instruction::I32Eq);
        f.instruction(&Instruction::If(BlockType::Empty));
        f.instruction(&Instruction::I32Const(0));
        f.instruction(&Instruction::Return);
        f.instruction(&Instruction::End);

        // The first write makes the stream. The host answers with a future
        // of how the writing went, which nothing here could wait for.
        f.instruction(&Instruction::GlobalGet(state));
        f.instruction(&Instruction::I32Eqz);
        f.instruction(&Instruction::If(BlockType::Empty));
        f.instruction(&Instruction::Call(import(Import::NewStream)));
        f.instruction(&Instruction::LocalSet(ends));
        f.instruction(&Instruction::LocalGet(ends));
        f.instruction(&Instruction::I32WrapI64);
        f.instruction(&Instruction::Call(import(Import::WriteViaStream)));
        f.instruction(&Instruction::Call(import(Import::DropFuture)));
        f.instruction(&Instruction::LocalGet(ends));
        f.instruction(&Instruction::I64Const(32));
        f.instruction(&Instruction::I64ShrU);
        f.instruction(&Instruction::I32WrapI64);
        f.instruction(&Instruction::GlobalSet(end));
        f.instruction(&Instruction::I32Const(OPEN));
        f.instruction(&Instruction::GlobalSet(state));
        f.instruction(&Instruction::End);

        // No more than the buffer holds: `min(len, CAPACITY)`, unsigned.
        f.instruction(&Instruction::LocalGet(len));
        f.instruction(&Instruction::I32Const(CAPACITY as i32));
        f.instruction(&Instruction::LocalGet(len));
        f.instruction(&Instruction::I32Const(CAPACITY as i32));
        f.instruction(&Instruction::I32LtU);
        f.instruction(&Instruction::Select);
        f.instruction(&Instruction::LocalSet(count));

        // The text into the buffer, where it stays if the host leaves it.
        f.instruction(&Instruction::I32Const(buffer as i32));
        f.instruction(&Instruction::LocalGet(ptr));
        f.instruction(&Instruction::LocalGet(count));
        f.instruction(&Instruction::MemoryCopy {
            src_mem: 0,
            dst_mem: 0,
        });

        // Handed over until the host has it all. A host may take a part.
        f.instruction(&Instruction::Block(BlockType::Empty));
        f.instruction(&Instruction::Loop(BlockType::Empty));
        f.instruction(&Instruction::LocalGet(taken));
        f.instruction(&Instruction::LocalGet(count));
        f.instruction(&Instruction::I32GeU);
        f.instruction(&Instruction::BrIf(1));
        f.instruction(&Instruction::GlobalGet(end));
        f.instruction(&Instruction::I32Const(buffer as i32));
        f.instruction(&Instruction::LocalGet(taken));
        f.instruction(&Instruction::I32Add);
        f.instruction(&Instruction::LocalGet(count));
        f.instruction(&Instruction::LocalGet(taken));
        f.instruction(&Instruction::I32Sub);
        f.instruction(&Instruction::Call(import(Import::Write)));
        f.instruction(&Instruction::LocalTee(answer));
        // The host took none and the write is pending: the output closes
        // over it.
        f.instruction(&Instruction::I32Const(BLOCKED));
        f.instruction(&Instruction::I32Eq);
        f.instruction(&Instruction::If(BlockType::Empty));
        closed(&mut f);
        f.instruction(&Instruction::I32Const(0));
        f.instruction(&Instruction::Return);
        f.instruction(&Instruction::End);
        f.instruction(&Instruction::LocalGet(taken));
        f.instruction(&Instruction::LocalGet(answer));
        f.instruction(&Instruction::I32Const(4));
        f.instruction(&Instruction::I32ShrU);
        f.instruction(&Instruction::I32Add);
        f.instruction(&Instruction::LocalSet(taken));
        // The host let go of its end: nothing more goes through.
        f.instruction(&Instruction::LocalGet(answer));
        f.instruction(&Instruction::I32Const(0xF));
        f.instruction(&Instruction::I32And);
        f.instruction(&Instruction::If(BlockType::Empty));
        closed(&mut f);
        f.instruction(&Instruction::Br(2));
        f.instruction(&Instruction::End);
        // It answered and took nothing: this write stops, short.
        f.instruction(&Instruction::LocalGet(answer));
        f.instruction(&Instruction::I32Const(4));
        f.instruction(&Instruction::I32ShrU);
        f.instruction(&Instruction::I32Eqz);
        f.instruction(&Instruction::BrIf(1));
        f.instruction(&Instruction::Br(0));
        f.instruction(&Instruction::End);
        f.instruction(&Instruction::End);

        // All of it, a text cut to the buffer included.
        f.instruction(&Instruction::LocalGet(taken));
        f.instruction(&Instruction::LocalGet(len));
        f.instruction(&Instruction::I32Eq);
        f.instruction(&Instruction::End);
        f
    }
}
