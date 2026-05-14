//! Emit WASM instructions from MIR expressions.
//! Purely mechanical - reads pre-resolved types, places, and indices.

use std::cell::RefCell;

use hir::hir_def::interned::identifier::Ident;
use mir::{
    expr::{MirArgKind, MirBinOp, MirCall, MirConstant, MirExpr, MirPlace, MirUnaryOp},
    types::{MirElementary, MirType},
};
use rustc_hash::FxHashMap;
use wasm_encoder::{Instruction, MemArg};

use crate::{LocalInfo, mir_cast::emit_cast_instructions};

/// Per-function context for snapshotting nested STRING-returning call
/// results: producers write into one static return slot per callee, so a
/// nested call's result is copied into a unique scratch slot before the
/// next argument is evaluated.
pub(crate) struct StringSnapshotCtx {
    /// Pre-allocated scratch slot addresses for each nested STRING-returning
    /// call in this function (in encounter order).
    pub slots: Vec<u32>,
    /// Capacity each slot was sized to (uniform for now).
    pub slot_capacity: u32,
    /// Index into `slots` for the next nested STRING call.
    pub next_slot: usize,
    /// i32 temp holding the source ptr during a snapshot.
    pub ptr_tmp: u32,
    /// i32 temp holding the source len during a snapshot.
    pub len_tmp: u32,
    /// WASM index of the grafted `rk.str_assign` helper.
    pub str_assign_idx: u32,
}

thread_local! {
    /// Active snapshot context for the current function being emitted.
    /// Set by `emit_function` before walking the body, torn down after.
    pub(crate) static SNAPSHOT_CTX: RefCell<Option<StringSnapshotCtx>> =
        const { RefCell::new(None) };

    /// `Ident → text` for every function in the module, so the `emit_call`
    /// panic can name a missing callee without threading the db through.
    pub(crate) static FN_NAMES_FOR_DIAGNOSTIC: RefCell<FxHashMap<Ident, String>> =
        RefCell::new(FxHashMap::default());

    /// The function whose body is being emitted, for the `emit_call` panic.
    pub(crate) static CURRENT_EMIT_FN: RefCell<Option<String>> =
        const { RefCell::new(None) };
}

/// Emit the snapshot dance for a STRING-returning call result currently on
/// the stack as `(ptr, len)`. Replaces the top two stack values with
/// `(scratch+4, len)` where `scratch` is a freshly-allocated per-call-site
/// slot, and the source bytes have been memcpy'd into it via
/// `rk_str_assign`.
fn emit_string_snapshot(func: &mut wasm_encoder::Function) {
    SNAPSHOT_CTX.with(|cell| {
        let mut borrow = cell.borrow_mut();
        let Some(ctx) = borrow.as_mut() else {
            return;
        };
        let slot_addr = ctx.slots[ctx.next_slot];
        let slot_cap = ctx.slot_capacity;
        ctx.next_slot += 1;
        let ptr_tmp = ctx.ptr_tmp;
        let len_tmp = ctx.len_tmp;
        let str_assign = ctx.str_assign_idx;
        // Stack: [ptr, len]
        func.instruction(&Instruction::LocalSet(len_tmp));
        func.instruction(&Instruction::LocalSet(ptr_tmp));
        // rk_str_assign(slot_addr, slot_cap, ptr_tmp, len_tmp)
        func.instruction(&Instruction::I32Const(slot_addr as i32));
        func.instruction(&Instruction::I32Const(slot_cap as i32));
        func.instruction(&Instruction::LocalGet(ptr_tmp));
        func.instruction(&Instruction::LocalGet(len_tmp));
        func.instruction(&Instruction::Call(str_assign));
        // Push (scratch+4, len_tmp) - `rk_str_assign` clamps to slot_cap so
        // len_tmp may overstate the actually-written length, but slot_cap
        // matches the producer's max output for the targeted call sites.
        func.instruction(&Instruction::I32Const(slot_addr as i32 + 4));
        func.instruction(&Instruction::LocalGet(len_tmp));
    });
}

