// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! The `rk:plc/control` interface: what a host calls, in the shapes the
//! component model's canonical ABI gives them. The functions are exported
//! beside the module's own (`__init`, the bodies, the band globals) under
//! the names a component takes them by, `rk:plc/control@0.1.0#run-task` and
//! so on, which no IEC identifier can spell.
//!
//! ```wit
//! package rk:plc@0.1.0;
//!
//! interface control {
//!     init: func() -> result<_, string>;
//!     run-task: func(task: u32, inputs: list<u8>) -> result<list<u8>, string>;
//!     save-retain: func() -> list<u8>;
//!     restore-retain: func(retained: list<u8>);
//!     run-test: func(test: u32) -> result<_, string>;
//! }
//! ```
//!
//! An error is the message of the exception that stopped the call, which
//! is caught here: one that left the module would trap at a component's
//! boundary, and lock its store.

use mir::MirModule;
use wasm_encoder::{BlockType, Catch, ConstExpr, Function, Instruction, ValType};

use crate::WasmGen;
use crate::emit_expr::mem_arg;

/// The interface the functions are exported under, with its version.
pub const INTERFACE: &str = debug_format::wit::CONTROL_INTERFACE;

/// What an unknown task or test answers.
const NO_TASK: &[u8] = b"no such task";
const NO_TEST: &[u8] = b"no such test";

/// Where a result's payload is, past its tag, when nothing in it is wider
/// than 4 bytes: every result of the control interface.
pub(crate) const PAYLOAD: u64 = 4;

/// The memory the functions a host calls need, past the `{test}` result
/// areas: a return area of 16 bytes, the messages, and the scratch what a
/// host passes lands in (`cabi_realloc`), as large as the largest list the
/// control interface takes and as the STRINGs any one `{export}` FUNCTION
/// does.
pub(crate) struct ControlArea {
    /// Where a result is written: a tag at 0, then its payload, at 4 or,
    /// holding a 64-bit value, at 8.
    pub ret: u32,
    pub no_task: u32,
    pub no_test: u32,
    pub too_long: u32,
    pub scratch: u32,
    pub scratch_size: u32,
    /// The first byte past the area.
    pub end: u32,
}

/// The control area of `module`, which starts at `base`: where its `{test}`
/// result areas end.
pub(crate) fn control_area(module: &MirModule, base: u32) -> ControlArea {
    let ret = base.next_multiple_of(8);
    let no_task = ret + 16;
    let no_test = no_task + NO_TASK.len() as u32;
    let too_long = no_test + NO_TEST.len() as u32;
    // A list of bytes and a string ask for an alignment of 1; 8 serves any.
    let scratch = (too_long + crate::exports::TOO_LONG.len() as u32).next_multiple_of(8);
    let scratch_size = module
        .input_size
        .max(module.retain_size)
        .max(crate::exports::strings_passed(module));
    ControlArea {
        ret,
        no_task,
        no_test,
        too_long,
        scratch,
        scratch_size,
        end: scratch + scratch_size,
    }
}

