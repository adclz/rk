//! Emit WASM instructions from MIR statements.
//! Purely mechanical - reads structured control flow and emits WASM blocks.

use hir::hir_def::interned::identifier::Ident;
use mir::{
    expr::{MirConstant, MirExpr, MirPlace},
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

/// The byte size of an aggregate-typed value, or `None` for a scalar (or a
/// STRING, which has its own write path).
fn aggregate_value_size(value: &MirExpr) -> Option<u32> {
    let ty = match value {
        MirExpr::Load(_, ty) => ty,
        MirExpr::Call(call) => &call.return_type,
        _ => return None,
    };
    match ty {
        mir::types::MirType::Struct(_) | mir::types::MirType::Array(_) => Some(ty.size_bytes()),
        _ => None,
    }
}

/// How a function delivers its return value, resolved once per function
/// from the return slot's storage; `Return` and the epilogue push exactly
/// what the signature declares.
#[derive(Clone, Copy)]
pub(crate) enum ReturnValue {
    /// Scalar in a wasm local: push it.
    ScalarLocal(u32),
    /// STRING in a static slot: push `(ptr, len)` per the canonical ABI —
    /// buffer base at `addr + 4`, length loaded from `addr`.
    StringMem(u32),
    /// Aggregate in a static slot: push its address; the caller copies out
    /// of it.
    AggregateMem(u32),
}

/// Push a function's return value, matching its wasm signature.
pub(crate) fn emit_return_value(func: &mut wasm_encoder::Function, ret: ReturnValue) {
    match ret {
        ReturnValue::ScalarLocal(idx) => {
            func.instruction(&Instruction::LocalGet(idx));
        }
        ReturnValue::StringMem(addr) => {
            func.instruction(&Instruction::I32Const(addr as i32 + 4));
            func.instruction(&Instruction::I32Const(addr as i32));
            func.instruction(&Instruction::I32Load(wasm_encoder::MemArg {
                offset: 0,
                align: 2,
                memory_index: 0,
            }));
        }
        ReturnValue::AggregateMem(addr) => {
            func.instruction(&Instruction::I32Const(addr as i32));
        }
    }
}

struct Ctx<'a> {
    locals: &'a FxHashMap<Ident, LocalInfo>,
    fn_indices: &'a FxHashMap<Ident, u32>,
    return_value: Option<ReturnValue>,
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
    /// i32 scratch holding a runtime-computed `FbCall` receiver address (see
    /// [`stmts_need_dynamic_fb_base`]).
    fb_recv_tmp: Option<u32>,
    /// Per-`For` scratch locals for the end bound and step, in
    /// [`for_scratch_requests`] order; `None` means a constant emitted
    /// inline. Per statement, since loops nest.
    for_scratch: &'a std::cell::RefCell<std::collections::VecDeque<(Option<u32>, Option<u32>)>>,
    /// Open wasm labels at the emission point: `br N` is relative, so
    /// `EXIT`/`CONTINUE` need it to reach their loop from under enclosing
    /// `IF`/`CASE` labels.
    open_labels: &'a std::cell::Cell<u32>,
    /// The innermost loops' branch targets; depth from a site with
    /// `open_labels = O` to a target `L` is `O - L`.
    loop_stack: &'a std::cell::RefCell<Vec<LoopTargets>>,
}

/// The two branch targets a loop offers the statements in its body.
struct LoopTargets {
    /// `open_labels` inside the loop's outermost (exit) block.
    exit_open: u32,
    /// `open_labels` inside the label whose END is "next iteration": the
    /// loop header for `WHILE`, the block before the UNTIL check for
    /// `REPEAT`, the block before the increment for `FOR`.
    cont_open: u32,
}

/// Bracket a `block`/`loop`/`if` label: bump the open count when it starts.
fn open_label(ctx: &Ctx) {
    ctx.open_labels.set(ctx.open_labels.get() + 1);
}

/// ...and drop it at its `end`.
fn close_label(ctx: &Ctx) {
    ctx.open_labels.set(ctx.open_labels.get() - 1);
}

