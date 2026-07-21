//! Emit WASM instructions from MIR statements.
//! Purely mechanical - reads structured control flow and emits WASM blocks.

use hir::hir_def::interned::identifier::Ident;
use mir::{
    expr::{MirConstant, MirExpr},
    stmt::{MirCasePattern, MirSourceLocation, MirStmt},
};
use rustc_hash::FxHashMap;
use wasm_encoder::{BlockType, Instruction, MemArg};

use super::{
    LocalInfo,
    emit_expr::{
        emit_addr_of, emit_expr, emit_str_value, emit_string_capacity, emit_typed_mem_load,
        is_buffer_string, mem_arg,
    },
};

/// Context for statement emission.
struct Ctx<'a> {
    locals: &'a FxHashMap<Ident, LocalInfo>,
    fn_indices: &'a FxHashMap<Ident, u32>,
    return_local: Option<u32>,
    /// Builtin instruction name (`f32.sin`) → wasm index of its grafted
    /// implementation.
    builtin_indices: &'a FxHashMap<String, u32>,
    /// Index of the module-level `$rk_exception` tag, populated when
    /// any function in the module contains `MirStmt::Raise`. `None`
    /// otherwise — in which case `MirStmt::Raise` must never reach
    /// codegen.
    rk_exception_tag_idx: Option<u32>,
    /// `(within-body offset, source location)` at each `DebugTrap`, for the
    /// `debug-lines` table.
    lines: &'a std::cell::RefCell<Vec<(u32, MirSourceLocation)>>,
}

