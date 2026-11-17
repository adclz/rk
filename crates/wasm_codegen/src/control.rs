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
//!     enum test-state { passed, running }
//!     run-test: func(test: u32) -> result<test-state, test-error>;
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
/// areas: a return area, the message, and the scratch what a host passes
/// and what an import gives land in. Past them, where an import's record is
/// written, and where an `{export}` FUNCTION's VAR_OUTPUTs land before they
/// are written in its record.
pub(crate) struct ControlArea {
    /// Where a result is written: a tag at 0, then its payload, at 4 or,
    /// holding a 64-bit value, at 8. 16 bytes at least, more for the record
    /// of an `{export}` FUNCTION with VAR_OUTPUTs.
    pub ret: u32,
    pub too_long: u32,
    /// Where what a host passes lands (`cabi_realloc`), one block after the
    /// other from the start: each function a host calls starts it over, and
    /// so does each call of an import that gives a string or a list. As large
    /// as what one call places and reads (the inputs or the retain data the
    /// control interface takes, the STRINGs one `{export}` FUNCTION does, the
    /// elements of a list an import gives), then [`Self::refused`]. The
    /// memory never grows: a block past it traps.
    pub scratch: u32,
    pub scratch_size: u32,
    /// The end of the scratch, where a block nobody reads lands: a string
    /// longer than its STRING, a list longer than its arrays, what an import
    /// gives that no call keeps. Each such block of a call lands there, on
    /// the one before it. As long as the largest buffer a host writes into,
    /// and a page more.
    pub refused: u32,
    pub refused_size: u32,
    /// Where an import that returns a record of two fields or more has it
    /// written ([`crate::canonical`]).
    pub import_record: u32,
    /// Where an `{export}` FUNCTION's VAR_OUTPUTs are written by the
    /// FUNCTION, one slot each ([`crate::exports`]).
    pub outputs: u32,
    /// Where the host's next string goes, when an import gives one
    /// ([`HOST_STRING_AT`]): four words an import's wrapper sets around
    /// its call and `cabi_import_realloc` reads.
    pub host_string: Option<u32>,
    /// Where the host's next blocks go, when an import gives a list: the
    /// state its wrapper writes before the call, of [`LIST_HEADER`] words
    /// and [`LIST_FIELD`] more for each field of an element, which the
    /// builtin bundle reads (`wasm_builtins::host`).
    pub host_list: Option<u32>,
    /// The first byte past the area.
    pub end: u32,
}

/// The words of [`ControlArea::host_string`], by their offset: the STRING
/// the host's next string goes into, 0 outside a call that gives one; how
/// many bytes that STRING holds; what became of the string, 0 while none
/// came, [`LANDED`] once it is in the STRING, [`TOO_LONG_FOR_IT`] when it
/// was longer; and then how long it was, which a text that is cut to fit
/// is copied from ([`ControlArea::refused`], where it landed).
pub(crate) const HOST_STRING_AT: u64 = 0;
pub(crate) const HOST_STRING_CAPACITY: u64 = 4;
pub(crate) const HOST_STRING_LANDED: u64 = 8;
pub(crate) const HOST_STRING_LENGTH: u64 = 12;
const LANDED: i32 = 1;
const TOO_LONG_FOR_IT: i32 = 2;

/// The words of [`ControlArea::host_list`], as `wasm_builtins::host` reads
/// them: whether a call that gives a list runs, an element's size, how many
/// fields it has and how many of them are strings, then what the bundle
/// keeps, zero before the call, through the tenth word, then where a block
/// nobody reads goes and how long it may be ([`ControlArea::refused`]);
/// each field's words follow, whether it is a string, its offset and size
/// in an element, then its array's address, stride, STRING capacity and
/// length.
pub(crate) const LIST_ARMED: u32 = 0;
pub(crate) const LIST_ELEMENT_SIZE: u32 = 1;
pub(crate) const LIST_FIELDS: u32 = 2;
pub(crate) const LIST_STRINGS: u32 = 3;
pub(crate) const LIST_REFUSED_AT: u32 = 10;
pub(crate) const LIST_REFUSED_SIZE: u32 = 11;
pub(crate) const LIST_HEADER: u32 = 12;
pub(crate) const LIST_FIELD: u32 = 7;

