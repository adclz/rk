// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! The `{export}` FUNCTIONs as a component exports them: each under its
//! WIT name, in one interface named after the project, in the shapes the
//! component model's canonical ABI gives them.
//!
//! ```wit
//! export plant: interface {
//!     scale: func(raw: s16, gain: f32) -> result<f32, string>;
//! }
//! ```
//!
//! A function's parameters are the FUNCTION's VAR_INPUTs, a STRING its
//! bytes and their count. It returns the address of its result: what the
//! FUNCTION returned, or the message of the exception that stopped it,
//! caught here as the control interface catches one ([`crate::control`]).
//! A FUNCTION with VAR_OUTPUTs returns a record of them, its return value
//! last under its own name: the FUNCTION writes each in a slot of the
//! control area, and each is written in the record from there.
//!
//! What a host passes is checked before the FUNCTION runs: a STRING longer
//! than its parameter is an error, and so is a value outside a subrange. A
//! STRING the FUNCTION returns is checked for UTF-8 and a CHAR for a
//! character, as what an import is passed is.

use mir::MirModule;
use mir::function::{MirFunction, MirParam, MirParamKind};
use mir::types::{MirElementary, MirType};
use wasm_encoder::{BlockType, Function, Instruction, ValType};

use crate::WasmGen;
use crate::canonical::{Canonical, canonical, record_layout, result_payload, store};
use crate::control::{ControlArea, catching, start_over, write_err_static};
use crate::emit_expr::mem_arg;

/// What a STRING too long for its parameter answers.
pub(crate) const TOO_LONG: &[u8] = b"a STRING passed by the host is longer than its parameter";

/// The bytes the STRINGs a host passes one `{export}` FUNCTION take, at
/// their parameters' capacities, for the FUNCTION that takes the most: what
/// the scratch holds without the memory growing.
pub(crate) fn strings_passed(module: &MirModule) -> u32 {
    module
        .functions
        .iter()
        .filter(|func| func.wit_export.is_some())
        .map(|func| {
            func.params
                .iter()
                .filter_map(|param| match param.ty {
                    MirType::String { capacity } => Some(capacity),
                    _ => None,
                })
                .sum()
        })
        .max()
        .unwrap_or(0)
}

/// The builtins the exports of `module` call: the checks of what a host
/// passes and of what it gets back.
pub(crate) fn builtins(module: &MirModule) -> Vec<&'static str> {
    let mut names = Vec::new();
    for func in module.functions.iter().filter(|f| f.wit_export.is_some()) {
        names.extend(func.params.iter().filter_map(|p| range_check(&p.ty)));
        for returned in outputs(func).into_iter().chain(func.return_type.as_ref()) {
            match returned {
                MirType::String { .. } => names.push("rk.utf8_check"),
                MirType::Elementary(MirElementary::Char) => names.push("rk.char_check"),
                _ => {}
            }
        }
    }
    names
}

/// The VAR_OUTPUTs of `func`, by the type each holds.
fn outputs(func: &MirFunction) -> Vec<&MirType> {
    func.params
        .iter()
        .filter(|param| param.kind == MirParamKind::Output)
        .map(|param| match &param.ty {
            MirType::Pointer(held) => held.as_ref(),
            other => other,
        })
        .collect()
}

/// What `func` returns to a host, as the canonical ABI lays it out: its
/// VAR_OUTPUTs then its return value, a record when it has VAR_OUTPUTs.
fn result_fields(func: &MirFunction) -> Vec<Canonical> {
    outputs(func)
        .into_iter()
        .chain(func.return_type.as_ref())
        .map(|ty| {
            canonical(ty).unwrap_or_else(|| {
                panic!(
                    "internal compiler error: an export returns a scalar or a STRING, not {ty:?}"
                )
            })
        })
        .collect()
}