/// Emit a list of MIR statements with return local context. Returns the
/// `(within-body offset, source location)` records gathered from the body's
/// `DebugTrap` markers, for the `debug-lines` table.
pub(crate) fn emit_stmts_with_return(
    func: &mut wasm_encoder::Function,
    stmts: &[MirStmt],
    locals: &FxHashMap<Ident, LocalInfo>,
    fn_indices: &FxHashMap<Ident, u32>,
    builtin_indices: &FxHashMap<String, u32>,
    return_local: Option<u32>,
    rk_exception_tag_idx: Option<u32>,
) -> Vec<(u32, MirSourceLocation)> {
    let lines = std::cell::RefCell::new(Vec::new());
    {
        let ctx = Ctx {
            locals,
            fn_indices,
            return_local,
            builtin_indices,
            rk_exception_tag_idx,
            lines: &lines,
        };
        emit_stmts(func, stmts, &ctx);
    }
    lines.into_inner()
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
            // Resolve how to address the instance: a Local instance sits at a
            // static linear-memory address; a nested FB (a member of `this`)
            // is addressed dynamically off the this pointer (wasm local 0).
            let base = match instance {
                mir::expr::MirPlace::Local(ident) => match ctx.locals.get(ident) {
                    Some(LocalInfo::Memory { address, .. }) => FbBase::Static(*address),
                    _ => return,
                },
                mir::expr::MirPlace::ThisField { field_offset, .. } => FbBase::This(*field_offset),
                _ => return,
            };

            // 1. Write input values / inout addresses to the instance's fields
            for (field_offset, value, ty) in input_writes {
                emit_fb_field_write(func, base, *field_offset, value, ty, ctx);
            }

            // 2. Call __body__(&instance)
            let body_idx = ctx.fn_indices.get(body_func).copied().unwrap_or(0);
            match base {
                FbBase::Static(addr) => {
                    func.instruction(&Instruction::I32Const(addr as i32));
                }
                FbBase::This(offset) => {
                    func.instruction(&Instruction::LocalGet(0));
                    if offset > 0 {
                        func.instruction(&Instruction::I32Const(offset as i32));
                        func.instruction(&Instruction::I32Add);
                    }
                }
            }
            func.instruction(&Instruction::Call(body_idx));

            // 3. Read output values from the instance's fields
            for (field_offset, target, ty) in output_reads {
                emit_fb_field_read(func, base, *field_offset, target, ty, ctx);
            }
        }

        MirStmt::WasmIntrinsic {
            instruction,
            params,
            result,
        } => {
            // A builtin (`f32.sin`, `str_byte_len`) is a `call` to the grafted
            // function; each param is pushed as the ABI demands (a STRING as
            // `(ptr, len)`).
            if let Some(&fn_idx) = ctx.builtin_indices.get(instruction.as_str()) {
                // A string producer (result declared STRING) takes
                // `(...args, out_addr, out_cap)` and writes the destination
                // directly; nothing is returned.
                let producer_out: Option<(u32, u32)> =
                    result.and_then(|name| match ctx.locals.get(&name)? {
                        LocalInfo::StringMemory { address, capacity } => {
                            Some((*address, *capacity))
                        }
                        _ => None,
                    });

                // Push regular params first.
                for param_name in params {
                    match ctx.locals.get(param_name) {
                        Some(LocalInfo::Scalar { index, .. }) => {
                            func.instruction(&Instruction::LocalGet(*index));
                        }
                        Some(LocalInfo::StringParam {
                            ptr_index,
                            len_index,
                        }) => {
                            func.instruction(&Instruction::LocalGet(*ptr_index));
                            func.instruction(&Instruction::LocalGet(*len_index));
                        }
                        Some(LocalInfo::StringInOutParam {
                            addr_index,
                            cap_index,
                        }) => {
                            // A STRING `VAR_IN_OUT` (addr, cap) pair goes straight
                            // to the mutator.
                            func.instruction(&Instruction::LocalGet(*addr_index));
                            func.instruction(&Instruction::LocalGet(*cap_index));
                        }
                        Some(LocalInfo::StringMemory { address, .. }) => {
                            // STRING var passed by value: push (ptr, len).
                            func.instruction(&Instruction::I32Const(*address as i32 + 4));
                            func.instruction(&Instruction::I32Const(*address as i32));
                            func.instruction(&Instruction::I32Load(mem_arg(0, 2)));
                        }
                        _ => {}
                    }
                }

                // The producer's (out_addr, out_cap): a static slot or the
                // function's own out-buffer params.
                if let Some((addr, cap)) = producer_out {
                    func.instruction(&Instruction::I32Const(addr as i32));
                    func.instruction(&Instruction::I32Const(cap as i32));
                }

                func.instruction(&Instruction::Call(fn_idx));

                // The scalar path stores the result; a producer already wrote it.
                if producer_out.is_none()
                    && let Some(result_name) = result
                    && let Some(LocalInfo::Scalar { index, .. }) = ctx.locals.get(result_name)
                {
                    func.instruction(&Instruction::LocalSet(*index));
                }
                return;
            }

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

        MirStmt::DebugTrap { location, .. } => {
            // Position marker: within-body offset → source position. No
            // instructions.
            ctx.lines
                .borrow_mut()
                .push((func.byte_len() as u32, location.clone()));
        }

        MirStmt::Raise { message } => {
            // Evaluate the STRING message expression — STRING values leave
            // `(ptr, len)` on the stack — then throw the module-level
            // `$rk_exception` tag, which has signature `(i32, i32) -> ()`.
            //
            // No in-language catch: the exception propagates to the host
            // embedder, which surfaces it as a fault. The `rk_exception_tag_idx`
            // is guaranteed `Some` here by the WasmGen pre-pass that runs
            // before any function emission.
            emit_expr(func, message, ctx.locals, ctx.fn_indices);
            let tag_idx = ctx
                .rk_exception_tag_idx
                .expect("MIR contains MirStmt::Raise but no $rk_exception tag was registered");
            func.instruction(&Instruction::Throw(tag_idx));
        }
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

/// How an `FbCall`'s instance is addressed: a static linear-memory base for a
/// Local instance, or `this + offset` for a nested FB member.
#[derive(Clone, Copy)]
enum FbBase {
    Static(u32),
    This(u32),
}

/// Push the linear-memory address of `base`'s field at `field_offset`.
fn push_fb_field_addr(func: &mut wasm_encoder::Function, base: FbBase, field_offset: u32) {
    match base {
        FbBase::Static(addr) => {
            func.instruction(&Instruction::I32Const((addr + field_offset) as i32));
        }
        FbBase::This(base_offset) => {
            func.instruction(&Instruction::LocalGet(0));
            func.instruction(&Instruction::I32Const((base_offset + field_offset) as i32));
            func.instruction(&Instruction::I32Add);
        }
    }
}

/// Write one `FbCall` input into an instance field, dispatching on the field
/// type (see the `MirStmt::FbCall` doc for the per-type shapes).
fn emit_fb_field_write(
    func: &mut wasm_encoder::Function,
    base: FbBase,
    field_offset: u32,
    value: &MirExpr,
    ty: &mir::types::MirType,
    ctx: &Ctx,
) {
    use mir::types::MirType;
    match ty {
        MirType::Elementary(_) => {
            push_fb_field_addr(func, base, field_offset);
            emit_expr(func, value, ctx.locals, ctx.fn_indices);
            emit_typed_mem_store(func, ty);
        }
        // Enums and subranges are scalars of their storage type.
        MirType::Enum(e) => {
            push_fb_field_addr(func, base, field_offset);
            emit_expr(func, value, ctx.locals, ctx.fn_indices);
            emit_typed_mem_store(func, &MirType::Elementary(e.storage));
        }
        MirType::Subrange(s) => {
            push_fb_field_addr(func, base, field_offset);
            emit_expr(func, value, ctx.locals, ctx.fn_indices);
            emit_typed_mem_store(func, &MirType::Elementary(s.base));
        }
        // A by-ref VAR_IN_OUT: the value is `AddrOf(arg)` — store the address.
        MirType::Pointer(_) => {
            push_fb_field_addr(func, base, field_offset);
            emit_expr(func, value, ctx.locals, ctx.fn_indices);
            emit_mem_store(func, 4, 4);
        }
        // STRING input: capacity-bounded copy into the field's inline buffer.
        // The value pushes the source (ptr, len) pair.
        MirType::String { capacity } => {
            let assign_idx = ctx
                .builtin_indices
                .get("rk.str_assign")
                .copied()
                .expect("rk.str_assign must be grafted for STRING FB inputs");
            push_fb_field_addr(func, base, field_offset); // dest header addr
            func.instruction(&Instruction::I32Const(*capacity as i32)); // dest cap
            emit_str_value(func, value, ctx.locals, ctx.fn_indices); // (src_ptr, src_len)
            func.instruction(&Instruction::Call(assign_idx));
        }
        // Aggregates: the value is `AddrOf(source)` — bulk-copy the bytes.
        MirType::Struct(_) | MirType::Array(_) => {
            push_fb_field_addr(func, base, field_offset); // dst
            emit_expr(func, value, ctx.locals, ctx.fn_indices); // src
            func.instruction(&Instruction::I32Const(ty.size_bytes() as i32)); // len
            func.instruction(&Instruction::MemoryCopy {
                src_mem: 0,
                dst_mem: 0,
            });
        }
        MirType::Void => {}
    }
}

/// Copy one `FbCall` output field back into the bound target place,
/// dispatching on the field type.
fn emit_fb_field_read(
    func: &mut wasm_encoder::Function,
    base: FbBase,
    field_offset: u32,
    target: &mir::expr::MirPlace,
    ty: &mir::types::MirType,
    ctx: &Ctx,
) {
    use mir::types::MirType;
    let scalar_copy = |func: &mut wasm_encoder::Function, elem_ty: &MirType| {
        emit_addr_of(func, target, ctx.locals);
        push_fb_field_addr(func, base, field_offset);
        emit_typed_mem_load(func, elem_ty);
        emit_typed_mem_store(func, elem_ty);
    };
    match ty {
        MirType::Elementary(_) => scalar_copy(func, ty),
        MirType::Enum(e) => scalar_copy(func, &MirType::Elementary(e.storage)),
        MirType::Subrange(s) => scalar_copy(func, &MirType::Elementary(s.base)),
        // A by-ref VAR_IN_OUT field needs no copy-back — the body already
        // wrote through the caller's address.
        MirType::Pointer(_) => {}
        // STRING output: capacity-bounded copy from the field's inline buffer
        // into the target's buffer.
        MirType::String { .. } => {
            let assign_idx = ctx
                .builtin_indices
                .get("rk.str_assign")
                .copied()
                .expect("rk.str_assign must be grafted for STRING FB outputs");
            emit_addr_of(func, target, ctx.locals); // dest header addr
            emit_string_capacity(func, target, ctx.locals); // dest cap
            // src ptr = field header + 4
            push_fb_field_addr(func, base, field_offset);
            func.instruction(&Instruction::I32Const(4));
            func.instruction(&Instruction::I32Add);
            // src len = *field header
            push_fb_field_addr(func, base, field_offset);
            func.instruction(&Instruction::I32Load(MemArg {
                offset: 0,
                align: 2,
                memory_index: 0,
            }));
            func.instruction(&Instruction::Call(assign_idx));
        }
        // Aggregates: bulk-copy the field's bytes into the target.
        MirType::Struct(_) | MirType::Array(_) => {
            emit_addr_of(func, target, ctx.locals); // dst
            push_fb_field_addr(func, base, field_offset); // src
            func.instruction(&Instruction::I32Const(ty.size_bytes() as i32)); // len
            func.instruction(&Instruction::MemoryCopy {
                src_mem: 0,
                dst_mem: 0,
            });
        }
        MirType::Void => {}
    }
}

/// Unified STRING assignment into a buffer-backed target (local buffer, instance
/// field, global, array element, or `VAR_IN_OUT`). The dest header address is
/// computed uniformly via `emit_addr_of`; `rk.str_assign(dest_addr, dest_cap,
/// src_ptr, src_len)` does a capacity-bounded `memcpy` into the inline buffer
/// and updates the length prefix.
fn emit_string_assign(
    func: &mut wasm_encoder::Function,
    target: &mir::expr::MirPlace,
    value: &MirExpr,
    ctx: &Ctx,
) {
    let assign_idx = ctx
        .builtin_indices
        .get("rk.str_assign")
        .copied()
        .expect("rk.str_assign must be grafted for STRING assignment");
    emit_addr_of(func, target, ctx.locals);
    emit_string_capacity(func, target, ctx.locals);
    emit_str_value(func, value, ctx.locals, ctx.fn_indices);
    func.instruction(&Instruction::Call(assign_idx));
}

fn emit_assignment(
    func: &mut wasm_encoder::Function,
    target: &mir::expr::MirPlace,
    value: &MirExpr,
    ctx: &Ctx,
) {
    // Unified string write: any buffer-backed STRING target copies through
    // rk.str_assign, regardless of where it lives. A borrowed VAR_INPUT view
    // target (not buffer-backed) rebinds its (ptr, len) locals — handled in the
    // Local match below.
    if is_buffer_string(target, ctx.locals) {
        emit_string_assign(func, target, value, ctx);
        return;
    }
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
                    LocalInfo::StringMemory { .. } | LocalInfo::StringInOutParam { .. } => {
                        // Buffer-backed strings are handled above by
                        // `is_buffer_string` / `emit_string_assign`.
                        unreachable!("buffer-backed string target reached scalar emit_assignment")
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