impl WasmGen<'_> {
    /// Emits the interface's functions after every other function, exports
    /// them, and writes the two messages.
    pub(crate) fn emit_control(&mut self, area: &ControlArea) {
        for (at, message) in [
            (area.no_task, NO_TASK),
            (area.no_test, NO_TEST),
            (area.too_long, crate::exports::TOO_LONG),
        ] {
            self.extra_data_section.active(
                0,
                &ConstExpr::i32_const(at as i32),
                message.iter().copied(),
            );
        }
        // A catch hands its `(ptr, len)` to the block it branches out of.
        let caught = self.rk_exception_tag_idx.map(|tag| {
            let ty = match self.test_catch_block_type_idx {
                Some(ty) => ty,
                None => {
                    let ty = self.add_type(&[], &[ValType::I32, ValType::I32]);
                    self.test_catch_block_type_idx = Some(ty);
                    ty
                }
            };
            (tag, ty)
        });

        // Where the next thing a host passes lands in the scratch: each
        // function that takes one starts it over.
        let arena = self.global_section.len();
        self.global_section.global(
            wasm_encoder::GlobalType {
                val_type: ValType::I32,
                mutable: true,
                shared: false,
            },
            &ConstExpr::i32_const(area.scratch as i32),
        );

        let init = self.export_index("__init");
        let f = self.control_init(area, init, caught);
        self.add_control_function("init", &[], &[ValType::I32], &f);

        let f = self.control_run_task(area, caught, arena);
        self.add_control_function(
            "run-task",
            &[ValType::I32, ValType::I32, ValType::I32],
            &[ValType::I32],
            &f,
        );

        let f = self.control_save_retain(area);
        self.add_control_function("save-retain", &[], &[ValType::I32], &f);

        let f = self.control_restore_retain(area, arena);
        self.add_control_function("restore-retain", &[ValType::I32, ValType::I32], &[], &f);

        let f = self.control_run_test(area);
        self.add_control_function("run-test", &[ValType::I32], &[ValType::I32], &f);

        self.emit_exports(area, caught, arena);

        let spill = crate::core_memory_pages(self.module) * u64::from(crate::WASM_PAGE);
        let f = control_realloc(area, arena, spill);
        let index = self.add_function(&[ValType::I32; 4], &[ValType::I32], &f);
        self.export_section
            .export("cabi_realloc", wasm_encoder::ExportKind::Func, index);
        self.control_names.push((index, "cabi_realloc".to_string()));
    }

    pub(crate) fn add_type(&mut self, params: &[ValType], results: &[ValType]) -> u32 {
        let ty = self.next_type_idx;
        self.type_section
            .ty()
            .function(params.iter().copied(), results.iter().copied());
        self.next_type_idx += 1;
        ty
    }

    /// Appends `f` as the next defined function, and returns its index.
    pub(crate) fn add_function(
        &mut self,
        params: &[ValType],
        results: &[ValType],
        f: &Function,
    ) -> u32 {
        let ty = self.add_type(params, results);
        let index = self.module.extern_functions.len() as u32 + self.fn_section.len();
        self.fn_section.function(ty);
        self.code_section.function(f);
        index
    }

    fn add_control_function(
        &mut self,
        name: &str,
        params: &[ValType],
        results: &[ValType],
        f: &Function,
    ) {
        let index = self.add_function(params, results, f);
        let export = format!("{INTERFACE}#{name}");
        self.export_section
            .export(&export, wasm_encoder::ExportKind::Func, index);
        self.control_names.push((index, export));
    }

    /// The wasm index of the function exported as `name`: by its export
    /// name, or its own when it has none, as codegen exports it.
    fn export_index(&self, name: &str) -> Option<u32> {
        let func = self.module.functions.iter().find(|f| {
            f.linkage == mir::function::MirLinkage::Export
                && f.export_name
                    .as_deref()
                    .map_or_else(|| f.name.text(self.db) == name, |export| export == name)
        })?;
        self.index_remap.get(&func.index).copied()
    }

    /// `init`: the starting values, `__init`, when the module has any.
    fn control_init(
        &self,
        area: &ControlArea,
        init: Option<u32>,
        caught: Option<(u32, u32)>,
    ) -> Function {
        let mut f = Function::new([(2, ValType::I32)]);
        let (ptr, len) = (0, 1);
        catching(&mut f, area, caught, (ptr, len), PAYLOAD, |f| {
            if let Some(init) = init {
                f.instruction(&Instruction::Call(init));
            }
            write_ok(f, area, None);
            f.instruction(&Instruction::I32Const(area.ret as i32));
            f.instruction(&Instruction::Return);
        });
        f
    }

    /// `run-task`: the inputs copied into the input band, as much of them
    /// as it holds; each program instance of the task run in order; the
    /// output band returned. An unknown task is an error.
    fn control_run_task(
        &self,
        area: &ControlArea,
        caught: Option<(u32, u32)>,
        arena: u32,
    ) -> Function {
        // Params: task 0, ptr 1, len 2; locals: the caught message 3, 4.
        let mut f = Function::new([(2, ValType::I32)]);
        let (task, input_ptr, input_len) = (0, 1, 2);
        start_over(&mut f, area, arena);
        copy_in(
            &mut f,
            self.module.input_base,
            self.module.input_size,
            input_ptr,
            input_len,
        );
        let tasks: Vec<Vec<(u32, u32)>> = self
            .module
            .schedule_manifest
            .iter()
            .flat_map(|manifest| &manifest.tasks)
            .map(|task| {
                task.programs
                    .iter()
                    .filter_map(|program| {
                        let body = self.export_index(&program.export)?;
                        Some((body, program.instance_addr))
                    })
                    .collect()
            })
            .collect();
        let output = (self.module.output_base, self.module.output_size);
        catching(&mut f, area, caught, (3, 4), PAYLOAD, |f| {
            for (index, programs) in tasks.iter().enumerate() {
                f.instruction(&Instruction::LocalGet(task));
                f.instruction(&Instruction::I32Const(index as i32));
                f.instruction(&Instruction::I32Eq);
                f.instruction(&Instruction::If(BlockType::Empty));
                for (body, this) in programs {
                    f.instruction(&Instruction::I32Const(*this as i32));
                    f.instruction(&Instruction::Call(*body));
                }
                write_ok(f, area, Some(output));
                f.instruction(&Instruction::I32Const(area.ret as i32));
                f.instruction(&Instruction::Return);
                f.instruction(&Instruction::End);
            }
            write_err_static(f, area, PAYLOAD, area.no_task, NO_TASK.len() as u32);
            f.instruction(&Instruction::I32Const(area.ret as i32));
            f.instruction(&Instruction::Return);
        });
        f
    }

    /// `save-retain`: the retain band, which the host copies out.
    fn control_save_retain(&self, area: &ControlArea) -> Function {
        let mut f = Function::new([]);
        f.instruction(&Instruction::I32Const(area.ret as i32));
        f.instruction(&Instruction::I32Const(self.module.retain_base as i32));
        f.instruction(&Instruction::I32Store(mem_arg(0, 2)));
        f.instruction(&Instruction::I32Const(area.ret as i32));
        f.instruction(&Instruction::I32Const(self.module.retain_size as i32));
        f.instruction(&Instruction::I32Store(mem_arg(4, 2)));
        f.instruction(&Instruction::I32Const(area.ret as i32));
        f.instruction(&Instruction::End);
        f
    }

    /// `restore-retain`: the bytes copied into the retain band, as much of
    /// them as it holds.
    fn control_restore_retain(&self, area: &ControlArea, arena: u32) -> Function {
        let mut f = Function::new([]);
        start_over(&mut f, area, arena);
        copy_in(
            &mut f,
            self.module.retain_base,
            self.module.retain_size,
            0,
            1,
        );
        f.instruction(&Instruction::End);
        f
    }

    /// `run-test`: the test at that place in the `test-manifest`, whose own
    /// wrapper catches what it raises. A release runs none: its tests are
    /// left for the optimizer to remove.
    fn control_run_test(&self, area: &ControlArea) -> Function {
        let mut f = Function::new([]);
        if self.profile != crate::Profile::Release {
            for (index, entry) in self.module.test_manifest.tests.iter().enumerate() {
                let Some(wrapper) = self.export_index(&entry.export) else {
                    continue;
                };
                f.instruction(&Instruction::LocalGet(0));
                f.instruction(&Instruction::I32Const(index as i32));
                f.instruction(&Instruction::I32Eq);
                f.instruction(&Instruction::If(BlockType::Empty));
                f.instruction(&Instruction::Call(wrapper));
                f.instruction(&Instruction::Return);
                f.instruction(&Instruction::End);
            }
        }
        write_err_static(&mut f, area, PAYLOAD, area.no_test, NO_TEST.len() as u32);
        f.instruction(&Instruction::I32Const(area.ret as i32));
        f.instruction(&Instruction::End);
        f
    }
}

