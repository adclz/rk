//! Emit WASM instructions from MIR statements.
//! Purely mechanical — reads structured control flow and emits WASM blocks.

use hir::hir_def::interned::identifier::Ident;
use mir::{
    expr::{MirConstant, MirExpr},
    stmt::{MirCasePattern, MirStmt},
    types::MirElementary,
};
use rustc_hash::FxHashMap;
use wasm_encoder::{BlockType, Instruction, MemArg};

use super::{LocalInfo, emit_expr::{emit_addr_of, emit_expr}};

/// Context for statement emission.
struct Ctx<'a> {
    locals: &'a FxHashMap<Ident, LocalInfo>,
    fn_indices: &'a FxHashMap<Ident, u32>,
    return_local: Option<u32>,
}

/// Emit a list of MIR statements with return local context.
pub(crate) fn emit_stmts_with_return(
    func: &mut wasm_encoder::Function,
    stmts: &[MirStmt],
    locals: &FxHashMap<Ident, LocalInfo>,
    fn_indices: &FxHashMap<Ident, u32>,
    return_local: Option<u32>,
) {
    let ctx = Ctx { locals, fn_indices, return_local };
    emit_stmts(func, stmts, &ctx);
}

fn emit_stmts(func: &mut wasm_encoder::Function, stmts: &[MirStmt], ctx: &Ctx) {
    for stmt in stmts {
        emit_stmt(func, stmt, ctx);
    }
}