/// Emit instructions for a MIR expression (pushes result onto stack).
pub(crate) fn emit_expr(
    func: &mut wasm_encoder::Function,
    expr: &MirExpr,
    locals: &FxHashMap<Ident, LocalInfo>,
    fn_indices: &FxHashMap<Ident, u32>,
) {
    match expr {
        MirExpr::Constant(c) => emit_constant(func, c),

        MirExpr::Load(place, _ty) => emit_load(func, place, locals),

        MirExpr::BinOp { op, lhs, rhs, ty } => {
            emit_expr(func, lhs, locals, fn_indices);
            emit_expr(func, rhs, locals, fn_indices);
            emit_binop(func, *op, *ty);
        }

        MirExpr::UnaryOp {
            op,
            expr: inner,
            ty,
        } => {
            emit_expr(func, inner, locals, fn_indices);
            emit_unaryop(func, *op, *ty);
        }

        MirExpr::Cast {
            expr: inner,
            from,
            to,
        } => {
            emit_expr(func, inner, locals, fn_indices);
            for instr in emit_cast_instructions(*from, *to) {
                func.instruction(&instr);
            }
        }

        MirExpr::Call(call) => emit_call(func, call, locals, fn_indices),

        MirExpr::AddrOf(place) => emit_addr_of(func, place, locals),

        MirExpr::StringLiteral { offset, len, .. } => {
            func.instruction(&Instruction::I32Const(*offset as i32));
            func.instruction(&Instruction::I32Const(*len as i32));
        }
    }
}