/// `cabi_realloc`, where what a host passes lands: the next free bytes of
/// the scratch, which every function that takes a list or a string starts
/// over (`arena`).
///
/// What does not fit there is longer than anything the functions take: it
/// lands at `spill`, past everything the module laid out, in memory grown
/// to hold it, and only a host refusing the memory traps. The function it
/// was passed to cuts it or refuses it. Two such blocks of one call share
/// that place, and neither is read: an `{export}` FUNCTION refuses a call
/// that passed one too long, and the control interface takes one list.
fn control_realloc(area: &ControlArea, arena: u32, spill: u64) -> Function {
    // Params: the old block 0 and its size 1, the alignment 2, the size 3;
    // local: where the block is 4.
    let mut f = Function::new([(1, ValType::I32)]);
    let (old, old_size, align, size, at) = (0, 1, 2, 3, 4);
    let end = (area.scratch + area.scratch_size) as i32;
    // The next free byte, aligned: `(arena + align - 1) & -align`.
    f.instruction(&Instruction::GlobalGet(arena));
    f.instruction(&Instruction::LocalGet(align));
    f.instruction(&Instruction::I32Add);
    f.instruction(&Instruction::I32Const(1));
    f.instruction(&Instruction::I32Sub);
    f.instruction(&Instruction::I32Const(0));
    f.instruction(&Instruction::LocalGet(align));
    f.instruction(&Instruction::I32Sub);
    f.instruction(&Instruction::I32And);
    f.instruction(&Instruction::LocalSet(at));
    // It fits when `at <= end` and `size <= end - at`.
    f.instruction(&Instruction::LocalGet(at));
    f.instruction(&Instruction::I32Const(end));
    f.instruction(&Instruction::I32LeU);
    f.instruction(&Instruction::LocalGet(size));
    f.instruction(&Instruction::I32Const(end));
    f.instruction(&Instruction::LocalGet(at));
    f.instruction(&Instruction::I32Sub);
    f.instruction(&Instruction::I32LeU);
    f.instruction(&Instruction::I32And);
    f.instruction(&Instruction::If(BlockType::Empty));
    f.instruction(&Instruction::LocalGet(at));
    f.instruction(&Instruction::LocalGet(size));
    f.instruction(&Instruction::I32Add);
    f.instruction(&Instruction::GlobalSet(arena));
    f.instruction(&Instruction::Else);
    // The pages `spill + size` bytes take, less those the memory has.
    f.instruction(&Instruction::I64Const(spill as i64));
    f.instruction(&Instruction::LocalGet(size));
    f.instruction(&Instruction::I64ExtendI32U);
    f.instruction(&Instruction::I64Add);
    f.instruction(&Instruction::I64Const(i64::from(crate::WASM_PAGE) - 1));
    f.instruction(&Instruction::I64Add);
    f.instruction(&Instruction::I64Const(16));
    f.instruction(&Instruction::I64ShrU);
    f.instruction(&Instruction::I32WrapI64);
    f.instruction(&Instruction::MemorySize(0));
    f.instruction(&Instruction::I32Sub);
    f.instruction(&Instruction::LocalTee(at));
    f.instruction(&Instruction::I32Const(0));
    f.instruction(&Instruction::I32GtS);
    f.instruction(&Instruction::If(BlockType::Empty));
    f.instruction(&Instruction::LocalGet(at));
    f.instruction(&Instruction::MemoryGrow(0));
    f.instruction(&Instruction::I32Const(-1));
    f.instruction(&Instruction::I32Eq);
    f.instruction(&Instruction::If(BlockType::Empty));
    f.instruction(&Instruction::Unreachable);
    f.instruction(&Instruction::End);
    f.instruction(&Instruction::End);
    f.instruction(&Instruction::I32Const(spill as u32 as i32));
    f.instruction(&Instruction::LocalSet(at));
    f.instruction(&Instruction::End);
    // A block that moved keeps its bytes, as many as both sizes hold.
    f.instruction(&Instruction::LocalGet(old));
    f.instruction(&Instruction::If(BlockType::Empty));
    f.instruction(&Instruction::LocalGet(at));
    f.instruction(&Instruction::LocalGet(old));
    f.instruction(&Instruction::LocalGet(old_size));
    f.instruction(&Instruction::LocalGet(size));
    f.instruction(&Instruction::LocalGet(old_size));
    f.instruction(&Instruction::LocalGet(size));
    f.instruction(&Instruction::I32LtU);
    f.instruction(&Instruction::Select);
    f.instruction(&Instruction::MemoryCopy {
        src_mem: 0,
        dst_mem: 0,
    });
    f.instruction(&Instruction::End);
    f.instruction(&Instruction::LocalGet(at));
    f.instruction(&Instruction::End);
    f
}