fn emit_stmt(func: &mut wasm_encoder::Function, stmt: &MirStmt, ctx: &Ctx) {
    match stmt {
        MirStmt::Assign { target, value } => {
            emit_assignment(func, target, value, ctx);
        }

        MirStmt::Call(call) => {
            emit_expr(func, &MirExpr::Call(call.clone()), ctx.locals, ctx.fn_indices);
            if call.return_type != mir::types::MirType::Void {
                func.instruction(&Instruction::Drop);
            }
        }

        MirStmt::Return => {
            if let Some(ret_idx) = ctx.return_local {
                func.instruction(&Instruction::LocalGet(ret_idx));
            }
            func.instruction(&Instruction::Return);
        }

        MirStmt::If { condition, then_body, else_ifs, else_body } => {
            emit_expr(func, condition, ctx.locals, ctx.fn_indices);
            func.instruction(&Instruction::If(BlockType::Empty));
            emit_stmts(func, then_body, ctx);

            for (cond, body) in else_ifs {
                func.instruction(&Instruction::Else);
                emit_expr(func, cond, ctx.locals, ctx.fn_indices);
                func.instruction(&Instruction::If(BlockType::Empty));
                emit_stmts(func, body, ctx);
            }

            if let Some(else_stmts) = else_body {
                func.instruction(&Instruction::Else);
                emit_stmts(func, else_stmts, ctx);
            }

            func.instruction(&Instruction::End);
            for _ in else_ifs {
                func.instruction(&Instruction::End);
            }
        }

        MirStmt::Case { selector, arms, else_body } => {
            func.instruction(&Instruction::Block(BlockType::Empty));

            for arm in arms {
                // Evaluate all patterns and OR them together
                for (i, pattern) in arm.patterns.iter().enumerate() {
                    match pattern {
                        MirCasePattern::Value(val) => {
                            emit_expr(func, selector, ctx.locals, ctx.fn_indices);
                            emit_constant_expr(func, val);
                            func.instruction(&Instruction::I32Eq);
                        }
                        MirCasePattern::Range { lower, upper } => {
                            emit_expr(func, selector, ctx.locals, ctx.fn_indices);
                            emit_constant_expr(func, lower);
                            func.instruction(&Instruction::I32GeS);
                            emit_expr(func, selector, ctx.locals, ctx.fn_indices);
                            emit_constant_expr(func, upper);
                            func.instruction(&Instruction::I32LeS);
                            func.instruction(&Instruction::I32And);
                        }
                    }
                    // OR with previous pattern result (after second+ pattern)
                    if i > 0 {
                        func.instruction(&Instruction::I32Or);
                    }
                }

                func.instruction(&Instruction::If(BlockType::Empty));
                emit_stmts(func, &arm.body, ctx);
                func.instruction(&Instruction::Br(1)); // break out of outer block
                func.instruction(&Instruction::End);
            }

            if let Some(else_stmts) = else_body {
                emit_stmts(func, else_stmts, ctx);
            }

            func.instruction(&Instruction::End); // outer block
        }

        MirStmt::For { control_var, control_type, start, end, step, body } => {
            let ctrl_idx = match ctx.locals.get(control_var) {
                Some(LocalInfo::Scalar { index, .. }) => *index,
                _ => return,
            };

            // Initialize
            emit_expr(func, start, ctx.locals, ctx.fn_indices);
            func.instruction(&Instruction::LocalSet(ctrl_idx));

            func.instruction(&Instruction::Block(BlockType::Empty));
            func.instruction(&Instruction::Loop(BlockType::Empty));

            // Check bound
            func.instruction(&Instruction::LocalGet(ctrl_idx));
            emit_expr(func, end, ctx.locals, ctx.fn_indices);
            if control_type.is_signed() {
                func.instruction(&Instruction::I32GtS);
            } else {
                func.instruction(&Instruction::I32GtU);
            }
            func.instruction(&Instruction::BrIf(1));

            emit_stmts(func, body, ctx);

            // Increment
            func.instruction(&Instruction::LocalGet(ctrl_idx));
            emit_expr(func, step, ctx.locals, ctx.fn_indices);
            func.instruction(&Instruction::I32Add);
            func.instruction(&Instruction::LocalSet(ctrl_idx));

            func.instruction(&Instruction::Br(0));
            func.instruction(&Instruction::End); // loop
            func.instruction(&Instruction::End); // block
        }

        MirStmt::While { condition, body } => {
            func.instruction(&Instruction::Block(BlockType::Empty));
            func.instruction(&Instruction::Loop(BlockType::Empty));
            emit_expr(func, condition, ctx.locals, ctx.fn_indices);
            func.instruction(&Instruction::I32Eqz);
            func.instruction(&Instruction::BrIf(1));
            emit_stmts(func, body, ctx);
            func.instruction(&Instruction::Br(0));
            func.instruction(&Instruction::End);
            func.instruction(&Instruction::End);
        }

        MirStmt::Repeat { condition, body } => {
            func.instruction(&Instruction::Block(BlockType::Empty));
            func.instruction(&Instruction::Loop(BlockType::Empty));
            emit_stmts(func, body, ctx);
            emit_expr(func, condition, ctx.locals, ctx.fn_indices);
            func.instruction(&Instruction::BrIf(1));
            func.instruction(&Instruction::Br(0));
            func.instruction(&Instruction::End);
            func.instruction(&Instruction::End);
        }

        MirStmt::Exit => {
            func.instruction(&Instruction::Br(1));
        }

        MirStmt::Continue => {
            func.instruction(&Instruction::Br(0));
        }

        MirStmt::MemStore { offset, value } => {
            func.instruction(&Instruction::I32Const(*offset as i32));
            emit_constant_store(func, value);
        }

        MirStmt::DebugTrap { .. } => {}
    }
}

fn emit_assignment(
    func: &mut wasm_encoder::Function,
    target: &mir::expr::MirPlace,
    value: &MirExpr,
    ctx: &Ctx,
) {
    match target {
        mir::expr::MirPlace::Local(ident) => {
            if let Some(info) = ctx.locals.get(ident) {
                match info {
                    LocalInfo::Scalar { index, .. } => {
                        emit_expr(func, value, ctx.locals, ctx.fn_indices);
                        func.instruction(&Instruction::LocalSet(*index));
                    }
                    LocalInfo::Memory { address, size, align } => {
                        func.instruction(&Instruction::I32Const(*address as i32));
                        emit_expr(func, value, ctx.locals, ctx.fn_indices);
                        emit_mem_store(func, *size, *align);
                    }
                    LocalInfo::Pointer { index, pointee_elem } => {
                        func.instruction(&Instruction::LocalGet(*index));
                        emit_expr(func, value, ctx.locals, ctx.fn_indices);
                        if let Some(elem) = pointee_elem {
                            emit_typed_mem_store(func, &mir::types::MirType::Elementary(*elem));
                        } else {
                            emit_mem_store(func, 4, 4);
                        }
                    }
                }
            }
        }
        _ => {
            emit_addr_of(func, target, ctx.locals);
            emit_expr(func, value, ctx.locals, ctx.fn_indices);
            let ty = place_type(target);
            emit_typed_mem_store(func, &ty);
        }
    }
}