/// The size of the largest result an `{export}` FUNCTION of `module`
/// writes, its tag and its payload, a message or what it returns.
pub(crate) fn results_size(module: &MirModule) -> u32 {
    module
        .functions
        .iter()
        .filter(|func| func.wit_export.is_some())
        .map(|func| {
            let (_, size, align) = record_layout(&result_fields(func));
            result_payload(align) as u32 + size.max(8)
        })
        .max()
        .unwrap_or(0)
}

/// The slot a VAR_OUTPUT of `ty` takes, where its FUNCTION writes it as it
/// holds it: a STRING its length and characters.
fn slot_size(ty: &MirType) -> u32 {
    ty.size_bytes().next_multiple_of(8)
}

/// The slots of the `{export}` FUNCTION of `module` with the most
/// VAR_OUTPUTs, by size.
pub(crate) fn output_slots_size(module: &MirModule) -> u32 {
    module
        .functions
        .iter()
        .filter(|func| func.wit_export.is_some())
        .map(|func| outputs(func).into_iter().map(slot_size).sum())
        .max()
        .unwrap_or(0)
}

/// The VAR_INPUTs of `func`: what a host passes it.
fn inputs(func: &MirFunction) -> Vec<MirParam> {
    func.params
        .iter()
        .filter(|param| param.kind == MirParamKind::Input)
        .cloned()
        .collect()
}

/// The builtin that checks a value of `ty` a host passes: a subrange's
/// bounds, which a call checks where it stores an argument.
fn range_check(ty: &MirType) -> Option<&'static str> {
    let MirType::Subrange(sub) = ty else {
        return None;
    };
    match (
        sub.base.is_float(),
        sub.base.is_64bit(),
        sub.base.is_signed(),
    ) {
        (true, _, _) => None,
        (false, false, true) => Some("rk.range_check_i32"),
        (false, false, false) => Some("rk.range_check_u32"),
        (false, true, true) => Some("rk.range_check_i64"),
        (false, true, false) => Some("rk.range_check_u64"),
    }
}

/// What a value of `ty` is held as: an enum as its storage, a subrange as
/// its base.
fn elementary(ty: &MirType) -> Option<MirElementary> {
    match ty {
        MirType::Elementary(elementary) => Some(*elementary),
        MirType::Enum(enumeration) => Some(enumeration.storage),
        MirType::Subrange(subrange) => Some(subrange.base),
        _ => None,
    }
}