/// Starts the scratch over: what the host passed this call is in it, and
/// what it passes the next one lands at its start.
pub(crate) fn start_over(f: &mut Function, area: &ControlArea, arena: u32) {
    f.instruction(&Instruction::I32Const(area.scratch as i32));
    f.instruction(&Instruction::GlobalSet(arena));
}

/// Copies the list `(ptr, len)` the locals hold to `base`, as much of it as
/// `size` bytes take.
fn copy_in(f: &mut Function, base: u32, size: u32, ptr: u32, len: u32) {
    if size == 0 {
        return;
    }
    f.instruction(&Instruction::I32Const(base as i32));
    f.instruction(&Instruction::LocalGet(ptr));
    // min(len, size), unsigned.
    f.instruction(&Instruction::LocalGet(len));
    f.instruction(&Instruction::I32Const(size as i32));
    f.instruction(&Instruction::LocalGet(len));
    f.instruction(&Instruction::I32Const(size as i32));
    f.instruction(&Instruction::I32LtU);
    f.instruction(&Instruction::Select);
    f.instruction(&Instruction::MemoryCopy {
        src_mem: 0,
        dst_mem: 0,
    });
}

/// Runs `body`, which returns, and when the module can raise, catches what
/// it raises into an error: the message stashed in the locals `stash`, then
/// written at the return area, `payload` bytes in, whose address the
/// function returns.
pub(crate) fn catching(
    f: &mut Function,
    area: &ControlArea,
    caught: Option<(u32, u32)>,
    stash: (u32, u32),
    payload: u64,
    body: impl FnOnce(&mut Function),
) {
    let Some((tag, block_ty)) = caught else {
        body(f);
        f.instruction(&Instruction::End);
        return;
    };
    f.instruction(&Instruction::Block(BlockType::FunctionType(block_ty)));
    f.instruction(&Instruction::TryTable(
        BlockType::Empty,
        std::borrow::Cow::Owned(vec![Catch::One { tag, label: 0 }]),
    ));
    body(f);
    f.instruction(&Instruction::End);
    // `body` returned on every path: what follows the try_table is never
    // reached, and the block's `(ptr, len)` is never there.
    f.instruction(&Instruction::Unreachable);
    f.instruction(&Instruction::End);
    let (ptr, len) = stash;
    f.instruction(&Instruction::LocalSet(len));
    f.instruction(&Instruction::LocalSet(ptr));
    write_err(f, area, payload, ptr, len);
    f.instruction(&Instruction::I32Const(area.ret as i32));
    f.instruction(&Instruction::End);
}