fn emit_constant_expr(func: &mut wasm_encoder::Function, c: &MirConstant) {
    match c {
        MirConstant::Bool(v) => { func.instruction(&Instruction::I32Const(if *v { 1 } else { 0 })); }
        MirConstant::I32(v) => { func.instruction(&Instruction::I32Const(*v)); }
        MirConstant::I64(v) => { func.instruction(&Instruction::I64Const(*v)); }
        MirConstant::F32(v) => { func.instruction(&Instruction::F32Const((*v).into())); }
        MirConstant::F64(v) => { func.instruction(&Instruction::F64Const((*v).into())); }
        MirConstant::Null => { func.instruction(&Instruction::I32Const(0)); }
    }
}

fn emit_constant_store(func: &mut wasm_encoder::Function, value: &MirConstant) {
    match value {
        MirConstant::Bool(v) => {
            func.instruction(&Instruction::I32Const(if *v { 1 } else { 0 }));
            func.instruction(&Instruction::I32Store(MemArg { offset: 0, align: 2, memory_index: 0 }));
        }
        MirConstant::I32(v) => {
            func.instruction(&Instruction::I32Const(*v));
            func.instruction(&Instruction::I32Store(MemArg { offset: 0, align: 2, memory_index: 0 }));
        }
        MirConstant::I64(v) => {
            func.instruction(&Instruction::I64Const(*v));
            func.instruction(&Instruction::I64Store(MemArg { offset: 0, align: 3, memory_index: 0 }));
        }
        MirConstant::F32(v) => {
            func.instruction(&Instruction::F32Const((*v).into()));
            func.instruction(&Instruction::F32Store(MemArg { offset: 0, align: 2, memory_index: 0 }));
        }
        MirConstant::F64(v) => {
            func.instruction(&Instruction::F64Const((*v).into()));
            func.instruction(&Instruction::F64Store(MemArg { offset: 0, align: 3, memory_index: 0 }));
        }
        MirConstant::Null => {
            func.instruction(&Instruction::I32Const(0));
            func.instruction(&Instruction::I32Store(MemArg { offset: 0, align: 2, memory_index: 0 }));
        }
    }
}

fn emit_mem_store(func: &mut wasm_encoder::Function, size: u32, align: u32) {
    let align_log2 = align.trailing_zeros();
    match size {
        8 => { func.instruction(&Instruction::I64Store(MemArg { offset: 0, align: align_log2, memory_index: 0 })); }
        _ => { func.instruction(&Instruction::I32Store(MemArg { offset: 0, align: align_log2.min(2), memory_index: 0 })); }
    }
}

fn emit_typed_mem_store(func: &mut wasm_encoder::Function, ty: &mir::types::MirType) {
    let align_log2 = ty.alignment().trailing_zeros();
    match ty {
        mir::types::MirType::Elementary(e) if e.is_float() && e.is_64bit() => {
            func.instruction(&Instruction::F64Store(MemArg { offset: 0, align: align_log2, memory_index: 0 }));
        }
        mir::types::MirType::Elementary(e) if e.is_float() => {
            func.instruction(&Instruction::F32Store(MemArg { offset: 0, align: align_log2, memory_index: 0 }));
        }
        mir::types::MirType::Elementary(e) if e.is_64bit() => {
            func.instruction(&Instruction::I64Store(MemArg { offset: 0, align: align_log2, memory_index: 0 }));
        }
        _ => {
            func.instruction(&Instruction::I32Store(MemArg { offset: 0, align: align_log2.min(2), memory_index: 0 }));
        }
    }
}

fn place_type(place: &mir::expr::MirPlace) -> mir::types::MirType {
    match place {
        mir::expr::MirPlace::Field { field_type, .. } => field_type.clone(),
        mir::expr::MirPlace::Index { element_type, .. } => element_type.clone(),
        mir::expr::MirPlace::Deref { pointee_type, .. } => pointee_type.clone(),
        mir::expr::MirPlace::ThisField { field_type, .. } => field_type.clone(),
        _ => mir::types::MirType::Elementary(mir::types::MirElementary::Int),
    }
}
