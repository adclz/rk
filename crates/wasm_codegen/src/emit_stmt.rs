//! Emit WASM instructions from MIR statements.
//! Purely mechanical - reads structured control flow and emits WASM blocks.

use hir::hir_def::interned::identifier::Ident;
use mir::{
    expr::{MirConstant, MirExpr, MirPlace},
    stmt::{MirCasePattern, MirStmt},
};
use rustc_hash::FxHashMap;
use wasm_encoder::{BlockType, Instruction, MemArg};

use super::{
    LocalInfo,
    emit_expr::{emit_addr_of, emit_expr, emit_typed_mem_load, mem_arg},
};

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
    let ctx = Ctx {
        locals,
        fn_indices,
        return_local,
    };
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
            emit_expr(
                func,
                &MirExpr::Call(call.clone()),
                ctx.locals,
                ctx.fn_indices,
            );
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

        MirStmt::If {
            condition,
            then_body,
            else_ifs,
            else_body,
        } => {
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

        MirStmt::Case {
            selector,
            arms,
            else_body,
        } => {
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

        MirStmt::For {
            control_var,
            control_type,
            start,
            end,
            step,
            body,
        } => {
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

        MirStmt::FbCall {
            instance,
            body_func,
            body_func_index: _,
            input_writes,
            output_reads,
        } => {
            // Get the instance's memory address
            let instance_addr = match instance {
                mir::expr::MirPlace::Local(ident) => match ctx.locals.get(ident) {
                    Some(LocalInfo::Memory { address, .. }) => *address,
                    _ => return,
                },
                mir::expr::MirPlace::ThisField { field_offset, .. } => {
                    // Nested FB: instance at this_ptr + field_offset.
                    // Use dynamic addressing since the base comes from local 0 (this ptr).
                    let base_offset = *field_offset;

                    // 1. Write inputs to nested instance fields
                    for (fo, value, elem) in input_writes {
                        // Address: local.get 0 + base_offset + field_offset
                        func.instruction(&Instruction::LocalGet(0));
                        func.instruction(&Instruction::I32Const((base_offset + fo) as i32));
                        func.instruction(&Instruction::I32Add);
                        emit_expr(func, value, ctx.locals, ctx.fn_indices);
                        emit_typed_mem_store(func, &mir::types::MirType::Elementary(*elem));
                    }

                    // 2. Call __body__(&nested_instance)
                    let body_idx = ctx.fn_indices.get(body_func).copied().unwrap_or(0);
                    func.instruction(&Instruction::LocalGet(0));
                    if base_offset > 0 {
                        func.instruction(&Instruction::I32Const(base_offset as i32));
                        func.instruction(&Instruction::I32Add);
                    }
                    func.instruction(&Instruction::Call(body_idx));

                    // 3. Read outputs from nested instance
                    for (fo, target, elem) in output_reads {
                        emit_addr_of(func, target, ctx.locals);
                        func.instruction(&Instruction::LocalGet(0));
                        func.instruction(&Instruction::I32Const((base_offset + fo) as i32));
                        func.instruction(&Instruction::I32Add);
                        emit_typed_mem_load(func, &mir::types::MirType::Elementary(*elem));
                        emit_typed_mem_store(func, &mir::types::MirType::Elementary(*elem));
                    }
                    return;
                }
                _ => return,
            };

            // 1. Write input values to the FB instance's fields in memory
            for (field_offset, value, elem) in input_writes {
                // Push address (instance base + field offset)
                func.instruction(&Instruction::I32Const(
                    (instance_addr + field_offset) as i32,
                ));
                // Emit value
                emit_expr(func, value, ctx.locals, ctx.fn_indices);
                // Store typed
                let elem_ty = mir::types::MirType::Elementary(*elem);
                emit_typed_mem_store(func, &elem_ty);
            }

            // 2. Call __body__(&instance)
            let body_idx = ctx.fn_indices.get(body_func).copied().unwrap_or(0);
            func.instruction(&Instruction::I32Const(instance_addr as i32));
            func.instruction(&Instruction::Call(body_idx));

            // 3. Read output values from the FB instance's fields
            for (field_offset, target, elem) in output_reads {
                // First: emit target address
                emit_addr_of(func, target, ctx.locals);
                // Then: load from instance field
                func.instruction(&Instruction::I32Const(
                    (instance_addr + field_offset) as i32,
                ));
                let elem_ty = mir::types::MirType::Elementary(*elem);
                emit_typed_mem_load(func, &elem_ty);
                // Store to target
                emit_typed_mem_store(func, &elem_ty);
            }
        }

        MirStmt::WasmIntrinsic {
            instruction,
            params,
            result,
        } => {
            // Integer ABS has no wasm op: branchless `select` for signed, a no-op
            // for unsigned.
            if instruction == "i32.abs" || instruction == "i64.abs" {
                let in_name = params.first().expect("abs needs an input param");
                let (in_idx, elem) = match ctx.locals.get(in_name) {
                    Some(LocalInfo::Scalar { index, elem, .. }) => (*index, *elem),
                    _ => panic!("abs input must be a scalar local"),
                };
                if elem.is_signed() {
                    if elem.is_64bit() {
                        // -x
                        func.instruction(&Instruction::I64Const(0));
                        func.instruction(&Instruction::LocalGet(in_idx));
                        func.instruction(&Instruction::I64Sub);
                        // x
                        func.instruction(&Instruction::LocalGet(in_idx));
                        // x < 0 ?
                        func.instruction(&Instruction::LocalGet(in_idx));
                        func.instruction(&Instruction::I64Const(0));
                        func.instruction(&Instruction::I64LtS);
                        func.instruction(&Instruction::Select);
                    } else {
                        func.instruction(&Instruction::I32Const(0));
                        func.instruction(&Instruction::LocalGet(in_idx));
                        func.instruction(&Instruction::I32Sub);
                        func.instruction(&Instruction::LocalGet(in_idx));
                        func.instruction(&Instruction::LocalGet(in_idx));
                        func.instruction(&Instruction::I32Const(0));
                        func.instruction(&Instruction::I32LtS);
                        func.instruction(&Instruction::Select);
                    }
                } else {
                    // Unsigned: identity.
                    func.instruction(&Instruction::LocalGet(in_idx));
                }
                if let Some(result_name) = result
                    && let Some(LocalInfo::Scalar { index, .. }) = ctx.locals.get(result_name)
                {
                    func.instruction(&Instruction::LocalSet(*index));
                }
                return;
            }

            // Push params on stack
            for param_name in params {
                if let Some(info) = ctx.locals.get(param_name)
                    && let LocalInfo::Scalar { index, .. } = info
                {
                    func.instruction(&Instruction::LocalGet(*index));
                }
            }

            // Emit the WASM instruction
            emit_wasm_instruction(func, instruction);

            // Store result
            if let Some(result_name) = result
                && let Some(info) = ctx.locals.get(result_name)
                && let LocalInfo::Scalar { index, .. } = info
            {
                func.instruction(&Instruction::LocalSet(*index));
            }
        }

        MirStmt::DebugTrap { .. } => {}
    }
}

/// Map a WASM instruction name string to the corresponding wasm_encoder instruction.
fn emit_wasm_instruction(func: &mut wasm_encoder::Function, name: &str) {
    match name {
        // Integer arithmetic/bitwise (32-bit)
        "i32.shl" => {
            func.instruction(&Instruction::I32Shl);
        }
        "i32.shr_u" => {
            func.instruction(&Instruction::I32ShrU);
        }
        "i32.shr_s" => {
            func.instruction(&Instruction::I32ShrS);
        }
        "i32.rotl" => {
            func.instruction(&Instruction::I32Rotl);
        }
        "i32.rotr" => {
            func.instruction(&Instruction::I32Rotr);
        }
        "i32.and" => {
            func.instruction(&Instruction::I32And);
        }
        "i32.or" => {
            func.instruction(&Instruction::I32Or);
        }
        "i32.xor" => {
            func.instruction(&Instruction::I32Xor);
        }
        // Integer arithmetic/bitwise (64-bit)
        "i64.shl" => {
            func.instruction(&Instruction::I64Shl);
        }
        "i64.shr_u" => {
            func.instruction(&Instruction::I64ShrU);
        }
        "i64.shr_s" => {
            func.instruction(&Instruction::I64ShrS);
        }
        "i64.rotl" => {
            func.instruction(&Instruction::I64Rotl);
        }
        "i64.rotr" => {
            func.instruction(&Instruction::I64Rotr);
        }
        // Float conversions
        "f32.convert_i32_s" => {
            func.instruction(&Instruction::F32ConvertI32S);
        }
        "f32.convert_i32_u" => {
            func.instruction(&Instruction::F32ConvertI32U);
        }
        "f32.convert_i64_s" => {
            func.instruction(&Instruction::F32ConvertI64S);
        }
        "f64.convert_i32_s" => {
            func.instruction(&Instruction::F64ConvertI32S);
        }
        "f64.convert_i64_s" => {
            func.instruction(&Instruction::F64ConvertI64S);
        }
        // Int truncations from float
        "i32.trunc_f32_s" => {
            func.instruction(&Instruction::I32TruncF32S);
        }
        "i32.trunc_f64_s" => {
            func.instruction(&Instruction::I32TruncF64S);
        }
        "i64.trunc_f32_s" => {
            func.instruction(&Instruction::I64TruncF32S);
        }
        "i64.trunc_f64_s" => {
            func.instruction(&Instruction::I64TruncF64S);
        }
        // Float promotions/demotions
        "f32.demote_f64" => {
            func.instruction(&Instruction::F32DemoteF64);
        }
        "f64.promote_f32" => {
            func.instruction(&Instruction::F64PromoteF32);
        }
        // Integer wrapping/extending
        "i32.wrap_i64" => {
            func.instruction(&Instruction::I32WrapI64);
        }
        "i64.extend_i32_s" => {
            func.instruction(&Instruction::I64ExtendI32S);
        }
        "i64.extend_i32_u" => {
            func.instruction(&Instruction::I64ExtendI32U);
        }
        // Float math intrinsics (single-instruction)
        "f32.sqrt" => {
            func.instruction(&Instruction::F32Sqrt);
        }
        "f64.sqrt" => {
            func.instruction(&Instruction::F64Sqrt);
        }
        "f32.abs" => {
            func.instruction(&Instruction::F32Abs);
        }
        "f64.abs" => {
            func.instruction(&Instruction::F64Abs);
        }
        _ => {
            // Unknown instruction - emit unreachable as a trap
            func.instruction(&Instruction::Unreachable);
        }
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
                    LocalInfo::Memory { address, elem, .. } => {
                        func.instruction(&Instruction::I32Const(*address as i32));
                        emit_expr(func, value, ctx.locals, ctx.fn_indices);
                        if let Some(e) = elem {
                            emit_typed_mem_store(func, &mir::types::MirType::Elementary(*e));
                        } else {
                            emit_mem_store(func, 4, 4);
                        }
                    }
                    LocalInfo::Pointer {
                        index,
                        pointee_elem,
                    } => {
                        func.instruction(&Instruction::LocalGet(*index));
                        emit_expr(func, value, ctx.locals, ctx.fn_indices);
                        if let Some(elem) = pointee_elem {
                            emit_typed_mem_store(func, &mir::types::MirType::Elementary(*elem));
                        } else {
                            emit_mem_store(func, 4, 4);
                        }
                    }
                    LocalInfo::StringParam {
                        ptr_index,
                        len_index,
                    } => {
                        // Value pushes (ptr, len) pair on stack
                        emit_expr(func, value, ctx.locals, ctx.fn_indices);
                        func.instruction(&Instruction::LocalSet(*len_index));
                        func.instruction(&Instruction::LocalSet(*ptr_index));
                    }
                    LocalInfo::StringMemory { address } => {
                        // For string literals: we know it pushes (ptr, len)
                        // For string params/locals: also (ptr, len)
                        // Store ptr at addr, len at addr+4 as two separate stores
                        match value {
                            MirExpr::StringLiteral { offset, len, .. } => {
                                // Store ptr
                                func.instruction(&Instruction::I32Const(*address as i32));
                                func.instruction(&Instruction::I32Const(*offset as i32));
                                func.instruction(&Instruction::I32Store(mem_arg(0, 2)));
                                // Store len
                                func.instruction(&Instruction::I32Const(*address as i32 + 4));
                                func.instruction(&Instruction::I32Const(*len as i32));
                                func.instruction(&Instruction::I32Store(mem_arg(0, 2)));
                            }
                            MirExpr::Load(place, _) => {
                                // Load the source string (ptr, len) and store to target
                                if let Some(src_info) = match place {
                                    MirPlace::Local(id) => ctx.locals.get(id),
                                    _ => None,
                                } {
                                    match src_info {
                                        LocalInfo::StringParam {
                                            ptr_index,
                                            len_index,
                                        } => {
                                            func.instruction(&Instruction::I32Const(
                                                *address as i32,
                                            ));
                                            func.instruction(&Instruction::LocalGet(*ptr_index));
                                            func.instruction(&Instruction::I32Store(mem_arg(0, 2)));
                                            func.instruction(&Instruction::I32Const(
                                                *address as i32 + 4,
                                            ));
                                            func.instruction(&Instruction::LocalGet(*len_index));
                                            func.instruction(&Instruction::I32Store(mem_arg(0, 2)));
                                        }
                                        LocalInfo::StringMemory { address: src_addr } => {
                                            // Copy ptr
                                            func.instruction(&Instruction::I32Const(
                                                *address as i32,
                                            ));
                                            func.instruction(&Instruction::I32Const(
                                                *src_addr as i32,
                                            ));
                                            func.instruction(&Instruction::I32Load(mem_arg(0, 2)));
                                            func.instruction(&Instruction::I32Store(mem_arg(0, 2)));
                                            // Copy len
                                            func.instruction(&Instruction::I32Const(
                                                *address as i32 + 4,
                                            ));
                                            func.instruction(&Instruction::I32Const(
                                                *src_addr as i32 + 4,
                                            ));
                                            func.instruction(&Instruction::I32Load(mem_arg(0, 2)));
                                            func.instruction(&Instruction::I32Store(mem_arg(0, 2)));
                                        }
                                        _ => {
                                            // Fallback: zero out
                                            func.instruction(&Instruction::I32Const(
                                                *address as i32,
                                            ));
                                            func.instruction(&Instruction::I32Const(0));
                                            func.instruction(&Instruction::I32Store(mem_arg(0, 2)));
                                            func.instruction(&Instruction::I32Const(
                                                *address as i32 + 4,
                                            ));
                                            func.instruction(&Instruction::I32Const(0));
                                            func.instruction(&Instruction::I32Store(mem_arg(0, 2)));
                                        }
                                    }
                                }
                            }
                            _ => {
                                // Default: zero-initialize (empty string)
                                func.instruction(&Instruction::I32Const(*address as i32));
                                func.instruction(&Instruction::I32Const(0));
                                func.instruction(&Instruction::I32Store(mem_arg(0, 2)));
                                func.instruction(&Instruction::I32Const(*address as i32 + 4));
                                func.instruction(&Instruction::I32Const(0));
                                func.instruction(&Instruction::I32Store(mem_arg(0, 2)));
                            }
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
        MirConstant::Bool(v) => {
            func.instruction(&Instruction::I32Const(if *v { 1 } else { 0 }));
        }
        MirConstant::I32(v) => {
            func.instruction(&Instruction::I32Const(*v));
        }
        MirConstant::I64(v) => {
            func.instruction(&Instruction::I64Const(*v));
        }
        MirConstant::F32(v) => {
            func.instruction(&Instruction::F32Const((*v).into()));
        }
        MirConstant::F64(v) => {
            func.instruction(&Instruction::F64Const((*v).into()));
        }
        MirConstant::Null => {
            func.instruction(&Instruction::I32Const(0));
        }
    }
}

fn emit_constant_store(func: &mut wasm_encoder::Function, value: &MirConstant) {
    match value {
        MirConstant::Bool(v) => {
            func.instruction(&Instruction::I32Const(if *v { 1 } else { 0 }));
            func.instruction(&Instruction::I32Store(MemArg {
                offset: 0,
                align: 2,
                memory_index: 0,
            }));
        }
        MirConstant::I32(v) => {
            func.instruction(&Instruction::I32Const(*v));
            func.instruction(&Instruction::I32Store(MemArg {
                offset: 0,
                align: 2,
                memory_index: 0,
            }));
        }
        MirConstant::I64(v) => {
            func.instruction(&Instruction::I64Const(*v));
            func.instruction(&Instruction::I64Store(MemArg {
                offset: 0,
                align: 3,
                memory_index: 0,
            }));
        }
        MirConstant::F32(v) => {
            func.instruction(&Instruction::F32Const((*v).into()));
            func.instruction(&Instruction::F32Store(MemArg {
                offset: 0,
                align: 2,
                memory_index: 0,
            }));
        }
        MirConstant::F64(v) => {
            func.instruction(&Instruction::F64Const((*v).into()));
            func.instruction(&Instruction::F64Store(MemArg {
                offset: 0,
                align: 3,
                memory_index: 0,
            }));
        }
        MirConstant::Null => {
            func.instruction(&Instruction::I32Const(0));
            func.instruction(&Instruction::I32Store(MemArg {
                offset: 0,
                align: 2,
                memory_index: 0,
            }));
        }
    }
}

fn emit_mem_store(func: &mut wasm_encoder::Function, size: u32, align: u32) {
    let align_log2 = align.trailing_zeros();
    match size {
        8 => {
            func.instruction(&Instruction::I64Store(MemArg {
                offset: 0,
                align: align_log2,
                memory_index: 0,
            }));
        }
        _ => {
            func.instruction(&Instruction::I32Store(MemArg {
                offset: 0,
                align: align_log2.min(2),
                memory_index: 0,
            }));
        }
    }
}

fn emit_typed_mem_store(func: &mut wasm_encoder::Function, ty: &mir::types::MirType) {
    let align_log2 = ty.alignment().trailing_zeros();
    match ty {
        mir::types::MirType::Elementary(e) if e.is_float() && e.is_64bit() => {
            func.instruction(&Instruction::F64Store(MemArg {
                offset: 0,
                align: align_log2,
                memory_index: 0,
            }));
        }
        mir::types::MirType::Elementary(e) if e.is_float() => {
            func.instruction(&Instruction::F32Store(MemArg {
                offset: 0,
                align: align_log2,
                memory_index: 0,
            }));
        }
        mir::types::MirType::Elementary(e) if e.is_64bit() => {
            func.instruction(&Instruction::I64Store(MemArg {
                offset: 0,
                align: align_log2,
                memory_index: 0,
            }));
        }
        mir::types::MirType::Elementary(e) if e.size_bytes() == 1 => {
            func.instruction(&Instruction::I32Store8(MemArg {
                offset: 0,
                align: 0,
                memory_index: 0,
            }));
        }
        mir::types::MirType::Elementary(e) if e.size_bytes() == 2 => {
            func.instruction(&Instruction::I32Store16(MemArg {
                offset: 0,
                align: align_log2.min(1),
                memory_index: 0,
            }));
        }
        _ => {
            func.instruction(&Instruction::I32Store(MemArg {
                offset: 0,
                align: align_log2.min(2),
                memory_index: 0,
            }));
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