/// Whether `ext` gives a string, which the host writes into its STRING
/// `VAR_IN_OUT`, or a text, into its STRING `VAR_OUTPUT`.
pub(crate) fn gives_string(ext: &mir::function::MirExternFunction) -> bool {
    ext.params.iter().any(|param| {
        matches!(
            param.kind,
            mir::function::MirParamKind::InOut | mir::function::MirParamKind::Output
        ) && matches!(&param.ty, mir::types::MirType::Pointer(inner)
                if matches!(inner.as_ref(), mir::types::MirType::String { .. }))
    })
}

/// Whether what `ext` gives is a text, an error's or a case's, which is
/// cut to fit its STRING rather than refused when longer.
pub(crate) fn cuts_text(ext: &mir::function::MirExternFunction) -> bool {
    ext.params
        .iter()
        .any(|param| param.kind == mir::function::MirParamKind::Output)
}

/// The elements of the arrays `ext` gives a list into, its `ARRAY[*]`
/// `VAR_IN_OUT`s, one per field of an element.
pub(crate) fn list_fields(ext: &mir::function::MirExternFunction) -> Vec<&mir::types::MirType> {
    ext.params
        .iter()
        .filter(|param| param.kind == mir::function::MirParamKind::InOut)
        .filter_map(|param| match &param.ty {
            mir::types::MirType::Pointer(inner) => match inner.as_ref() {
                mir::types::MirType::Array(array) => Some(array.element_type.as_ref()),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

/// How an element of the list `ext` gives is laid out, as the canonical ABI
/// lays out the record of its fields, one per array: each field's place
/// and offset, and the element's size. `None` when it gives no list.
pub(crate) fn list_layout(
    ext: &mir::function::MirExternFunction,
) -> Option<(Vec<crate::canonical::Canonical>, Vec<u32>, u32)> {
    let elements = list_fields(ext);
    if elements.is_empty() {
        return None;
    }
    let fields: Vec<crate::canonical::Canonical> = elements
        .iter()
        .map(|element| {
            crate::canonical::canonical(element).unwrap_or_else(|| {
                panic!(
                    "internal compiler error: a list's element is a scalar or a STRING \
                     (E1502): {element:?}"
                )
            })
        })
        .collect();
    let (offsets, size, _) = crate::canonical::record_layout(&fields);
    Some((fields, offsets, size))
}

/// The bytes of the elements of the longest list a call keeps of what an
/// import gives, as the host writes them.
fn list_elements(module: &MirModule) -> u64 {
    module
        .extern_functions
        .iter()
        .zip(&module.longest_lists)
        .filter_map(|(ext, longest)| {
            list_layout(ext).map(|(_, _, size)| u64::from(size) * u64::from(*longest))
        })
        .max()
        .unwrap_or(0)
}

/// The control area of `module`, which starts at `base`: where its `{test}`
/// result areas end.
pub(crate) fn control_area(module: &MirModule, base: u32) -> ControlArea {
    let ret = base.next_multiple_of(8);
    let too_long = ret + crate::exports::results_size(module).max(16);
    // A list of bytes and a string ask for an alignment of 1, a list of
    // ranges 4; 8 serves any.
    let scratch = (too_long + crate::exports::TOO_LONG.len() as u32).next_multiple_of(8);
    let lists = list_elements(module);
    let placed = u64::from(
        module
            .input_size
            .max(module.retain_size)
            .max(crate::exports::strings_passed(module)),
    )
    .max(lists);
    let narrow = |bytes: u64| u32::try_from(bytes).unwrap_or(u32::MAX);
    let refused = narrow((u64::from(scratch) + placed).next_multiple_of(8));
    let refused_size =
        narrow(u64::from(module.largest_string).max(lists) + u64::from(crate::WASM_PAGE));
    let scratch_size = narrow(u64::from(refused) + u64::from(refused_size) - u64::from(scratch));
    let import_record = (scratch + scratch_size).next_multiple_of(8);
    let outputs = (import_record + import_records_size(module)).next_multiple_of(8);
    let end = outputs + crate::exports::output_slots_size(module);
    // Past the rest, so that a module no import gives a string to is laid
    // out as before.
    let host_string = module
        .extern_functions
        .iter()
        .any(gives_string)
        .then_some(end.next_multiple_of(4));
    let end = host_string.map_or(end, |at| at + 16);
    let fields = module
        .extern_functions
        .iter()
        .map(|ext| list_fields(ext).len() as u32)
        .max()
        .unwrap_or(0);
    let host_list = (fields > 0).then_some(end.next_multiple_of(4));
    let end = host_list.map_or(end, |at| at + 4 * (LIST_HEADER + LIST_FIELD * fields));
    ControlArea {
        ret,
        too_long,
        scratch,
        scratch_size,
        refused,
        refused_size,
        import_record,
        outputs,
        host_string,
        host_list,
        end,
    }
}

/// The largest record an import of `module` returns: its VAR_OUTPUTs and
/// its return value, when it has two fields or more.
fn import_records_size(module: &MirModule) -> u32 {
    module
        .extern_functions
        .iter()
        .filter_map(|ext| {
            let fields: Vec<_> = ext
                .results()
                .filter_map(crate::canonical::canonical)
                .collect();
            (fields.len() > 1).then(|| crate::canonical::record_layout(&fields).1)
        })
        .max()
        .unwrap_or(0)
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

        let arena = self.arena;

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

        let f = control_read_memory(area, arena);
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

        let f = control_realloc(area, arena);
        let index = self.add_function(&[ValType::I32; 4], &[ValType::I32], &f);
        self.export_section
            .export("cabi_realloc", wasm_encoder::ExportKind::Func, index);
        self.control_names.push((index, "cabi_realloc".to_string()));

        // What an import gives lands through a realloc of its own, which
        // the component takes for every import when the module has one.
        if !self.module.extern_functions.is_empty() {
            let list = area.host_list.map(|state| {
                let builtin = |name: &str| {
                    self.builtin_indices.get(name).copied().unwrap_or_else(|| {
                        panic!("internal compiler error: `{name}` was not grafted")
                    })
                };
                (state, builtin(LIST_BLOCK), builtin(LIST_PLACED))
            });
            let f = import_realloc(area, list, index);
            let index = self.add_function(&[ValType::I32; 4], &[ValType::I32], &f);
            self.export_section.export(
                "cabi_import_realloc",
                wasm_encoder::ExportKind::Func,
                index,
            );
            self.control_names
                .push((index, "cabi_import_realloc".to_string()));
        }
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
    /// wrapper catches what it raises into a `result<test-state, string>` of
    /// its own, read back here into the interface's: a test FUNCTION whole,
    /// or a cycle of a test FUNCTION_BLOCK, `running` until its DONE is TRUE.
    /// A release runs none: its tests are left for the optimizer to remove.
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
                // The test's `test-state`, a byte.
                f.instruction(&Instruction::I32Const(area.ret as i32));
                f.instruction(&Instruction::LocalGet(result));
                f.instruction(&Instruction::I32Load8U(mem_arg(4, 0)));
                f.instruction(&Instruction::I32Store8(mem_arg(PAYLOAD, 0)));
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
/// The list goes in the scratch, after the ranges when they are there: one
/// that does not fit is `too-large`, the memory never growing. The buffers
/// that follow the scratch are the outputs', which a pending write may
/// still hold.
fn control_read_memory(area: &ControlArea, arena: u32) -> Function {
    // Params: the ranges 0 and their count 1; locals: the range at hand 2,
    // its address 3 and size 4, where the list goes 5, where the next bytes
    // go 6, the error's case 7, and in 64 bits the list's size 8 and where
    // the ranges end 9.
    let mut f = Function::new([(6, ValType::I32), (2, ValType::I64)]);
    let (ptr, count) = (0, 1);
    let (i, address, size, out, cursor, why) = (2, 3, 4, 5, 6, 7);
    let (total, ranges_end) = (8, 9);
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

    // Where the list goes: after the ranges when they are in the scratch,
    // at its start otherwise.
    f.instruction(&Instruction::LocalGet(ptr));
    f.instruction(&Instruction::I64ExtendI32U);
    f.instruction(&Instruction::LocalGet(count));
    f.instruction(&Instruction::I64ExtendI32U);
    f.instruction(&Instruction::I64Const(3));
    f.instruction(&Instruction::I64Shl);
    f.instruction(&Instruction::I64Add);
    f.instruction(&Instruction::I64Const(i64::from(area.scratch)));
    f.instruction(&Instruction::LocalGet(ptr));
    f.instruction(&Instruction::I32Const(area.scratch as i32));
    f.instruction(&Instruction::I32GeU);
    f.instruction(&Instruction::Select);
    f.instruction(&Instruction::LocalTee(ranges_end));
    f.instruction(&Instruction::I32WrapI64);
    f.instruction(&Instruction::LocalSet(out));
    // Too large when it ends past the scratch.
    why_is(&mut f, TOO_LARGE);
    f.instruction(&Instruction::LocalGet(ranges_end));
    f.instruction(&Instruction::LocalGet(total));
    f.instruction(&Instruction::I64Add);
    f.instruction(&Instruction::I64Const(scratch_end as i64));
    f.instruction(&Instruction::I64GtU);
    f.instruction(&Instruction::BrIf(0));

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
/// The scratch holds what one call places and a page more
/// ([`ControlArea::refused`]): what is longer, the function it is passed to
/// would cut or refuse, and it traps instead, the memory never growing.
fn control_realloc(area: &ControlArea, arena: u32) -> Function {
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
    f.instruction(&Instruction::I32Eqz);
    f.instruction(&Instruction::If(BlockType::Empty));
    f.instruction(&Instruction::Unreachable);
    f.instruction(&Instruction::End);
    f.instruction(&Instruction::LocalGet(at));
    f.instruction(&Instruction::LocalGet(size));
    f.instruction(&Instruction::I32Add);
    f.instruction(&Instruction::GlobalSet(arena));
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

/// `cabi_import_realloc`, where what an import gives lands. Inside a call
/// that gives a string, the first block the host asks for, a string's, is
/// the STRING the call passed for it: the host writes the bytes there, and
/// the block's size is the STRING's length. A string longer than the STRING
/// lands with the blocks nobody reads, and how long it is is kept: once the
/// host returns, the call raises, or for a text copies the first bytes that
/// fit into the STRING.
///
/// The host asks for a string's bytes once, at their exact size and with an
/// alignment of 1, and never grows the block.
///
/// Inside a call that gives a list, `list` (the state, and the bundle's
/// functions that read it) says where each block goes: the bundle answers,
/// and the block of the elements it leaves goes where `cabi_realloc` puts
/// it, which it is told.
///
/// Every block nobody reads, a string or a list refused and what an import
/// gives that no call keeps, lands at [`ControlArea::refused`], on the one
/// before it; one longer than that place traps.
fn import_realloc(
    area: &ControlArea,
    list: Option<(u32, u32, u32)>,
    cabi_realloc: u32,
) -> Function {
    // Params: the old block 0 and its size 1, the alignment 2, the size 3;
    // local: where a block goes 4.
    let mut f = Function::new([(1, ValType::I32)]);
    let (old, old_size, align, size, at) = (0, 1, 2, 3, 4);
    let fallback = |f: &mut Function| {
        f.instruction(&Instruction::LocalGet(old));
        f.instruction(&Instruction::LocalGet(old_size));
        f.instruction(&Instruction::LocalGet(align));
        f.instruction(&Instruction::LocalGet(size));
        f.instruction(&Instruction::Call(cabi_realloc));
    };
    let refuse = |f: &mut Function| {
        f.instruction(&Instruction::LocalGet(size));
        f.instruction(&Instruction::I32Const(area.refused_size as i32));
        f.instruction(&Instruction::I32GtU);
        f.instruction(&Instruction::If(BlockType::Empty));
        f.instruction(&Instruction::Unreachable);
        f.instruction(&Instruction::End);
        f.instruction(&Instruction::I32Const(area.refused as i32));
        f.instruction(&Instruction::Return);
    };
    // Any other new block: nobody reads it.
    let refuse_new = |f: &mut Function| {
        f.instruction(&Instruction::LocalGet(old));
        f.instruction(&Instruction::I32Eqz);
        f.instruction(&Instruction::If(BlockType::Empty));
        refuse(f);
        f.instruction(&Instruction::End);
        fallback(f);
        f.instruction(&Instruction::End);
    };
    if let Some((state, block, placed)) = list {
        // A call that gives a list runs, and this is a new block: one the
        // host grows keeps where `cabi_realloc` put it.
        f.instruction(&Instruction::I32Const(state as i32));
        f.instruction(&Instruction::I32Load(mem_arg(4 * u64::from(LIST_ARMED), 2)));
        f.instruction(&Instruction::LocalGet(old));
        f.instruction(&Instruction::I32Eqz);
        f.instruction(&Instruction::I32And);
        f.instruction(&Instruction::If(BlockType::Empty));
        f.instruction(&Instruction::I32Const(state as i32));
        f.instruction(&Instruction::LocalGet(size));
        f.instruction(&Instruction::Call(block));
        f.instruction(&Instruction::LocalTee(at));
        f.instruction(&Instruction::If(BlockType::Empty));
        f.instruction(&Instruction::LocalGet(at));
        f.instruction(&Instruction::Return);
        f.instruction(&Instruction::End);
        fallback(&mut f);
        f.instruction(&Instruction::LocalSet(at));
        f.instruction(&Instruction::I32Const(state as i32));
        f.instruction(&Instruction::LocalGet(at));
        f.instruction(&Instruction::Call(placed));
        f.instruction(&Instruction::LocalGet(at));
        f.instruction(&Instruction::Return);
        f.instruction(&Instruction::End);
    }
    let Some(host_string) = area.host_string else {
        refuse_new(&mut f);
        return f;
    };
    let word = |f: &mut Function, offset: u64| {
        f.instruction(&Instruction::I32Const(host_string as i32));
        f.instruction(&Instruction::I32Load(mem_arg(offset, 2)));
    };
    let set_landed = |f: &mut Function, landed: i32| {
        f.instruction(&Instruction::I32Const(host_string as i32));
        f.instruction(&Instruction::I32Const(landed));
        f.instruction(&Instruction::I32Store(mem_arg(HOST_STRING_LANDED, 2)));
    };
    // A STRING waits, none came yet, and this is a new block of bytes.
    word(&mut f, HOST_STRING_AT);
    f.instruction(&Instruction::I32Const(0));
    f.instruction(&Instruction::I32Ne);
    word(&mut f, HOST_STRING_LANDED);
    f.instruction(&Instruction::I32Eqz);
    f.instruction(&Instruction::I32And);
    f.instruction(&Instruction::LocalGet(old));
    f.instruction(&Instruction::I32Eqz);
    f.instruction(&Instruction::I32And);
    f.instruction(&Instruction::LocalGet(align));
    f.instruction(&Instruction::I32Const(1));
    f.instruction(&Instruction::I32Eq);
    f.instruction(&Instruction::I32And);
    f.instruction(&Instruction::If(BlockType::Empty));
    f.instruction(&Instruction::LocalGet(size));
    word(&mut f, HOST_STRING_CAPACITY);
    f.instruction(&Instruction::I32LeU);
    f.instruction(&Instruction::If(BlockType::Empty));
    // Its length, then where its bytes go.
    word(&mut f, HOST_STRING_AT);
    f.instruction(&Instruction::LocalGet(size));
    f.instruction(&Instruction::I32Store(mem_arg(0, 2)));
    set_landed(&mut f, LANDED);
    word(&mut f, HOST_STRING_AT);
    f.instruction(&Instruction::I32Const(4));
    f.instruction(&Instruction::I32Add);
    f.instruction(&Instruction::Return);
    f.instruction(&Instruction::Else);
    set_landed(&mut f, TOO_LONG_FOR_IT);
    // How long it is: a text cut to fit is copied from where it lands.
    f.instruction(&Instruction::I32Const(host_string as i32));
    f.instruction(&Instruction::LocalGet(size));
    f.instruction(&Instruction::I32Store(mem_arg(HOST_STRING_LENGTH, 2)));
    f.instruction(&Instruction::End);
    f.instruction(&Instruction::End);
    refuse_new(&mut f);
    f
}

/// The bundle's functions that place and check a list's blocks.
pub(crate) const LIST_BLOCK: &str = "rk.host_list_block";
pub(crate) const LIST_PLACED: &str = "rk.host_list_placed";
pub(crate) const LIST_END: &str = "rk.host_list_end";

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