/// `Ok` at the return area, with the list `(ptr, len)` it carries.
fn write_ok(f: &mut Function, area: &ControlArea, list: Option<(u32, u32)>) {
    f.instruction(&Instruction::I32Const(area.ret as i32));
    f.instruction(&Instruction::I32Const(0));
    f.instruction(&Instruction::I32Store8(mem_arg(0, 0)));
    if let Some((ptr, len)) = list {
        for (offset, value) in [(4, ptr), (8, len)] {
            f.instruction(&Instruction::I32Const(area.ret as i32));
            f.instruction(&Instruction::I32Const(value as i32));
            f.instruction(&Instruction::I32Store(mem_arg(offset, 2)));
        }
    }
}

/// `Err` at the return area, with the message the locals `ptr`, `len` hold,
/// `payload` bytes in.
fn write_err(f: &mut Function, area: &ControlArea, payload: u64, ptr: u32, len: u32) {
    f.instruction(&Instruction::I32Const(area.ret as i32));
    f.instruction(&Instruction::I32Const(1));
    f.instruction(&Instruction::I32Store8(mem_arg(0, 0)));
    for (offset, local) in [(payload, ptr), (payload + 4, len)] {
        f.instruction(&Instruction::I32Const(area.ret as i32));
        f.instruction(&Instruction::LocalGet(local));
        f.instruction(&Instruction::I32Store(mem_arg(offset, 2)));
    }
}

/// `Err` at the return area, with a message the module holds at `at`,
/// `payload` bytes in.
pub(crate) fn write_err_static(
    f: &mut Function,
    area: &ControlArea,
    payload: u64,
    at: u32,
    len: u32,
) {
    f.instruction(&Instruction::I32Const(area.ret as i32));
    f.instruction(&Instruction::I32Const(1));
    f.instruction(&Instruction::I32Store8(mem_arg(0, 0)));
    for (offset, value) in [(payload, at), (payload + 4, len)] {
        f.instruction(&Instruction::I32Const(area.ret as i32));
        f.instruction(&Instruction::I32Const(value as i32));
        f.instruction(&Instruction::I32Store(mem_arg(offset, 2)));
    }
}
