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
//!     variant scan-error { raised(string), no-such-task }
//!     run-task: func(task: u32, inputs: list<u8>) -> result<list<u8>, scan-error>;
//!     save-retain: func() -> list<u8>;
//!     restore-retain: func(retained: list<u8>);
//!     record range { address: u32, size: u32 }
//!     variant memory-error { outside-memory(u32), too-large, outside-variables }
//!     read-memory: func(ranges: list<range>) -> result<list<u8>, memory-error>;
//!     write-memory: func(address: u32, bytes: list<u8>) -> result<_, memory-error>;
//!     variant test-error { failed(string), no-such-test }
//!     run-test: func(test: u32) -> result<_, test-error>;
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

/// The cases of the interface's errors, numbered in the order the WIT lists
/// them: `scan-error`'s, `test-error`'s and `memory-error`'s.
const RAISED: u8 = 0;
const NO_SUCH_TASK: u8 = 1;
const FAILED: u8 = 0;
const NO_SUCH_TEST: u8 = 1;
const OUTSIDE_MEMORY: u8 = 0;
const TOO_LARGE: u8 = 1;
const OUTSIDE_VARIABLES: u8 = 2;

/// Where a result's payload is, past its tag, when nothing in it is wider
/// than 4 bytes: every result of the control interface. An error that is a
/// variant has its case there, and what the case holds 4 bytes on.
pub(crate) const PAYLOAD: u64 = 4;

/// What a raise caught at the interface becomes.
#[derive(Clone, Copy)]
pub(crate) enum Raised {
    /// The error itself, a `string`, `payload` bytes into the result.
    Message { payload: u64 },
    /// The `string` of that case of the error's variant.
    Case(u8),
}

/// The memory the functions a host calls need, past the `{test}` result
/// areas: a return area of 16 bytes, the message, and the scratch what a
/// host passes lands in (`cabi_realloc`), as large as the largest list the
/// control interface takes and as the STRINGs any one `{export}` FUNCTION
/// does. Past them, the buffer of each output the module writes to
/// ([`crate::console`]).
pub(crate) struct ControlArea {
    /// Where a result is written: a tag at 0, then its payload, at 4 or,
    /// holding a 64-bit value, at 8.
    pub ret: u32,
    pub too_long: u32,
    pub scratch: u32,
    pub scratch_size: u32,
    /// Where the text written to the standard output waits for the host,
    /// then to the standard error: for an output the module writes to.
    pub console: [u32; 2],
    /// The first byte past the area.
    pub end: u32,
}

/// The control area of `module`, which starts at `base`: where its `{test}`
/// result areas end.
pub(crate) fn control_area(module: &MirModule, base: u32) -> ControlArea {
    let ret = base.next_multiple_of(8);
    let too_long = ret + 16;
    // A list of bytes and a string ask for an alignment of 1, a list of
    // ranges 4; 8 serves any.
    let scratch = (too_long + crate::exports::TOO_LONG.len() as u32).next_multiple_of(8);
    let scratch_size = module
        .input_size
        .max(module.retain_size)
        .max(crate::exports::strings_passed(module));
    let mut end = scratch + scratch_size;
    let mut console = [0; 2];
    for output in mir::console::outputs(module) {
        console[output as usize] = end;
        end += crate::console::CAPACITY;
    }
    ControlArea {
        ret,
        too_long,
        scratch,
        scratch_size,
        console,
        end,
    }
}