/// Emit MIR statements with return context; returns the `DebugTrap`
/// records for the `debug-lines` table.
#[allow(clippy::too_many_arguments)]
pub(crate) fn emit_stmts_with_return(
    func: &mut wasm_encoder::Function,
    stmts: &[MirStmt],
    locals: &FxHashMap<Ident, LocalInfo>,
    fn_indices: &FxHashMap<Ident, u32>,
    builtin_indices: &FxHashMap<String, u32>,
    return_value: Option<ReturnValue>,
    rk_exception_tag_idx: Option<u32>,
    fb_recv_tmp: Option<u32>,
    for_scratch: std::collections::VecDeque<(Option<u32>, Option<u32>)>,
) -> Vec<(u32, MirSourceLocation)> {
    let lines = std::cell::RefCell::new(Vec::new());
    let for_scratch = std::cell::RefCell::new(for_scratch);
    let open_labels = std::cell::Cell::new(0);
    let loop_stack = std::cell::RefCell::new(Vec::new());
    {
        let ctx = Ctx {
            locals,
            fn_indices,
            return_value,
            builtin_indices,
            rk_exception_tag_idx,
            lines: &lines,
            fb_recv_tmp,
            for_scratch: &for_scratch,
            open_labels: &open_labels,
            loop_stack: &loop_stack,
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
            if let Some(ret) = ctx.return_value {
                emit_return_value(func, ret);
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
            open_label(ctx);
            emit_stmts(func, then_body, ctx);

            for (cond, body) in else_ifs {
                func.instruction(&Instruction::Else);
                emit_expr(func, cond, ctx.locals, ctx.fn_indices);
                func.instruction(&Instruction::If(BlockType::Empty));
                open_label(ctx);
                emit_stmts(func, body, ctx);
            }

            if let Some(else_stmts) = else_body {
                func.instruction(&Instruction::Else);
                emit_stmts(func, else_stmts, ctx);
            }

            func.instruction(&Instruction::End);
            close_label(ctx);
            for _ in else_ifs {
                func.instruction(&Instruction::End);
                close_label(ctx);
            }
        }

        MirStmt::Case {
            selector,
            arms,
            else_body,
        } => {
            func.instruction(&Instruction::Block(BlockType::Empty));
            open_label(ctx);

            // The comparison lane follows the pattern constant: a 64-bit selector
            // arrives with I64 constants.
            let wide = |c: &mir::expr::MirConstant| matches!(c, mir::expr::MirConstant::I64(_));
            for arm in arms {
                // Evaluate all patterns and OR them together
                for (i, pattern) in arm.patterns.iter().enumerate() {
                    match pattern {
                        // A label carrying its own test (a STRING compare) already
                        // yields the arm's bool.
                        MirCasePattern::Test(test) => {
                            emit_expr(func, test, ctx.locals, ctx.fn_indices);
                        }
                        MirCasePattern::Value(val) => {
                            emit_expr(func, selector, ctx.locals, ctx.fn_indices);
                            emit_constant_expr(func, val);
                            func.instruction(&(if wide(val) {
                                Instruction::I64Eq
                            } else {
                                Instruction::I32Eq
                            }));
                        }
                        MirCasePattern::Range { lower, upper } => {
                            let w = wide(lower) || wide(upper);
                            emit_expr(func, selector, ctx.locals, ctx.fn_indices);
                            emit_constant_expr(func, lower);
                            func.instruction(&(if w {
                                Instruction::I64GeS
                            } else {
                                Instruction::I32GeS
                            }));
                            emit_expr(func, selector, ctx.locals, ctx.fn_indices);
                            emit_constant_expr(func, upper);
                            func.instruction(&(if w {
                                Instruction::I64LeS
                            } else {
                                Instruction::I32LeS
                            }));
                            func.instruction(&Instruction::I32And);
                        }
                    }
                    // OR with previous pattern result (after second+ pattern)
                    if i > 0 {
                        func.instruction(&Instruction::I32Or);
                    }
                }

                func.instruction(&Instruction::If(BlockType::Empty));
                open_label(ctx);
                emit_stmts(func, &arm.body, ctx);
                func.instruction(&Instruction::Br(1)); // break out of outer block
                func.instruction(&Instruction::End);
                close_label(ctx);
            }

            if let Some(else_stmts) = else_body {
                emit_stmts(func, else_stmts, ctx);
            }

            func.instruction(&Instruction::End); // outer block
            close_label(ctx);
        }

        MirStmt::For {
            control,
            control_type,
            start,
            end,
            step,
            body,
        } => {
            // The counter is read and written through its place: a wasm local or
            // linear memory.
            let ctrl_local = match control {
                MirPlace::Local(ident) => match ctx.locals.get(ident) {
                    Some(LocalInfo::Scalar { index, .. }) => Some(*index),
                    _ => None,
                },
                _ => None,
            };
            let ctrl_ty = mir::types::MirType::Elementary(*control_type);
            // Load the counter onto the stack.
            let ctrl_load =
                |func: &mut wasm_encoder::Function, ctx: &Ctx| match ctrl_local {
                    Some(idx) => {
                        func.instruction(&Instruction::LocalGet(idx));
                    }
                    None => {
                        emit_addr_of(func, control, ctx.locals, ctx.fn_indices);
                        emit_typed_mem_load(func, &ctrl_ty);
                    }
                };

            // A memory-resident counter needs its address below the value.
            match ctrl_local {
                Some(idx) => {
                    emit_expr(func, start, ctx.locals, ctx.fn_indices);
                    func.instruction(&Instruction::LocalSet(idx));
                }
                None => {
                    emit_addr_of(func, control, ctx.locals, ctx.fn_indices);
                    emit_expr(func, start, ctx.locals, ctx.fn_indices);
                    emit_typed_mem_store(func, &ctrl_ty);
                }
            }

            // IEC: the end and step are evaluated once, at loop entry; each
            // non-constant one is snapshotted into this statement's scratch.
            let (end_tmp, step_tmp) = ctx
                .for_scratch
                .borrow_mut()
                .pop_front()
                .unwrap_or_else(|| missing_for_scratch(ctx));
            if let Some(idx) = end_tmp {
                emit_expr(func, end, ctx.locals, ctx.fn_indices);
                func.instruction(&Instruction::LocalSet(idx));
            }
            if let Some(idx) = step_tmp {
                emit_expr(func, step, ctx.locals, ctx.fn_indices);
                func.instruction(&Instruction::LocalSet(idx));
            }

            func.instruction(&Instruction::Block(BlockType::Empty));
            open_label(ctx);
            let exit_open = ctx.open_labels.get();
            func.instruction(&Instruction::Loop(BlockType::Empty));
            open_label(ctx);

            // A descending loop (constant negative BY) exits below the end bound;
            // non-constant steps default to ascending; unsigned counters are
            // always ascending.
            let descending = matches!(&**step, MirExpr::Constant(MirConstant::I32(c)) if *c < 0)
                || matches!(&**step, MirExpr::Constant(MirConstant::I64(c)) if *c < 0);
            let is_64 = control_type.is_64bit();

            // Check bound
            ctrl_load(func, ctx);
            match end_tmp {
                Some(idx) => {
                    func.instruction(&Instruction::LocalGet(idx));
                }
                None => emit_expr(func, end, ctx.locals, ctx.fn_indices),
            }
            let cmp = match (is_64, control_type.is_signed(), descending) {
                (false, true, false) => Instruction::I32GtS,
                (false, true, true) => Instruction::I32LtS,
                (false, false, _) => Instruction::I32GtU,
                (true, true, false) => Instruction::I64GtS,
                (true, true, true) => Instruction::I64LtS,
                (true, false, _) => Instruction::I64GtU,
            };
            func.instruction(&cmp);
            func.instruction(&Instruction::BrIf(1));

            // A FOR's "next iteration" is the increment, not the header.
            func.instruction(&Instruction::Block(BlockType::Empty));
            open_label(ctx);
            ctx.loop_stack.borrow_mut().push(LoopTargets {
                exit_open,
                cont_open: ctx.open_labels.get(),
            });
            emit_stmts(func, body, ctx);
            ctx.loop_stack.borrow_mut().pop();
            func.instruction(&Instruction::End); // continue target: the increment
            close_label(ctx);

            // A bound at the type's maximum must terminate the loop, not wrap: exit
            // before incrementing when the headroom to the type's edge is smaller
            // than the step. Wrapping lane subtraction read unsigned is the exact
            // headroom.
            let bits = u64::from(control_type.rk_bits());
            let (ty_max, ty_min): (i64, i64) = match (control_type.is_signed(), bits) {
                (true, 64) => (i64::MAX, i64::MIN),
                (true, _) => ((1i64 << (bits - 1)) - 1, -(1i64 << (bits - 1))),
                (false, 64) => (-1, 0), // u64::MAX's lane pattern
                (false, _) => ((1i64 << bits) - 1, 0),
            };
            if descending {
                ctrl_load(func, ctx);
                if is_64 {
                    func.instruction(&Instruction::I64Const(ty_min));
                } else {
                    func.instruction(&Instruction::I32Const(ty_min as i32));
                }
            } else {
                if is_64 {
                    func.instruction(&Instruction::I64Const(ty_max));
                } else {
                    func.instruction(&Instruction::I32Const(ty_max as i32));
                }
                ctrl_load(func, ctx);
            }
            func.instruction(if is_64 {
                &Instruction::I64Sub
            } else {
                &Instruction::I32Sub
            });
            // The step's magnitude: wrapping negation read unsigned is exact
            // even for the lane minimum.
            if descending {
                if is_64 {
                    func.instruction(&Instruction::I64Const(0));
                } else {
                    func.instruction(&Instruction::I32Const(0));
                }
            }
            match step_tmp {
                Some(idx) => {
                    func.instruction(&Instruction::LocalGet(idx));
                }
                None => emit_expr(func, step, ctx.locals, ctx.fn_indices),
            }
            if descending {
                func.instruction(if is_64 {
                    &Instruction::I64Sub
                } else {
                    &Instruction::I32Sub
                });
            }
            func.instruction(if is_64 {
                &Instruction::I64LtU
            } else {
                &Instruction::I32LtU
            });
            func.instruction(&Instruction::BrIf(1));

            // Increment; sub-width counters wrap like any other arithmetic.
            match ctrl_local {
                Some(idx) => {
                    func.instruction(&Instruction::LocalGet(idx));
                }
                None => {
                    // Store needs (addr, value): address first, then the
                    // incremented value computed from a fresh load.
                    emit_addr_of(func, control, ctx.locals, ctx.fn_indices);
                    ctrl_load(func, ctx);
                }
            }
            match step_tmp {
                Some(idx) => {
                    func.instruction(&Instruction::LocalGet(idx));
                }
                None => emit_expr(func, step, ctx.locals, ctx.fn_indices),
            }
            if is_64 {
                func.instruction(&Instruction::I64Add);
            } else {
                func.instruction(&Instruction::I32Add);
                super::emit_expr::normalize_subwidth(func, *control_type);
            }
            match ctrl_local {
                Some(idx) => {
                    func.instruction(&Instruction::LocalSet(idx));
                }
                None => {
                    emit_typed_mem_store(func, &ctrl_ty);
                }
            }

            func.instruction(&Instruction::Br(0));
            func.instruction(&Instruction::End); // loop
            close_label(ctx);
            func.instruction(&Instruction::End); // block
            close_label(ctx);
        }

        MirStmt::While { condition, body } => {
            func.instruction(&Instruction::Block(BlockType::Empty));
            open_label(ctx);
            let exit_open = ctx.open_labels.get();
            func.instruction(&Instruction::Loop(BlockType::Empty));
            open_label(ctx);
            // A WHILE's "next iteration" is the loop header.
            ctx.loop_stack.borrow_mut().push(LoopTargets {
                exit_open,
                cont_open: ctx.open_labels.get(),
            });
            emit_expr(func, condition, ctx.locals, ctx.fn_indices);
            func.instruction(&Instruction::I32Eqz);
            func.instruction(&Instruction::BrIf(1));
            emit_stmts(func, body, ctx);
            ctx.loop_stack.borrow_mut().pop();
            func.instruction(&Instruction::Br(0));
            func.instruction(&Instruction::End);
            close_label(ctx);
            func.instruction(&Instruction::End);
            close_label(ctx);
        }

        MirStmt::Repeat { condition, body } => {
            func.instruction(&Instruction::Block(BlockType::Empty));
            open_label(ctx);
            let exit_open = ctx.open_labels.get();
            func.instruction(&Instruction::Loop(BlockType::Empty));
            open_label(ctx);
            // A REPEAT's "next iteration" is the UNTIL check, not the body's
            // start.
            func.instruction(&Instruction::Block(BlockType::Empty));
            open_label(ctx);
            ctx.loop_stack.borrow_mut().push(LoopTargets {
                exit_open,
                cont_open: ctx.open_labels.get(),
            });
            emit_stmts(func, body, ctx);
            ctx.loop_stack.borrow_mut().pop();
            func.instruction(&Instruction::End); // continue target: the check
            close_label(ctx);
            emit_expr(func, condition, ctx.locals, ctx.fn_indices);
            func.instruction(&Instruction::BrIf(1));
            func.instruction(&Instruction::Br(0));
            func.instruction(&Instruction::End);
            close_label(ctx);
            func.instruction(&Instruction::End);
            close_label(ctx);
        }

        MirStmt::Exit => {
            // Branch depth is relative: from under k IF/CASE labels, the loop's
            // exit block is k labels further out.
            let depth = {
                let stack = ctx.loop_stack.borrow();
                let t = stack.last().unwrap_or_else(|| missing_loop_targets("EXIT"));
                ctx.open_labels.get() - t.exit_open
            };
            func.instruction(&Instruction::Br(depth));
        }

        MirStmt::Continue => {
            let depth = {
                let stack = ctx.loop_stack.borrow();
                let t = stack
                    .last()
                    .unwrap_or_else(|| missing_loop_targets("CONTINUE"));
                ctx.open_labels.get() - t.cont_open
            };
            func.instruction(&Instruction::Br(depth));
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
            // A memory local or a global folds to a constant address, a `this`
            // member to `this + offset`; anything else is materialised into a
            // scratch local.
            let base = resolve_fb_base(func, instance, ctx);

            // 1. Write input values / inout addresses to the instance's fields
            for (field_offset, value, ty) in input_writes {
                emit_fb_field_write(func, base, *field_offset, value, ty, ctx);
            }

            // 2. Call __body__(&instance); an unresolved body symbol is fatal, as
            // in `emit_call`.
            let body_idx = ctx
                .fn_indices
                .get(body_func)
                .copied()
                .unwrap_or_else(|| missing_body_fn(body_func, ctx));
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
                FbBase::Dynamic(local) => {
                    func.instruction(&Instruction::LocalGet(local));
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
                        // Sub-width wrap: ABS(SINT#-128) is -128.
                        {
                            let mut instrs = Vec::new();
                            crate::mir_cast::append_subwidth_normalization(elem, &mut instrs);
                            for instr in &instrs {
                                func.instruction(instr);
                            }
                        }
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

            // IEC-width shifts/rotates (`rk.shl8`, `rk.rotl16`, …): raw
            // wasm shift ops work at the i32/i64 lane width, so sub-width
            // types need result masking, sub-width rotates need the
            // shl|shr composition (an 8-bit rotate is NOT a masked
            // i32.rotl — the wrapped bit lands at bit 31, not bit 7), and
            // wasm masks the shift count mod lane width, so shift-by-type-
            // width must be guarded to 0 explicitly. Needs the param
            // *locals* (IN, N referenced more than once) — same pattern as
            // the abs block above.
            if let Some(op) = instruction.strip_prefix("rk.") {
                let get_local = |name: &_| match ctx.locals.get(name) {
                    Some(LocalInfo::Scalar { index, .. }) => *index,
                    _ => panic!("rk.* shift/rotate params must be scalar locals"),
                };
                let in_idx = get_local(params.first().expect("rk.* op needs IN"));
                let n_idx = get_local(params.get(1).expect("rk.* op needs N"));
                emit_iec_shift_rotate(func, op, in_idx, n_idx);
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

/// Emit an IEC-width shift/rotate (`shl8`, `shr16`, `rotl8`, …) from the
/// IN/N param locals, IEC 61131-3 Table 30 semantics: a count `>= W`
/// yields 0, sub-width results are masked to W bits, sub-width rotates
/// are composed as `((IN << N') | (IN >> (W - N'))) & mask` with
/// `N' = N mod W`.
fn emit_iec_shift_rotate(func: &mut wasm_encoder::Function, op: &str, in_idx: u32, n_idx: u32) {
    // Shift, W in {8, 16, 32}: shift, mask, then `select(shifted, 0, N <u 32)`.
    let shift_i32 = |func: &mut wasm_encoder::Function, w: u32, shift_op: Instruction<'static>| {
        func.instruction(&Instruction::LocalGet(in_idx));
        func.instruction(&Instruction::LocalGet(n_idx));
        func.instruction(&shift_op);
        if w < 32 {
            func.instruction(&Instruction::I32Const(((1u64 << w) - 1) as i32));
            func.instruction(&Instruction::I32And);
        }
        func.instruction(&Instruction::I32Const(0));
        func.instruction(&Instruction::LocalGet(n_idx));
        func.instruction(&Instruction::I32Const(32));
        func.instruction(&Instruction::I32LtU);
        func.instruction(&Instruction::Select);
    };
    // 64-bit shift: extend the i32 count, guard N >= 64.
    let shift_i64 = |func: &mut wasm_encoder::Function, shift_op: Instruction<'static>| {
        func.instruction(&Instruction::LocalGet(in_idx));
        func.instruction(&Instruction::LocalGet(n_idx));
        func.instruction(&Instruction::I64ExtendI32U);
        func.instruction(&shift_op);
        func.instruction(&Instruction::I64Const(0));
        func.instruction(&Instruction::LocalGet(n_idx));
        func.instruction(&Instruction::I32Const(64));
        func.instruction(&Instruction::I32LtU);
        func.instruction(&Instruction::Select);
    };
    // Sub-width rotate: `((IN fwd N') | (IN back (W - N'))) & mask`,
    // `N' = N & (W-1)`.
    let rotate = |func: &mut wasm_encoder::Function,
                  w: u32,
                  fwd: Instruction<'static>,
                  back: Instruction<'static>| {
        func.instruction(&Instruction::LocalGet(in_idx));
        func.instruction(&Instruction::LocalGet(n_idx));
        func.instruction(&Instruction::I32Const((w - 1) as i32));
        func.instruction(&Instruction::I32And);
        func.instruction(&fwd);
        func.instruction(&Instruction::LocalGet(in_idx));
        func.instruction(&Instruction::I32Const(w as i32));
        func.instruction(&Instruction::LocalGet(n_idx));
        func.instruction(&Instruction::I32Const((w - 1) as i32));
        func.instruction(&Instruction::I32And);
        func.instruction(&Instruction::I32Sub);
        func.instruction(&back);
        func.instruction(&Instruction::I32Or);
        func.instruction(&Instruction::I32Const(((1u64 << w) - 1) as i32));
        func.instruction(&Instruction::I32And);
    };

    match op {
        "shl8" => shift_i32(func, 8, Instruction::I32Shl),
        "shl16" => shift_i32(func, 16, Instruction::I32Shl),
        "shl32" => shift_i32(func, 32, Instruction::I32Shl),
        "shl64" => shift_i64(func, Instruction::I64Shl),
        "shr8" => shift_i32(func, 8, Instruction::I32ShrU),
        "shr16" => shift_i32(func, 16, Instruction::I32ShrU),
        "shr32" => shift_i32(func, 32, Instruction::I32ShrU),
        "shr64" => shift_i64(func, Instruction::I64ShrU),
        "rotl8" => rotate(func, 8, Instruction::I32Shl, Instruction::I32ShrU),
        "rotl16" => rotate(func, 16, Instruction::I32Shl, Instruction::I32ShrU),
        "rotr8" => rotate(func, 8, Instruction::I32ShrU, Instruction::I32Shl),
        "rotr16" => rotate(func, 16, Instruction::I32ShrU, Instruction::I32Shl),
        other => panic!("unknown rk.* pseudo-op: rk.{}", other),
    }
}

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
        // 64-bit shifts and rotates: the IEC count is INT (i32), extended
        // to i64.
        "i64.shl" => {
            func.instruction(&Instruction::I64ExtendI32U);
            func.instruction(&Instruction::I64Shl);
        }
        "i64.shr_u" => {
            func.instruction(&Instruction::I64ExtendI32U);
            func.instruction(&Instruction::I64ShrU);
        }
        "i64.shr_s" => {
            func.instruction(&Instruction::I64ExtendI32U);
            func.instruction(&Instruction::I64ShrS);
        }
        "i64.rotl" => {
            func.instruction(&Instruction::I64ExtendI32U);
            func.instruction(&Instruction::I64Rotl);
        }
        "i64.rotr" => {
            func.instruction(&Instruction::I64ExtendI32U);
            func.instruction(&Instruction::I64Rotr);
        }
        // Bit-pattern reinterprets (Convert.st `REAL_TO_DWORD` family)
        "i32.reinterpret_f32" => {
            func.instruction(&Instruction::I32ReinterpretF32);
        }
        "i64.reinterpret_f64" => {
            func.instruction(&Instruction::I64ReinterpretF64);
        }
        "f32.reinterpret_i32" => {
            func.instruction(&Instruction::F32ReinterpretI32);
        }
        "f64.reinterpret_i64" => {
            func.instruction(&Instruction::F64ReinterpretI64);
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
        other => {
            // E0248 refuses unknown names at check; reaching one here means
            // the check and the emitter disagree, and a loud death beats a
            // valid module carrying code the program never asked for.
            panic!("unknown wasm instruction `{other}` survived the check");
        }
    }
}

/// How an `FbCall`'s instance is addressed: the first two fold at compile
/// time; `Dynamic` holds an already-materialised runtime address in a
/// scratch local, since the base is pushed several times per call.
#[derive(Clone, Copy)]
enum FbBase {
    Static(u32),
    This(u32),
    Dynamic(u32),
}

/// Fold `place` to a compile-time instance base, or `None`. The single
/// answer to "can this receiver fold?", shared with the scratch-local
/// pre-pass.
fn static_fb_base(place: &MirPlace, locals: &FxHashMap<Ident, LocalInfo>) -> Option<FbBase> {
    match place {
        MirPlace::Local(ident) => match locals.get(ident) {
            Some(LocalInfo::Memory { address, .. }) => Some(FbBase::Static(*address)),
            // A pointer parameter carries its address at runtime.
            _ => None,
        },
        // A VAR_GLOBAL instance sits at a fixed address, like a memory local.
        MirPlace::Global { address, .. } => Some(FbBase::Static(*address)),
        MirPlace::ThisField { field_offset, .. } => Some(FbBase::This(*field_offset)),
        MirPlace::Field {
            base, field_offset, ..
        } => match static_fb_base(base, locals)? {
            FbBase::Static(a) => Some(FbBase::Static(a + field_offset)),
            FbBase::This(o) => Some(FbBase::This(o + field_offset)),
            FbBase::Dynamic(_) => None,
        },
        MirPlace::Index {
            base,
            index,
            element_size,
            lower_bound,
            ..
        } => {
            // Only a literal subscript folds; anything else takes the dynamic
            // path.
            let MirExpr::Constant(MirConstant::I32(k)) = &**index else {
                return None;
            };
            let delta = u32::try_from((*k as i64 - *lower_bound) * *element_size as i64).ok()?;
            match static_fb_base(base, locals)? {
                FbBase::Static(a) => Some(FbBase::Static(a + delta)),
                FbBase::This(o) => Some(FbBase::This(o + delta)),
                FbBase::Dynamic(_) => None,
            }
        }
        // An instance reached through a pointer (a VAR_IN_OUT FB member).
        MirPlace::Deref { .. } => None,
    }
}

/// Whether any `FbCall` in `stmts` needs a runtime receiver address,
/// asked through `static_fb_base`.
pub(crate) fn stmts_need_dynamic_fb_base(
    stmts: &[MirStmt],
    locals: &FxHashMap<Ident, LocalInfo>,
) -> bool {
    stmts.iter().any(|s| stmt_needs_dynamic_fb_base(s, locals))
}

fn stmt_needs_dynamic_fb_base(stmt: &MirStmt, locals: &FxHashMap<Ident, LocalInfo>) -> bool {
    let nested = |b: &[MirStmt]| stmts_need_dynamic_fb_base(b, locals);
    match stmt {
        MirStmt::FbCall { instance, .. } => static_fb_base(instance, locals).is_none(),
        MirStmt::If {
            then_body,
            else_ifs,
            else_body,
            ..
        } => {
            nested(then_body)
                || else_ifs.iter().any(|(_, b)| nested(b))
                || else_body.as_deref().is_some_and(nested)
        }
        MirStmt::While { body, .. } | MirStmt::Repeat { body, .. } => nested(body),
        MirStmt::For { body, .. } => nested(body),
        MirStmt::Case {
            arms, else_body, ..
        } => arms.iter().any(|a| nested(&a.body)) || else_body.as_deref().is_some_and(nested),
        _ => false,
    }
}

/// One `For` statement's scratch needs: which of its end/step expressions
/// are snapshotted, at what lane width. In emitter order.
pub(crate) struct ForScratchReq {
    pub need_end: bool,
    pub need_step: bool,
    pub is_64: bool,
}

/// Every `For` statement's scratch needs, in emitter (pre-order) order; a
/// constant end/step needs none.
pub(crate) fn for_scratch_requests(stmts: &[MirStmt]) -> Vec<ForScratchReq> {
    let mut out = Vec::new();
    collect_for_scratch(stmts, &mut out);
    out
}

fn collect_for_scratch(stmts: &[MirStmt], out: &mut Vec<ForScratchReq>) {
    for stmt in stmts {
        match stmt {
            MirStmt::For {
                control_type,
                end,
                step,
                body,
                ..
            } => {
                out.push(ForScratchReq {
                    need_end: !matches!(end, MirExpr::Constant(_)),
                    need_step: !matches!(&**step, MirExpr::Constant(_)),
                    is_64: control_type.is_64bit(),
                });
                collect_for_scratch(body, out);
            }
            MirStmt::If {
                then_body,
                else_ifs,
                else_body,
                ..
            } => {
                collect_for_scratch(then_body, out);
                for (_, b) in else_ifs {
                    collect_for_scratch(b, out);
                }
                if let Some(b) = else_body {
                    collect_for_scratch(b, out);
                }
            }
            MirStmt::While { body, .. } | MirStmt::Repeat { body, .. } => {
                collect_for_scratch(body, out);
            }
            MirStmt::Case {
                arms, else_body, ..
            } => {
                for a in arms {
                    collect_for_scratch(&a.body, out);
                }
                if let Some(b) = else_body {
                    collect_for_scratch(b, out);
                }
            }
            _ => {}
        }
    }
}

/// An `EXIT`/`CONTINUE` reached the emitter with no enclosing loop on the
/// stack. HIR rejects both outside iteration statements (E1001/E1002), so
/// MIR cannot legitimately carry one here.
fn missing_loop_targets(what: &str) -> &'static LoopTargets {
    let caller = crate::emit_expr::CURRENT_EMIT_FN
        .with(|c| c.borrow().clone())
        .unwrap_or_else(|| "<unknown>".to_string());
    panic!(
        "internal compiler error: while emitting `{caller}`, a {what} has no \
         enclosing loop on the emitter's stack - HIR's E1001/E1002 checks \
         should have rejected it"
    )
}

/// A `For` with no scratch entry left: the plan walk and the emit walk
/// disagree.
fn missing_for_scratch(ctx: &Ctx) -> (Option<u32>, Option<u32>) {
    let caller = crate::emit_expr::CURRENT_EMIT_FN
        .with(|c| c.borrow().clone())
        .unwrap_or_else(|| "<unknown>".to_string());
    let _ = ctx;
    panic!(
        "internal compiler error: while emitting `{caller}`, a FOR statement \
         has no scratch-local plan entry - `for_scratch_requests` and the \
         emitter walked the body in different orders"
    )
}

/// Resolve the receiver's base, materialising a runtime address into the
/// function's scratch local when it cannot be folded.
fn resolve_fb_base(func: &mut wasm_encoder::Function, instance: &MirPlace, ctx: &Ctx) -> FbBase {
    if let Some(base) = static_fb_base(instance, ctx.locals) {
        return base;
    }
    let tmp = ctx
        .fb_recv_tmp
        .unwrap_or_else(|| missing_fb_scratch(instance, ctx));
    emit_addr_of(func, instance, ctx.locals, ctx.fn_indices);
    func.instruction(&Instruction::LocalSet(tmp));
    FbBase::Dynamic(tmp)
}

/// A runtime receiver address with no scratch allocated: the pre-pass and
/// the emitter disagree.
fn missing_fb_scratch(instance: &MirPlace, ctx: &Ctx) -> u32 {
    let caller = crate::emit_expr::CURRENT_EMIT_FN
        .with(|c| c.borrow().clone())
        .unwrap_or_else(|| "<unknown>".to_string());
    let _ = ctx;
    panic!(
        "internal compiler error: while emitting `{caller}`, a function-block \
         invocation needs a computed receiver address but no scratch local was \
         allocated - `stmts_need_dynamic_fb_base` and `resolve_fb_base` disagree. \
         Receiver: {instance:?}"
    )
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
        FbBase::Dynamic(local) => {
            func.instruction(&Instruction::LocalGet(local));
            if field_offset > 0 {
                func.instruction(&Instruction::I32Const(field_offset as i32));
                func.instruction(&Instruction::I32Add);
            }
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
        emit_addr_of(func, target, ctx.locals, ctx.fn_indices);
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
            emit_addr_of(func, target, ctx.locals, ctx.fn_indices); // dest header addr
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
            emit_addr_of(func, target, ctx.locals, ctx.fn_indices); // dst
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
    emit_addr_of(func, target, ctx.locals, ctx.fn_indices);
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
    // An aggregate assignment copies bytes, decided from the value's own
    // MIR type.
    if let Some(size) = aggregate_value_size(value) {
        emit_addr_of(func, target, ctx.locals, ctx.fn_indices); // dst
        match value {
            MirExpr::Load(src, _) => {
                emit_addr_of(func, src, ctx.locals, ctx.fn_indices); // src
            }
            // An aggregate-returning call pushes its static return slot's
            // address.
            MirExpr::Call(_) => {
                emit_expr(func, value, ctx.locals, ctx.fn_indices);
            }
            other => {
                let caller = crate::emit_expr::CURRENT_EMIT_FN
                    .with(|c| c.borrow().clone())
                    .unwrap_or_else(|| "<unknown>".to_string());
                panic!(
                    "internal compiler error: while emitting `{caller}`, an aggregate \
                     assignment's value is neither a place nor a call: {other:?}"
                );
            }
        }
        func.instruction(&Instruction::I32Const(size as i32));
        func.instruction(&Instruction::MemoryCopy {
            src_mem: 0,
            dst_mem: 0,
        });
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
            emit_addr_of(func, target, ctx.locals, ctx.fn_indices);
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

/// Public wrapper for `emit_call`'s extern-result stores.
pub(crate) fn emit_typed_mem_store_pub(
    func: &mut wasm_encoder::Function,
    ty: &mir::types::MirType,
) {
    emit_typed_mem_store(func, ty);
}

fn emit_typed_mem_store(func: &mut wasm_encoder::Function, ty: &mir::types::MirType) {
    // An enum stores at its declared storage lane, a subrange at its base.
    let resolved;
    let ty = match ty {
        mir::types::MirType::Enum(e) => {
            resolved = mir::types::MirType::Elementary(e.storage);
            &resolved
        }
        mir::types::MirType::Subrange(s) => {
            resolved = mir::types::MirType::Elementary(s.base);
            &resolved
        }
        other => other,
    };
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

/// The type a store through `place` must use.
///
/// Every variant that knows its own type answers with it. `Global` is one of
/// them and used to be missing: it fell through to the `Int` default, so a
/// `REAL` global or PROGRAM field emitted `i32.store` under an `f32` value and
/// the whole module failed wasm validation - reported against `__init`, with
/// nothing pointing back at the initializer. `rk compile` still exited 0.
///
/// `Local` is the only variant with no type of its own; a scalar local is
/// handled by the `LocalInfo` arms above and never reaches here, so the
/// remaining case is an address-taken aggregate, for which the width is
/// carried by the value rather than the place.
fn place_type(place: &mir::expr::MirPlace) -> mir::types::MirType {
    match place {
        mir::expr::MirPlace::Field { field_type, .. } => field_type.clone(),
        mir::expr::MirPlace::Index { element_type, .. } => element_type.clone(),
        mir::expr::MirPlace::Deref { pointee_type, .. } => pointee_type.clone(),
        mir::expr::MirPlace::ThisField { field_type, .. } => field_type.clone(),
        mir::expr::MirPlace::Global { ty, .. } => ty.clone(),
        // Structurally unreachable: `emit_assignment` handles Local first.
        mir::expr::MirPlace::Local(ident) => {
            panic!("place_type asked for a bare Local ('{ident:?}'): it carries no type")
        }
    }
}

/// Panic with a breadcrumb when an `FbCall` names a body function that
/// was never emitted.
fn missing_body_fn(body_func: &hir::hir_def::interned::identifier::Ident, ctx: &Ctx) -> u32 {
    crate::emit_expr::FN_NAMES_FOR_DIAGNOSTIC.with(|cell| {
        let names = cell.borrow();
        let missing = names
            .get(body_func)
            .cloned()
            .unwrap_or_else(|| format!("{body_func:?}"));
        let caller = crate::emit_expr::CURRENT_EMIT_FN
            .with(|c| c.borrow().clone())
            .unwrap_or_else(|| "<unknown>".to_string());
        let mut available: Vec<&str> = ctx
            .fn_indices
            .keys()
            .filter_map(|k| names.get(k).map(|s| s.as_str()))
            .filter(|n: &&str| n.contains("$__body__"))
            .collect();
        available.sort();
        panic!(
            "internal compiler error: while emitting `{caller}`, a function-block \
             invocation references unknown body function `{missing}` - it was not \
             lowered.\nBody functions that do exist: {available:?}"
        )
    })
}