fn emit_constant(func: &mut wasm_encoder::Function, c: &MirConstant) {
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

fn emit_load(
    func: &mut wasm_encoder::Function,
    place: &MirPlace,
    locals: &FxHashMap<Ident, LocalInfo>,
) {
    match place {
        MirPlace::Local(ident) => {
            if let Some(info) = locals.get(ident) {
                match info {
                    LocalInfo::Scalar { index, .. } => {
                        func.instruction(&Instruction::LocalGet(*index));
                    }
                    LocalInfo::Memory { address, elem, .. } => {
                        func.instruction(&Instruction::I32Const(*address as i32));
                        if let Some(e) = elem {
                            emit_typed_mem_load(func, &MirType::Elementary(*e));
                        } else {
                            func.instruction(&Instruction::I32Load(mem_arg(0, 2)));
                        }
                    }
                    LocalInfo::Pointer {
                        index,
                        pointee_elem,
                    } => {
                        func.instruction(&Instruction::LocalGet(*index));
                        // Load through pointer with correct type
                        if let Some(elem) = pointee_elem {
                            emit_typed_mem_load(func, &MirType::Elementary(*elem));
                        } else {
                            func.instruction(&Instruction::I32Load(mem_arg(0, 2)));
                        }
                    }
                    LocalInfo::StringParam {
                        ptr_index,
                        len_index,
                    } => {
                        // Push (ptr, len) pair on the stack
                        func.instruction(&Instruction::LocalGet(*ptr_index));
                        func.instruction(&Instruction::LocalGet(*len_index));
                    }
                    LocalInfo::StringMemory { address, .. } => {
                        // Layout: 4-byte len at `address`, then embedded
                        // buffer at `address + 4`. Push (ptr, len) where
                        // ptr is a const (no load) and len is loaded.
                        func.instruction(&Instruction::I32Const(*address as i32 + 4));
                        func.instruction(&Instruction::I32Const(*address as i32));
                        func.instruction(&Instruction::I32Load(mem_arg(0, 2)));
                    }
                    LocalInfo::StringInOutParam { addr_index, .. } => {
                        // Reading a STRING `VAR_IN_OUT` param from inside
                        // its function body: synthesise (ptr, len) where
                        // ptr = addr+4 and len = *addr.
                        func.instruction(&Instruction::LocalGet(*addr_index));
                        func.instruction(&Instruction::I32Const(4));
                        func.instruction(&Instruction::I32Add);
                        func.instruction(&Instruction::LocalGet(*addr_index));
                        func.instruction(&Instruction::I32Load(mem_arg(0, 2)));
                    }
                }
            } else {
                // Variable not in local map - push 0 as fallback
                func.instruction(&Instruction::I32Const(0));
            }
        }

        MirPlace::Field {
            base,
            field_offset,
            field_type,
            ..
        } => {
            emit_addr_of(func, base, locals);
            if *field_offset > 0 {
                func.instruction(&Instruction::I32Const(*field_offset as i32));
                func.instruction(&Instruction::I32Add);
            }
            emit_typed_mem_load(func, field_type);
        }

        MirPlace::Index {
            base,
            index,
            element_size,
            element_type,
            lower_bound,
        } => {
            emit_addr_of(func, base, locals);
            emit_expr(func, index, locals, &FxHashMap::default());
            if *lower_bound != 0 {
                func.instruction(&Instruction::I32Const(*lower_bound as i32));
                func.instruction(&Instruction::I32Sub);
            }
            func.instruction(&Instruction::I32Const(*element_size as i32));
            func.instruction(&Instruction::I32Mul);
            func.instruction(&Instruction::I32Add);
            emit_typed_mem_load(func, element_type);
        }

        MirPlace::Deref { base, pointee_type } => {
            emit_load(func, base, locals);
            emit_typed_mem_load(func, pointee_type);
        }

        MirPlace::ThisField {
            field_offset,
            field_type,
            ..
        } => {
            func.instruction(&Instruction::LocalGet(0));
            if *field_offset > 0 {
                func.instruction(&Instruction::I32Const(*field_offset as i32));
                func.instruction(&Instruction::I32Add);
            }
            emit_typed_mem_load(func, field_type);
        }
    }
}

/// Emit the address of a place (for stores, AddrOf, and as base for field/index).
pub(crate) fn emit_addr_of(
    func: &mut wasm_encoder::Function,
    place: &MirPlace,
    locals: &FxHashMap<Ident, LocalInfo>,
) {
    match place {
        MirPlace::Local(ident) => {
            if let Some(info) = locals.get(ident) {
                match info {
                    LocalInfo::Memory { address, .. } => {
                        func.instruction(&Instruction::I32Const(*address as i32));
                    }
                    LocalInfo::Pointer { index, .. } => {
                        func.instruction(&Instruction::LocalGet(*index));
                    }
                    LocalInfo::Scalar { .. } => {
                        // Address of a scalar - shouldn't happen if MIR is correct
                        // (address-taken scalars are in memory)
                    }
                    LocalInfo::StringParam { .. } => {
                        // String params are on the stack, not addressable
                    }
                    LocalInfo::StringMemory { address, .. } => {
                        func.instruction(&Instruction::I32Const(*address as i32));
                    }
                    LocalInfo::StringInOutParam { addr_index, .. } => {
                        func.instruction(&Instruction::LocalGet(*addr_index));
                    }
                }
            } else {
                // Variable not in local map - emit 0 as fallback address
                func.instruction(&Instruction::I32Const(0));
            }
        }
        MirPlace::Field {
            base, field_offset, ..
        } => {
            emit_addr_of(func, base, locals);
            if *field_offset > 0 {
                func.instruction(&Instruction::I32Const(*field_offset as i32));
                func.instruction(&Instruction::I32Add);
            }
        }
        MirPlace::Index {
            base,
            index,
            element_size,
            lower_bound,
            ..
        } => {
            emit_addr_of(func, base, locals);
            emit_expr(func, index, locals, &FxHashMap::default());
            if *lower_bound != 0 {
                func.instruction(&Instruction::I32Const(*lower_bound as i32));
                func.instruction(&Instruction::I32Sub);
            }
            func.instruction(&Instruction::I32Const(*element_size as i32));
            func.instruction(&Instruction::I32Mul);
            func.instruction(&Instruction::I32Add);
        }
        MirPlace::Deref { base, .. } => {
            // Address of a deref is the pointer value itself
            emit_load(func, base, locals);
        }
        MirPlace::ThisField { field_offset, .. } => {
            func.instruction(&Instruction::LocalGet(0));
            if *field_offset > 0 {
                func.instruction(&Instruction::I32Const(*field_offset as i32));
                func.instruction(&Instruction::I32Add);
            }
        }
    }
}

fn emit_call(
    func: &mut wasm_encoder::Function,
    call: &MirCall,
    locals: &FxHashMap<Ident, LocalInfo>,
    fn_indices: &FxHashMap<Ident, u32>,
) {
    // Emit arguments
    for arg in &call.args {
        match arg.kind {
            MirArgKind::ByValue => {
                emit_expr(func, &arg.value, locals, fn_indices);
                // A STRING-returning call as a `ByValue` arg points into the
                // callee's static return slot; snapshot it so the next arg
                // cannot clobber it.
                if let MirExpr::Call(inner) = &arg.value
                    && matches!(inner.return_type, MirType::String { .. })
                {
                    emit_string_snapshot(func);
                }
            }
            MirArgKind::ByRef => {
                // STRING `VAR_IN_OUT` flattens to (addr, cap) - symmetric
                // with the (ptr, len) we use for STRING `VAR_INPUT`. The
                // callee mutator clamps writes against cap.
                if let MirExpr::AddrOf(MirPlace::Local(name)) = &arg.value {
                    match locals.get(name) {
                        Some(LocalInfo::StringMemory { address, capacity }) => {
                            func.instruction(&Instruction::I32Const(*address as i32));
                            func.instruction(&Instruction::I32Const(*capacity as i32));
                            continue;
                        }
                        Some(LocalInfo::StringInOutParam {
                            addr_index,
                            cap_index,
                        }) => {
                            func.instruction(&Instruction::LocalGet(*addr_index));
                            func.instruction(&Instruction::LocalGet(*cap_index));
                            continue;
                        }
                        _ => {}
                    }
                }
                if let MirExpr::AddrOf(place) = &arg.value {
                    emit_addr_of(func, place, locals);
                } else {
                    emit_expr(func, &arg.value, locals, fn_indices);
                }
            }
        }
    }

    // An unresolved callee is a compiler error, never `call 0`.
    let idx = fn_indices.get(&call.callee).copied().unwrap_or_else(|| {
        FN_NAMES_FOR_DIAGNOSTIC.with(|cell| {
            let names = cell.borrow();
            let missing = names
                .get(&call.callee)
                .cloned()
                .unwrap_or_else(|| format!("{:?}", call.callee));
            let mut available: Vec<&str> = fn_indices
                .keys()
                .filter_map(|k| names.get(k).map(|s| s.as_str()))
                .collect();
            available.sort();
            let caller = CURRENT_EMIT_FN
                .with(|c| c.borrow().clone())
                .unwrap_or_else(|| "<unknown>".to_string());
            // Monomorphized variants of the missing callee, to show whether
            // the rewrite fired.
            let mono_variants: Vec<&str> = available
                .iter()
                .copied()
                .filter(|n| n.starts_with(&format!("{}.", missing)))
                .collect();
            panic!(
                "internal compiler error: while emitting `{}`, call site references \
                 unknown function `{}` — monomorphization or import registration missed \
                 this callee.\n\
                 Mono variants of `{}` that *do* exist ({}): {:?}\n\
                 All available function names ({}): {:?}",
                caller,
                missing,
                missing,
                mono_variants.len(),
                mono_variants,
                available.len(),
                available,
            )
        })
    });

    func.instruction(&Instruction::Call(idx));
}

fn emit_binop(func: &mut wasm_encoder::Function, op: MirBinOp, ty: MirElementary) {
    let is_float = ty.is_float();
    let is_64 = ty.is_64bit();
    let is_signed = ty.is_signed();

    match op {
        MirBinOp::Add => match (is_float, is_64) {
            (true, true) => {
                func.instruction(&Instruction::F64Add);
            }
            (true, false) => {
                func.instruction(&Instruction::F32Add);
            }
            (false, true) => {
                func.instruction(&Instruction::I64Add);
            }
            (false, false) => {
                func.instruction(&Instruction::I32Add);
            }
        },
        MirBinOp::Sub => match (is_float, is_64) {
            (true, true) => {
                func.instruction(&Instruction::F64Sub);
            }
            (true, false) => {
                func.instruction(&Instruction::F32Sub);
            }
            (false, true) => {
                func.instruction(&Instruction::I64Sub);
            }
            (false, false) => {
                func.instruction(&Instruction::I32Sub);
            }
        },
        MirBinOp::Mul => match (is_float, is_64) {
            (true, true) => {
                func.instruction(&Instruction::F64Mul);
            }
            (true, false) => {
                func.instruction(&Instruction::F32Mul);
            }
            (false, true) => {
                func.instruction(&Instruction::I64Mul);
            }
            (false, false) => {
                func.instruction(&Instruction::I32Mul);
            }
        },
        MirBinOp::Div => match (is_float, is_64, is_signed) {
            (true, true, _) => {
                func.instruction(&Instruction::F64Div);
            }
            (true, false, _) => {
                func.instruction(&Instruction::F32Div);
            }
            (false, true, true) => {
                func.instruction(&Instruction::I64DivS);
            }
            (false, true, false) => {
                func.instruction(&Instruction::I64DivU);
            }
            (false, false, true) => {
                func.instruction(&Instruction::I32DivS);
            }
            (false, false, false) => {
                func.instruction(&Instruction::I32DivU);
            }
        },
        MirBinOp::Mod => match (is_64, is_signed) {
            (true, true) => {
                func.instruction(&Instruction::I64RemS);
            }
            (true, false) => {
                func.instruction(&Instruction::I64RemU);
            }
            (false, true) => {
                func.instruction(&Instruction::I32RemS);
            }
            (false, false) => {
                func.instruction(&Instruction::I32RemU);
            }
        },
        MirBinOp::And => {
            if is_64 {
                func.instruction(&Instruction::I64And);
            } else {
                func.instruction(&Instruction::I32And);
            }
        }
        MirBinOp::Or => {
            if is_64 {
                func.instruction(&Instruction::I64Or);
            } else {
                func.instruction(&Instruction::I32Or);
            }
        }
        MirBinOp::Xor => {
            if is_64 {
                func.instruction(&Instruction::I64Xor);
            } else {
                func.instruction(&Instruction::I32Xor);
            }
        }
        MirBinOp::Eq => match (is_float, is_64) {
            (true, true) => {
                func.instruction(&Instruction::F64Eq);
            }
            (true, false) => {
                func.instruction(&Instruction::F32Eq);
            }
            (false, true) => {
                func.instruction(&Instruction::I64Eq);
            }
            (false, false) => {
                func.instruction(&Instruction::I32Eq);
            }
        },
        MirBinOp::Ne => match (is_float, is_64) {
            (true, true) => {
                func.instruction(&Instruction::F64Ne);
            }
            (true, false) => {
                func.instruction(&Instruction::F32Ne);
            }
            (false, true) => {
                func.instruction(&Instruction::I64Ne);
            }
            (false, false) => {
                func.instruction(&Instruction::I32Ne);
            }
        },
        MirBinOp::Lt => match (is_float, is_64, is_signed) {
            (true, true, _) => {
                func.instruction(&Instruction::F64Lt);
            }
            (true, false, _) => {
                func.instruction(&Instruction::F32Lt);
            }
            (false, true, true) => {
                func.instruction(&Instruction::I64LtS);
            }
            (false, true, false) => {
                func.instruction(&Instruction::I64LtU);
            }
            (false, false, true) => {
                func.instruction(&Instruction::I32LtS);
            }
            (false, false, false) => {
                func.instruction(&Instruction::I32LtU);
            }
        },
        MirBinOp::Le => match (is_float, is_64, is_signed) {
            (true, true, _) => {
                func.instruction(&Instruction::F64Le);
            }
            (true, false, _) => {
                func.instruction(&Instruction::F32Le);
            }
            (false, true, true) => {
                func.instruction(&Instruction::I64LeS);
            }
            (false, true, false) => {
                func.instruction(&Instruction::I64LeU);
            }
            (false, false, true) => {
                func.instruction(&Instruction::I32LeS);
            }
            (false, false, false) => {
                func.instruction(&Instruction::I32LeU);
            }
        },
        MirBinOp::Gt => match (is_float, is_64, is_signed) {
            (true, true, _) => {
                func.instruction(&Instruction::F64Gt);
            }
            (true, false, _) => {
                func.instruction(&Instruction::F32Gt);
            }
            (false, true, true) => {
                func.instruction(&Instruction::I64GtS);
            }
            (false, true, false) => {
                func.instruction(&Instruction::I64GtU);
            }
            (false, false, true) => {
                func.instruction(&Instruction::I32GtS);
            }
            (false, false, false) => {
                func.instruction(&Instruction::I32GtU);
            }
        },
        MirBinOp::Ge => match (is_float, is_64, is_signed) {
            (true, true, _) => {
                func.instruction(&Instruction::F64Ge);
            }
            (true, false, _) => {
                func.instruction(&Instruction::F32Ge);
            }
            (false, true, true) => {
                func.instruction(&Instruction::I64GeS);
            }
            (false, true, false) => {
                func.instruction(&Instruction::I64GeU);
            }
            (false, false, true) => {
                func.instruction(&Instruction::I32GeS);
            }
            (false, false, false) => {
                func.instruction(&Instruction::I32GeU);
            }
        },
        MirBinOp::Power => {
            // TODO: Power operator not yet supported
        }
    }
}

fn emit_unaryop(func: &mut wasm_encoder::Function, op: MirUnaryOp, ty: MirElementary) {
    match op {
        MirUnaryOp::Neg => {
            if ty.is_float() {
                if ty.is_64bit() {
                    func.instruction(&Instruction::F64Neg);
                } else {
                    func.instruction(&Instruction::F32Neg);
                }
            } else if ty.is_64bit() {
                func.instruction(&Instruction::I64Const(0));
                func.instruction(&Instruction::I64Sub);
                // Swap: we need 0 - value, but value is already on stack
                // Actually need: push 0, then swap... WASM doesn't have swap.
                // The correct approach: emit 0 first, then the value, then sub.
                // But the value was already emitted. We'd need to restructure.
                // For now, this is a known limitation - proper fix needs operand reordering.
            } else {
                // 0 - value for i32
                // Same issue - value already on stack.
                // Workaround: use (i32.const 0) (local.get tmp) (i32.sub)
                // For now emit xor with -1 and add 1 (two's complement negate)
                func.instruction(&Instruction::I32Const(-1));
                func.instruction(&Instruction::I32Xor);
                func.instruction(&Instruction::I32Const(1));
                func.instruction(&Instruction::I32Add);
            }
        }
        MirUnaryOp::Not => {
            if ty == MirElementary::Bool {
                func.instruction(&Instruction::I32Eqz);
            } else if ty.is_64bit() {
                func.instruction(&Instruction::I64Const(-1));
                func.instruction(&Instruction::I64Xor);
            } else {
                func.instruction(&Instruction::I32Const(-1));
                func.instruction(&Instruction::I32Xor);
            }
        }
    }
}

/// Emit a memory load instruction based on type.
pub(crate) fn emit_typed_mem_load(func: &mut wasm_encoder::Function, ty: &MirType) {
    let align_log2 = ty.alignment().trailing_zeros();
    match ty {
        MirType::Elementary(e) if e.is_float() && e.is_64bit() => {
            func.instruction(&Instruction::F64Load(mem_arg(0, align_log2)));
        }
        MirType::Elementary(e) if e.is_float() => {
            func.instruction(&Instruction::F32Load(mem_arg(0, align_log2)));
        }
        MirType::Elementary(e) if e.is_64bit() => {
            func.instruction(&Instruction::I64Load(mem_arg(0, align_log2)));
        }
        MirType::Elementary(e) if e.size_bytes() == 1 => {
            func.instruction(&Instruction::I32Load8U(mem_arg(0, 0)));
        }
        MirType::Elementary(e) if e.size_bytes() == 2 => {
            func.instruction(&Instruction::I32Load16U(mem_arg(0, align_log2.min(1))));
        }
        _ => {
            func.instruction(&Instruction::I32Load(mem_arg(0, align_log2.min(2))));
        }
    }
}

pub(crate) fn mem_arg(offset: u64, align: u32) -> MemArg {
    MemArg {
        offset,
        align,
        memory_index: 0,
    }
}
