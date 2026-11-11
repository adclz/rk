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
//! A function's parameters are the FUNCTION's own, a STRING its bytes and
//! their count. It returns the address of its result: what the FUNCTION
//! returned, or the message of the exception that stopped it, caught here
//! as the control interface catches one ([`crate::control`]).
//!
//! What a host passes is checked before the FUNCTION runs: a STRING longer
//! than its parameter is an error, and so is a value outside a subrange. A
//! STRING the FUNCTION returns is checked for UTF-8 and a CHAR for a
//! character, as what an import is passed is.

use mir::MirModule;
use mir::function::MirFunction;
use mir::types::{MirElementary, MirType};
use wasm_encoder::{BlockType, Function, Instruction, ValType};

use crate::WasmGen;
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
        match &func.return_type {
            Some(MirType::String { .. }) => names.push("rk.utf8_check"),
            Some(MirType::Elementary(MirElementary::Char)) => names.push("rk.char_check"),
            _ => {}
        }
    }
    names
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

/// The store of a value of `elementary` at the return area, `payload`
/// bytes in, as the canonical ABI lays it out: a BOOL and an 8-bit integer
/// in one byte, a 16-bit one in two.
fn store(elementary: MirElementary, payload: u64) -> Instruction<'static> {
    use MirElementary::*;
    match elementary {
        Bool | SInt | USInt | Byte => Instruction::I32Store8(mem_arg(payload, 0)),
        Int | UInt | Word => Instruction::I32Store16(mem_arg(payload, 1)),
        Real => Instruction::F32Store(mem_arg(payload, 2)),
        LReal => Instruction::F64Store(mem_arg(payload, 3)),
        wide if wide.is_64bit() => Instruction::I64Store(mem_arg(payload, 3)),
        _ => Instruction::I32Store(mem_arg(payload, 2)),
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
            let (params, _) = crate::build_signature(&func.params, &None);
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
        // A 64-bit value sets the payload 8 bytes in, any other 4.
        let scalar = func.return_type.as_ref().and_then(elementary);
        let payload = match scalar {
            Some(scalar) if scalar.is_64bit() => 8,
            _ => crate::control::PAYLOAD,
        };

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
            for param in &func.params {
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
            for local in 0..count {
                f.instruction(&Instruction::LocalGet(local));
            }
            f.instruction(&Instruction::Call(callee));

            match (&func.return_type, scalar) {
                (Some(MirType::String { .. }), _) => {
                    f.instruction(&Instruction::LocalSet(result + 1));
                    f.instruction(&Instruction::LocalSet(result));
                    f.instruction(&Instruction::LocalGet(result));
                    f.instruction(&Instruction::LocalGet(result + 1));
                    f.instruction(&Instruction::Call(builtin("rk.utf8_check")));
                    for (offset, part) in [(payload, result), (payload + 4, result + 1)] {
                        f.instruction(&Instruction::I32Const(area.ret as i32));
                        f.instruction(&Instruction::LocalGet(part));
                        f.instruction(&Instruction::I32Store(mem_arg(offset, 2)));
                    }
                }
                (Some(_), Some(scalar)) => {
                    f.instruction(&Instruction::LocalSet(result));
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
                    f.instruction(&store(scalar, payload));
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