impl WasmGen<'_> {
    /// Emits the interface's functions after every other function, exports
    /// them, and writes the message an `{export}` FUNCTION refuses a long
    /// STRING with.
    pub(crate) fn emit_control(&mut self, area: &ControlArea) {
        self.extra_data_section.active(
            0,
            &ConstExpr::i32_const(area.too_long as i32),
            crate::exports::TOO_LONG.iter().copied(),
        );
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

        // Past everything the module laid out: where what does not fit in
        // the scratch lands.
        let spill = crate::core_memory_pages(self.module) * u64::from(crate::WASM_PAGE);
        let f = control_read_memory(area, arena, spill);
        self.add_control_function(
            "read-memory",
            &[ValType::I32, ValType::I32],
            &[ValType::I32],
            &f,
        );

        // The variables: past the builtins' memory, up to the string
        // literals, which come after the static layout.
        let variables = (
            mir::memory::BUILTIN_RESERVED_FLOOR,
            self.module.memory_layout.total_size(),
        );
        let f = control_write_memory(area, arena, variables);
        self.add_control_function(
            "write-memory",
            &[ValType::I32, ValType::I32, ValType::I32],
            &[ValType::I32],
            &f,
        );

        let f = self.control_run_test(area);
        self.add_control_function("run-test", &[ValType::I32], &[ValType::I32], &f);

        self.emit_exports(area, caught, arena);

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
        let raised = Raised::Message { payload: PAYLOAD };
        catching(&mut f, area, caught, (ptr, len), raised, |f| {
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
        catching(&mut f, area, caught, (3, 4), Raised::Case(RAISED), |f| {
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
            write_err_case(f, area, NO_SUCH_TASK);
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
    /// wrapper catches what it raises into a `result<_, string>` of its own,
    /// read back here into the interface's. A release runs none: its tests
    /// are left for the optimizer to remove.
    fn control_run_test(&self, area: &ControlArea) -> Function {
        // Param: the test 0; local: where the wrapper's result is 1.
        let mut f = Function::new([(1, ValType::I32)]);
        let (test, result) = (0, 1);
        if self.profile != crate::Profile::Release {
            for (index, entry) in self.module.test_manifest.tests.iter().enumerate() {
                let Some(wrapper) = self.export_index(&entry.export) else {
                    continue;
                };
                f.instruction(&Instruction::LocalGet(test));
                f.instruction(&Instruction::I32Const(index as i32));
                f.instruction(&Instruction::I32Eq);
                f.instruction(&Instruction::If(BlockType::Empty));
                f.instruction(&Instruction::Call(wrapper));
                f.instruction(&Instruction::LocalTee(result));
                f.instruction(&Instruction::I32Load8U(mem_arg(0, 0)));
                f.instruction(&Instruction::If(BlockType::Empty));
                write_err_case(&mut f, area, FAILED);
                for offset in [4, 8] {
                    f.instruction(&Instruction::I32Const(area.ret as i32));
                    f.instruction(&Instruction::LocalGet(result));
                    f.instruction(&Instruction::I32Load(mem_arg(offset, 2)));
                    f.instruction(&Instruction::I32Store(mem_arg(PAYLOAD + offset, 2)));
                }
                f.instruction(&Instruction::Else);
                write_ok(&mut f, area, None);
                f.instruction(&Instruction::End);
                f.instruction(&Instruction::I32Const(area.ret as i32));
                f.instruction(&Instruction::Return);
                f.instruction(&Instruction::End);
            }
        }
        write_err_case(&mut f, area, NO_SUCH_TEST);
        f.instruction(&Instruction::I32Const(area.ret as i32));
        f.instruction(&Instruction::End);
        f
    }
}

/// `read-memory`: the bytes of each range of the list `(ptr, count)`, one
/// after the other, in a list of the module's that the host copies when the
/// call returns. Each range of 8 bytes is an address, then a size.
///
/// The list goes after the ranges in the scratch when it fits there, and
/// otherwise past everything the module laid out (`spill`), or past the
/// ranges when they are there themselves, in memory grown to hold it: the
/// buffers that follow the scratch are the outputs', which a pending write
/// may still hold.
fn control_read_memory(area: &ControlArea, arena: u32, spill: u64) -> Function {
    // Params: the ranges 0 and their count 1; locals: the range at hand 2,
    // its address 3 and size 4, where the list goes 5, where the next bytes
    // go 6, the error's case 7, and in 64 bits the list's size 8, where the
    // ranges end 9, and where the list starts past the scratch 10.
    let mut f = Function::new([(6, ValType::I32), (3, ValType::I64)]);
    let (ptr, count) = (0, 1);
    let (i, address, size, out, cursor, why) = (2, 3, 4, 5, 6, 7);
    let (total, ranges_end, start) = (8, 9, 10);
    let scratch_end = u64::from(area.scratch) + u64::from(area.scratch_size);
    let why_is = |f: &mut Function, case: u8| {
        f.instruction(&Instruction::I32Const(i32::from(case)));
        f.instruction(&Instruction::LocalSet(why));
    };
    // The range at `i`: its address, then its size.
    let range_at = |f: &mut Function| {
        f.instruction(&Instruction::LocalGet(ptr));
        f.instruction(&Instruction::LocalGet(i));
        f.instruction(&Instruction::I32Const(3));
        f.instruction(&Instruction::I32Shl);
        f.instruction(&Instruction::I32Add);
        f.instruction(&Instruction::LocalTee(address));
        f.instruction(&Instruction::I32Load(mem_arg(4, 2)));
        f.instruction(&Instruction::LocalSet(size));
        f.instruction(&Instruction::LocalGet(address));
        f.instruction(&Instruction::I32Load(mem_arg(0, 2)));
        f.instruction(&Instruction::LocalSet(address));
    };

    f.instruction(&Instruction::Block(BlockType::Empty));
    // Each range in the memory, and the list's size.
    why_is(&mut f, OUTSIDE_MEMORY);
    f.instruction(&Instruction::Block(BlockType::Empty));
    f.instruction(&Instruction::Loop(BlockType::Empty));
    f.instruction(&Instruction::LocalGet(i));
    f.instruction(&Instruction::LocalGet(count));
    f.instruction(&Instruction::I32GeU);
    f.instruction(&Instruction::BrIf(1));
    range_at(&mut f);
    // `address + size` past the memory's end.
    f.instruction(&Instruction::LocalGet(address));
    f.instruction(&Instruction::I64ExtendI32U);
    f.instruction(&Instruction::LocalGet(size));
    f.instruction(&Instruction::I64ExtendI32U);
    f.instruction(&Instruction::I64Add);
    f.instruction(&Instruction::MemorySize(0));
    f.instruction(&Instruction::I64ExtendI32U);
    f.instruction(&Instruction::I64Const(16));
    f.instruction(&Instruction::I64Shl);
    f.instruction(&Instruction::I64GtU);
    f.instruction(&Instruction::BrIf(2));
    f.instruction(&Instruction::LocalGet(total));
    f.instruction(&Instruction::LocalGet(size));
    f.instruction(&Instruction::I64ExtendI32U);
    f.instruction(&Instruction::I64Add);
    f.instruction(&Instruction::LocalSet(total));
    f.instruction(&Instruction::LocalGet(i));
    f.instruction(&Instruction::I32Const(1));
    f.instruction(&Instruction::I32Add);
    f.instruction(&Instruction::LocalSet(i));
    f.instruction(&Instruction::Br(0));
    f.instruction(&Instruction::End);
    f.instruction(&Instruction::End);

    // Where the list goes.
    f.instruction(&Instruction::LocalGet(ptr));
    f.instruction(&Instruction::I64ExtendI32U);
    f.instruction(&Instruction::LocalGet(count));
    f.instruction(&Instruction::I64ExtendI32U);
    f.instruction(&Instruction::I64Const(3));
    f.instruction(&Instruction::I64Shl);
    f.instruction(&Instruction::I64Add);
    f.instruction(&Instruction::LocalSet(ranges_end));
    // After the ranges in the scratch, when they are there and it fits.
    f.instruction(&Instruction::LocalGet(ptr));
    f.instruction(&Instruction::I32Const(area.scratch as i32));
    f.instruction(&Instruction::I32GeU);
    f.instruction(&Instruction::LocalGet(ranges_end));
    f.instruction(&Instruction::LocalGet(total));
    f.instruction(&Instruction::I64Add);
    f.instruction(&Instruction::I64Const(scratch_end as i64));
    f.instruction(&Instruction::I64LeU);
    f.instruction(&Instruction::I32And);
    f.instruction(&Instruction::If(BlockType::Empty));
    f.instruction(&Instruction::LocalGet(ranges_end));
    f.instruction(&Instruction::I32WrapI64);
    f.instruction(&Instruction::LocalSet(out));
    f.instruction(&Instruction::Else);
    // Otherwise past the spill, or past the ranges when they are there.
    f.instruction(&Instruction::LocalGet(ranges_end));
    f.instruction(&Instruction::I64Const(spill as i64));
    f.instruction(&Instruction::LocalGet(ranges_end));
    f.instruction(&Instruction::I64Const(spill as i64));
    f.instruction(&Instruction::I64GtU);
    f.instruction(&Instruction::Select);
    f.instruction(&Instruction::LocalSet(start));
    why_is(&mut f, TOO_LARGE);
    f.instruction(&Instruction::LocalGet(start));
    f.instruction(&Instruction::LocalGet(total));
    f.instruction(&Instruction::I64Add);
    f.instruction(&Instruction::I64Const(1 << 32));
    f.instruction(&Instruction::I64GtU);
    f.instruction(&Instruction::BrIf(1));
    f.instruction(&Instruction::LocalGet(start));
    f.instruction(&Instruction::I32WrapI64);
    f.instruction(&Instruction::LocalSet(out));
    // The pages `start + total` bytes take, less those the memory has.
    f.instruction(&Instruction::LocalGet(start));
    f.instruction(&Instruction::LocalGet(total));
    f.instruction(&Instruction::I64Add);
    f.instruction(&Instruction::I64Const(i64::from(crate::WASM_PAGE) - 1));
    f.instruction(&Instruction::I64Add);
    f.instruction(&Instruction::I64Const(16));
    f.instruction(&Instruction::I64ShrU);
    f.instruction(&Instruction::I32WrapI64);
    f.instruction(&Instruction::MemorySize(0));
    f.instruction(&Instruction::I32Sub);
    f.instruction(&Instruction::LocalTee(size));
    f.instruction(&Instruction::I32Const(0));
    f.instruction(&Instruction::I32GtS);
    f.instruction(&Instruction::If(BlockType::Empty));
    f.instruction(&Instruction::LocalGet(size));
    f.instruction(&Instruction::MemoryGrow(0));
    f.instruction(&Instruction::I32Const(-1));
    f.instruction(&Instruction::I32Eq);
    f.instruction(&Instruction::If(BlockType::Empty));
    f.instruction(&Instruction::Unreachable);
    f.instruction(&Instruction::End);
    f.instruction(&Instruction::End);
    f.instruction(&Instruction::End);

    // The bytes of each range, one after the other.
    f.instruction(&Instruction::LocalGet(out));
    f.instruction(&Instruction::LocalSet(cursor));
    f.instruction(&Instruction::I32Const(0));
    f.instruction(&Instruction::LocalSet(i));
    f.instruction(&Instruction::Block(BlockType::Empty));
    f.instruction(&Instruction::Loop(BlockType::Empty));
    f.instruction(&Instruction::LocalGet(i));
    f.instruction(&Instruction::LocalGet(count));
    f.instruction(&Instruction::I32GeU);
    f.instruction(&Instruction::BrIf(1));
    range_at(&mut f);
    f.instruction(&Instruction::LocalGet(cursor));
    f.instruction(&Instruction::LocalGet(address));
    f.instruction(&Instruction::LocalGet(size));
    f.instruction(&Instruction::MemoryCopy {
        src_mem: 0,
        dst_mem: 0,
    });
    f.instruction(&Instruction::LocalGet(cursor));
    f.instruction(&Instruction::LocalGet(size));
    f.instruction(&Instruction::I32Add);
    f.instruction(&Instruction::LocalSet(cursor));
    f.instruction(&Instruction::LocalGet(i));
    f.instruction(&Instruction::I32Const(1));
    f.instruction(&Instruction::I32Add);
    f.instruction(&Instruction::LocalSet(i));
    f.instruction(&Instruction::Br(0));
    f.instruction(&Instruction::End);
    f.instruction(&Instruction::End);

    // `Ok` with the list.
    f.instruction(&Instruction::I32Const(area.ret as i32));
    f.instruction(&Instruction::I32Const(0));
    f.instruction(&Instruction::I32Store8(mem_arg(0, 0)));
    f.instruction(&Instruction::I32Const(area.ret as i32));
    f.instruction(&Instruction::LocalGet(out));
    f.instruction(&Instruction::I32Store(mem_arg(PAYLOAD, 2)));
    f.instruction(&Instruction::I32Const(area.ret as i32));
    f.instruction(&Instruction::LocalGet(total));
    f.instruction(&Instruction::I32WrapI64);
    f.instruction(&Instruction::I32Store(mem_arg(PAYLOAD + 4, 2)));
    start_over(&mut f, area, arena);
    f.instruction(&Instruction::I32Const(area.ret as i32));
    f.instruction(&Instruction::Return);
    f.instruction(&Instruction::End);

    // The case, and the place of the range at hand: the one outside the
    // memory, for `outside-memory`.
    f.instruction(&Instruction::I32Const(area.ret as i32));
    f.instruction(&Instruction::I32Const(1));
    f.instruction(&Instruction::I32Store8(mem_arg(0, 0)));
    f.instruction(&Instruction::I32Const(area.ret as i32));
    f.instruction(&Instruction::LocalGet(why));
    f.instruction(&Instruction::I32Store8(mem_arg(PAYLOAD, 0)));
    f.instruction(&Instruction::I32Const(area.ret as i32));
    f.instruction(&Instruction::LocalGet(i));
    f.instruction(&Instruction::I32Store(mem_arg(PAYLOAD + 4, 2)));
    start_over(&mut f, area, arena);
    f.instruction(&Instruction::I32Const(area.ret as i32));
    f.instruction(&Instruction::End);
    f
}

/// `write-memory`: the list `(ptr, len)` copied to `address`, when it lands
/// in `variables`, `[start, end)`; an error otherwise.
fn control_write_memory(area: &ControlArea, arena: u32, variables: (u32, u32)) -> Function {
    // Params: the address 0, then the list 1, 2.
    let mut f = Function::new([]);
    let (address, ptr, len) = (0, 1, 2);
    let (start, end) = variables;
    // What the host passed stays where it is until the copy: nothing else
    // lands in the scratch meanwhile.
    start_over(&mut f, area, arena);
    f.instruction(&Instruction::Block(BlockType::Empty));
    f.instruction(&Instruction::LocalGet(address));
    f.instruction(&Instruction::I32Const(start as i32));
    f.instruction(&Instruction::I32LtU);
    f.instruction(&Instruction::BrIf(0));
    f.instruction(&Instruction::LocalGet(address));
    f.instruction(&Instruction::I64ExtendI32U);
    f.instruction(&Instruction::LocalGet(len));
    f.instruction(&Instruction::I64ExtendI32U);
    f.instruction(&Instruction::I64Add);
    f.instruction(&Instruction::I64Const(i64::from(end)));
    f.instruction(&Instruction::I64GtU);
    f.instruction(&Instruction::BrIf(0));
    f.instruction(&Instruction::LocalGet(address));
    f.instruction(&Instruction::LocalGet(ptr));
    f.instruction(&Instruction::LocalGet(len));
    f.instruction(&Instruction::MemoryCopy {
        src_mem: 0,
        dst_mem: 0,
    });
    write_ok(&mut f, area, None);
    f.instruction(&Instruction::I32Const(area.ret as i32));
    f.instruction(&Instruction::Return);
    f.instruction(&Instruction::End);
    write_err_case(&mut f, area, OUTSIDE_VARIABLES);
    f.instruction(&Instruction::I32Const(area.ret as i32));
    f.instruction(&Instruction::End);
    f
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
/// written at the return area as `raised` says, whose address the function
/// returns.
pub(crate) fn catching(
    f: &mut Function,
    area: &ControlArea,
    caught: Option<(u32, u32)>,
    stash: (u32, u32),
    raised: Raised,
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
    match raised {
        Raised::Message { payload } => write_err(f, area, payload, ptr, len),
        Raised::Case(case) => {
            write_err_case(f, area, case);
            for (offset, local) in [(PAYLOAD + 4, ptr), (PAYLOAD + 8, len)] {
                f.instruction(&Instruction::I32Const(area.ret as i32));
                f.instruction(&Instruction::LocalGet(local));
                f.instruction(&Instruction::I32Store(mem_arg(offset, 2)));
            }
        }
    }
    f.instruction(&Instruction::I32Const(area.ret as i32));
    f.instruction(&Instruction::End);
}

/// `Err` at the return area, of the case `case` of the error's variant: what
/// the case holds, the caller writes 4 bytes on.
fn write_err_case(f: &mut Function, area: &ControlArea, case: u8) {
    f.instruction(&Instruction::I32Const(area.ret as i32));
    f.instruction(&Instruction::I32Const(1));
    f.instruction(&Instruction::I32Store8(mem_arg(0, 0)));
    f.instruction(&Instruction::I32Const(area.ret as i32));
    f.instruction(&Instruction::I32Const(i32::from(case)));
    f.instruction(&Instruction::I32Store8(mem_arg(PAYLOAD, 0)));
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