impl WasmGen<'_> {
    /// Emits a function for each `{export}` FUNCTION, exported as
    /// `<interface>#<name>`, the name a component takes it by.
    pub(crate) fn emit_exports(
        &mut self,
        area: &ControlArea,
        caught: Option<(u32, u32)>,
        arena: u32,
    ) {
        let module = self.module;
        let interface = hir::hir_ty::exports::export_interface(self.db);
        for func in &module.functions {
            let (Some(name), Some(&callee)) = (&func.wit_export, self.index_remap.get(&func.index))
            else {
                continue;
            };
            let (params, _) = crate::build_signature(&inputs(func), &None);
            let f = self.export_function(area, caught, arena, func, callee, &params);
            let index = self.add_function(&params, &[ValType::I32], &f);
            let export = format!("{interface}#{name}");
            self.export_section
                .export(&export, wasm_encoder::ExportKind::Func, index);
            self.control_names.push((index, export));
        }
    }

    /// The function a host calls `func` through, `callee`: its parameters
    /// checked, the FUNCTION called, and its result or what it raised
    /// written at the return area, whose address is returned.
    fn export_function(
        &self,
        area: &ControlArea,
        caught: Option<(u32, u32)>,
        arena: u32,
        func: &MirFunction,
        callee: u32,
        params: &[ValType],
    ) -> Function {
        let builtin = |name: &str| {
            self.builtin_indices
                .get(name)
                .copied()
                .unwrap_or_else(|| panic!("internal compiler error: `{name}` was not grafted"))
        };
        // Past the parameters: the caught message, then what was returned.
        let returned: Vec<ValType> = match &func.return_type {
            None => Vec::new(),
            Some(MirType::String { .. }) => vec![ValType::I32, ValType::I32],
            Some(ty) => vec![crate::mir_type_to_val_type(ty).unwrap_or(ValType::I32)],
        };
        let mut locals = vec![(2, ValType::I32)];
        locals.extend(returned.iter().map(|ty| (1, *ty)));
        let mut f = Function::new(locals);
        let count = params.len() as u32;
        let (message, result) = ((count, count + 1), count + 2);
        let scalar = func.return_type.as_ref().and_then(elementary);
        // The payload past the tag, as aligned as what it holds; then each
        // VAR_OUTPUT, and the return value last.
        let (fields, _, align) = record_layout(&result_fields(func));
        let payload = result_payload(align);
        let outputs = outputs(func);
        let returned_at = payload + u64::from(fields.last().copied().unwrap_or(0));

        // Only the host calls it: the stack of recursive calls starts
        // empty, even after an exception left frames behind, as it does
        // where a host enters a PROGRAM. The FUNCTION itself cannot say so,
        // the program may call it too.
        if let Some(stack) = &self.stack {
            f.instruction(&Instruction::I32Const(stack.base as i32));
            f.instruction(&Instruction::GlobalSet(stack.sp_global));
        }
        if func
            .params
            .iter()
            .any(|param| matches!(param.ty, MirType::String { .. }))
        {
            start_over(&mut f, area, arena);
        }
        let raised = crate::control::Raised::Message { payload };
        catching(&mut f, area, caught, message, raised, |f| {
            let mut local = 0;
            for param in &inputs(func) {
                match &param.ty {
                    MirType::String { capacity } => {
                        f.instruction(&Instruction::LocalGet(local + 1));
                        f.instruction(&Instruction::I32Const(*capacity as i32));
                        f.instruction(&Instruction::I32GtU);
                        f.instruction(&Instruction::If(BlockType::Empty));
                        write_err_static(f, area, payload, area.too_long, TOO_LONG.len() as u32);
                        f.instruction(&Instruction::I32Const(area.ret as i32));
                        f.instruction(&Instruction::Return);
                        f.instruction(&Instruction::End);
                        local += 2;
                    }
                    MirType::Subrange(sub) => {
                        if let Some(check) = range_check(&param.ty) {
                            f.instruction(&Instruction::LocalGet(local));
                            for bound in [sub.lower, sub.upper] {
                                f.instruction(&match sub.base.is_64bit() {
                                    true => Instruction::I64Const(bound),
                                    false => Instruction::I32Const(bound as i32),
                                });
                            }
                            f.instruction(&Instruction::Call(builtin(check)));
                            f.instruction(&Instruction::LocalSet(local));
                        }
                        local += 1;
                    }
                    _ => local += 1,
                }
            }
            // The FUNCTION's own parameters: a VAR_INPUT from the host, a
            // VAR_OUTPUT the slot it is written in.
            let mut local = 0;
            let mut slot = area.outputs;
            let mut slots = Vec::new();
            for param in &func.params {
                match &param.ty {
                    MirType::Pointer(held) if param.kind == MirParamKind::Output => {
                        f.instruction(&Instruction::I32Const(slot as i32));
                        if let MirType::String { capacity } = held.as_ref() {
                            f.instruction(&Instruction::I32Const(*capacity as i32));
                        }
                        slots.push(slot);
                        slot += slot_size(held);
                    }
                    MirType::String { .. } => {
                        f.instruction(&Instruction::LocalGet(local));
                        f.instruction(&Instruction::LocalGet(local + 1));
                        local += 2;
                    }
                    _ => {
                        f.instruction(&Instruction::LocalGet(local));
                        local += 1;
                    }
                }
            }
            f.instruction(&Instruction::Call(callee));

            // What it returned waits in its locals while each VAR_OUTPUT is
            // written in the record, checked as a returned value is.
            match &func.return_type {
                Some(MirType::String { .. }) => {
                    f.instruction(&Instruction::LocalSet(result + 1));
                    f.instruction(&Instruction::LocalSet(result));
                }
                Some(_) => {
                    f.instruction(&Instruction::LocalSet(result));
                }
                None => {}
            }
            for ((at, held), offset) in slots.iter().zip(&outputs).zip(&fields) {
                let offset = payload + u64::from(*offset);
                match held {
                    MirType::String { .. } => {
                        // Its characters past the length, and the length.
                        f.instruction(&Instruction::I32Const(*at as i32 + 4));
                        f.instruction(&Instruction::I32Const(*at as i32));
                        f.instruction(&Instruction::I32Load(mem_arg(0, 2)));
                        f.instruction(&Instruction::Call(builtin("rk.utf8_check")));
                        f.instruction(&Instruction::I32Const(area.ret as i32));
                        f.instruction(&Instruction::I32Const(*at as i32 + 4));
                        f.instruction(&Instruction::I32Store(mem_arg(offset, 2)));
                        f.instruction(&Instruction::I32Const(area.ret as i32));
                        f.instruction(&Instruction::I32Const(*at as i32));
                        f.instruction(&Instruction::I32Load(mem_arg(0, 2)));
                        f.instruction(&Instruction::I32Store(mem_arg(offset + 4, 2)));
                    }
                    held => {
                        let Some(elementary) = elementary(held) else {
                            continue;
                        };
                        if elementary == MirElementary::Char {
                            f.instruction(&Instruction::I32Const(*at as i32));
                            crate::emit_expr::emit_typed_mem_load(f, held);
                            f.instruction(&Instruction::Call(builtin("rk.char_check")));
                        }
                        f.instruction(&Instruction::I32Const(area.ret as i32));
                        f.instruction(&Instruction::I32Const(*at as i32));
                        crate::emit_expr::emit_typed_mem_load(f, held);
                        if elementary == MirElementary::Bool {
                            f.instruction(&Instruction::I32Const(0));
                            f.instruction(&Instruction::I32Ne);
                        }
                        f.instruction(&store(elementary, offset));
                    }
                }
            }

            match (&func.return_type, scalar) {
                (Some(MirType::String { .. }), _) => {
                    f.instruction(&Instruction::LocalGet(result));
                    f.instruction(&Instruction::LocalGet(result + 1));
                    f.instruction(&Instruction::Call(builtin("rk.utf8_check")));
                    for (offset, part) in [(returned_at, result), (returned_at + 4, result + 1)] {
                        f.instruction(&Instruction::I32Const(area.ret as i32));
                        f.instruction(&Instruction::LocalGet(part));
                        f.instruction(&Instruction::I32Store(mem_arg(offset, 2)));
                    }
                }
                (Some(_), Some(scalar)) => {
                    if scalar == MirElementary::Char {
                        f.instruction(&Instruction::LocalGet(result));
                        f.instruction(&Instruction::Call(builtin("rk.char_check")));
                    }
                    f.instruction(&Instruction::I32Const(area.ret as i32));
                    f.instruction(&Instruction::LocalGet(result));
                    if scalar == MirElementary::Bool {
                        // One byte, 0 or 1, whatever the lane held.
                        f.instruction(&Instruction::I32Const(0));
                        f.instruction(&Instruction::I32Ne);
                    }
                    f.instruction(&store(scalar, returned_at));
                }
                _ => {}
            }
            f.instruction(&Instruction::I32Const(area.ret as i32));
            f.instruction(&Instruction::I32Const(0));
            f.instruction(&Instruction::I32Store8(mem_arg(0, 0)));
            f.instruction(&Instruction::I32Const(area.ret as i32));
            f.instruction(&Instruction::Return);
        });
        f
    }
}
