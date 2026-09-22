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

    /// Index of `rk.null_check` for the module, `None` when nothing
    /// dereferences.
    pub(crate) static NULL_CHECK_IDX: RefCell<Option<u32>> =
        const { RefCell::new(None) };
}

/// Fault the pointer on the stack if it is null, leaving it in place.
fn emit_null_check(func: &mut wasm_encoder::Function) {
    if let Some(idx) = NULL_CHECK_IDX.with(|c| *c.borrow()) {
        func.instruction(&Instruction::Call(idx));
    }
}

/// Snapshot the STRING result `(ptr, len)` on the stack into a fresh
/// per-call-site slot via `rk_str_assign`, leaving `(scratch+4, len)`.
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
        // `rk_str_assign` clamps to the slot capacity, which matches the
        // producers' maximum output.
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

        MirExpr::Load(place, _ty) => emit_load(func, place, locals, fn_indices),

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

        MirExpr::AddrOf(place) => emit_addr_of(func, place, locals, fn_indices),

        // Aggregate VAR_INPUT arg: copy into the scratch and yield its address.
        MirExpr::CopyIntoScratch { scratch, src, size } => {
            let dst = mir::expr::MirPlace::Local(*scratch);
            emit_addr_of(func, &dst, locals, fn_indices); // dst
            emit_expr(func, src, locals, fn_indices); // src address
            func.instruction(&Instruction::I32Const(*size as i32)); // len
            func.instruction(&Instruction::MemoryCopy {
                src_mem: 0,
                dst_mem: 0,
            });
            emit_addr_of(func, &dst, locals, fn_indices); // the arg value
        }

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

/// Whether `place` holds a STRING, at any place kind a string can live.
pub(crate) fn place_is_string(place: &MirPlace, locals: &FxHashMap<Ident, LocalInfo>) -> bool {
    match place {
        MirPlace::Local(id) => matches!(
            locals.get(id),
            Some(
                LocalInfo::StringParam { .. }
                    | LocalInfo::StringMemory { .. }
                    | LocalInfo::StringInOutParam { .. }
            )
        ),
        MirPlace::Field { field_type, .. } | MirPlace::ThisField { field_type, .. } => {
            matches!(field_type, MirType::String { .. })
        }
        MirPlace::Global { ty, .. } => matches!(ty, MirType::String { .. }),
        MirPlace::Index { element_type, .. } => matches!(element_type, MirType::String { .. }),
        MirPlace::Deref { pointee_type, .. } => matches!(pointee_type, MirType::String { .. }),
    }
}

/// Push a string operand's `(ptr, len)` for any place: a borrowed
/// `VAR_INPUT` view reads its two locals; every owned string is addressed
/// via `emit_addr_of`, `ptr = header+4`, `len = *header`.
pub(crate) fn emit_str_place_value(
    func: &mut wasm_encoder::Function,
    place: &MirPlace,
    locals: &FxHashMap<Ident, LocalInfo>,
    fn_indices: &FxHashMap<Ident, u32>,
) {
    if let MirPlace::Local(id) = place
        && let Some(LocalInfo::StringParam {
            ptr_index,
            len_index,
        }) = locals.get(id)
    {
        func.instruction(&Instruction::LocalGet(*ptr_index));
        func.instruction(&Instruction::LocalGet(*len_index));
        return;
    }
    // Owned inline buffer: ptr = header + 4, len = *header.
    emit_addr_of(func, place, locals, fn_indices);
    func.instruction(&Instruction::I32Const(4));
    func.instruction(&Instruction::I32Add);
    emit_addr_of(func, place, locals, fn_indices);
    func.instruction(&Instruction::I32Load(mem_arg(0, 2)));
}

/// A string that owns an inline `[len]+buffer`: any string place except a
/// borrowed `VAR_INPUT` view. These route through `rk.str_assign`.
pub(crate) fn is_buffer_string(place: &MirPlace, locals: &FxHashMap<Ident, LocalInfo>) -> bool {
    if let MirPlace::Local(id) = place
        && matches!(locals.get(id), Some(LocalInfo::StringParam { .. }))
    {
        return false;
    }
    place_is_string(place, locals)
}

/// Push a string source as `(ptr, len)`: a literal, a place load, or a
/// STRING-returning call.
pub(crate) fn emit_str_value(
    func: &mut wasm_encoder::Function,
    value: &MirExpr,
    locals: &FxHashMap<Ident, LocalInfo>,
    fn_indices: &FxHashMap<Ident, u32>,
) {
    match value {
        MirExpr::StringLiteral { offset, len, .. } => {
            func.instruction(&Instruction::I32Const(*offset as i32));
            func.instruction(&Instruction::I32Const(*len as i32));
        }
        MirExpr::Load(place, _) => emit_str_place_value(func, place, locals, fn_indices),
        _ => emit_expr(func, value, locals, fn_indices),
    }
}

/// Push the capacity of a string place: a constant for owned storage, the
/// `cap` local for a `VAR_IN_OUT`.
pub(crate) fn emit_string_capacity(
    func: &mut wasm_encoder::Function,
    place: &MirPlace,
    locals: &FxHashMap<Ident, LocalInfo>,
) {
    let cap = match place {
        MirPlace::Local(id) => match locals.get(id) {
            Some(LocalInfo::StringMemory { capacity, .. }) => *capacity,
            Some(LocalInfo::StringInOutParam { cap_index, .. }) => {
                func.instruction(&Instruction::LocalGet(*cap_index));
                return;
            }
            _ => unreachable!("non-string local has no capacity"),
        },
        MirPlace::ThisField { field_type, .. } | MirPlace::Field { field_type, .. } => {
            string_capacity_of(field_type)
        }
        MirPlace::Global { ty, .. } => string_capacity_of(ty),
        MirPlace::Index { element_type, .. } => string_capacity_of(element_type),
        MirPlace::Deref { pointee_type, .. } => string_capacity_of(pointee_type),
    };
    func.instruction(&Instruction::I32Const(cap as i32));
}

fn string_capacity_of(ty: &MirType) -> u32 {
    match ty {
        MirType::String { capacity } => *capacity,
        _ => unreachable!("expected a STRING type for capacity"),
    }
}

fn emit_load(
    func: &mut wasm_encoder::Function,
    place: &MirPlace,
    locals: &FxHashMap<Ident, LocalInfo>,
    fn_indices: &FxHashMap<Ident, u32>,
) {
    // Strings are `(ptr, len)` operands, not scalar loads.
    if place_is_string(place, locals) {
        emit_str_place_value(func, place, locals, fn_indices);
        return;
    }
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
                    LocalInfo::StringParam { .. }
                    | LocalInfo::StringMemory { .. }
                    | LocalInfo::StringInOutParam { .. } => {
                        // Handled above by `place_is_string` / `emit_str_place_value`.
                        unreachable!("string local reached scalar emit_load")
                    }
                }
            } else {
                // A name with no local is a lowering bug, not a zero.
                panic!("emit_load: no local named {ident:?} in this function")
            }
        }

        MirPlace::Field {
            base,
            field_offset,
            field_type,
            ..
        } => {
            emit_addr_of(func, base, locals, fn_indices);
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
            emit_addr_of(func, base, locals, fn_indices);
            emit_expr(func, index, locals, fn_indices);
            if *lower_bound != 0 {
                func.instruction(&Instruction::I32Const(*lower_bound as i32));
                func.instruction(&Instruction::I32Sub);
            }
            func.instruction(&Instruction::I32Const(*element_size as i32));
            func.instruction(&Instruction::I32Mul);
            func.instruction(&Instruction::I32Add);
            emit_typed_mem_load(func, element_type);
        }

        MirPlace::Deref {
            base,
            pointee_type,
            checked,
        } => {
            emit_load(func, base, locals, fn_indices);
            if *checked {
                emit_null_check(func);
            }
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

        MirPlace::Global { address, ty, .. } => {
            // A VAR_GLOBAL at a fixed address: a type-aware load.
            func.instruction(&Instruction::I32Const(*address as i32));
            emit_typed_mem_load(func, ty);
        }
    }
}

/// Emit the address of a place (for stores, AddrOf, and as base for field/index).
pub(crate) fn emit_addr_of(
    func: &mut wasm_encoder::Function,
    place: &MirPlace,
    locals: &FxHashMap<Ident, LocalInfo>,
    fn_indices: &FxHashMap<Ident, u32>,
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
                        // A scalar in a wasm local has no address: the address-taken
                        // scan missed a `REF()`. Fail here.
                        panic!(
                            "emit_addr_of: {ident:?} is a wasm local, so it has no address; \
                             the address-taken scan missed a REF() naming it"
                        );
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
            emit_addr_of(func, base, locals, fn_indices);
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
            emit_addr_of(func, base, locals, fn_indices);
            emit_expr(func, index, locals, fn_indices);
            if *lower_bound != 0 {
                func.instruction(&Instruction::I32Const(*lower_bound as i32));
                func.instruction(&Instruction::I32Sub);
            }
            func.instruction(&Instruction::I32Const(*element_size as i32));
            func.instruction(&Instruction::I32Mul);
            func.instruction(&Instruction::I32Add);
        }
        MirPlace::Deref { base, checked, .. } => {
            // Address of a deref is the pointer value itself
            emit_load(func, base, locals, fn_indices);
            if *checked {
                emit_null_check(func);
            }
        }
        MirPlace::ThisField { field_offset, .. } => {
            func.instruction(&Instruction::LocalGet(0));
            if *field_offset > 0 {
                func.instruction(&Instruction::I32Const(*field_offset as i32));
                func.instruction(&Instruction::I32Add);
            }
        }
        MirPlace::Global { address, .. } => {
            // The address of a VAR_GLOBAL is just its fixed linear-memory address.
            func.instruction(&Instruction::I32Const(*address as i32));
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
                // STRING `VAR_IN_OUT` flattens to (header_addr, cap), for any
                // buffer-backed place; the callee clamps writes against cap.
                if let MirExpr::AddrOf(place) = &arg.value
                    && is_buffer_string(place, locals)
                {
                    emit_addr_of(func, place, locals, fn_indices);
                    emit_string_capacity(func, place, locals);
                    continue;
                }
                if let MirExpr::AddrOf(place) = &arg.value {
                    emit_addr_of(func, place, locals, fn_indices);
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

    // A `=>` destination wider than the output received it in a memory
    // scratch; convert into place now. The return value stays underneath.
    for bind in &call.output_bindings {
        let from_ty = mir::types::MirType::Elementary(bind.from);
        let to_ty = mir::types::MirType::Elementary(bind.to);
        let cast = crate::mir_cast::emit_cast_instructions(bind.from, bind.to);
        let scratch = mir::expr::MirPlace::Local(bind.scratch);
        match &bind.target {
            mir::expr::MirPlace::Local(name)
                if matches!(locals.get(name), Some(LocalInfo::Scalar { .. })) =>
            {
                emit_addr_of(func, &scratch, locals, fn_indices);
                crate::emit_stmt::emit_typed_mem_load_pub(func, &from_ty);
                for instr in &cast {
                    func.instruction(instr);
                }
                let Some(LocalInfo::Scalar { index, .. }) = locals.get(name) else {
                    unreachable!()
                };
                func.instruction(&Instruction::LocalSet(*index));
            }
            target => {
                emit_addr_of(func, target, locals, fn_indices);
                emit_addr_of(func, &scratch, locals, fn_indices);
                crate::emit_stmt::emit_typed_mem_load_pub(func, &from_ty);
                for instr in &cast {
                    func.instruction(instr);
                }
                crate::emit_stmt::emit_typed_mem_store_pub(func, &to_ty);
            }
        }
    }
    // Extern results pop in reverse wire order: the return value first (into
    // its scratch), then each output into its scratch, then the stores; the
    // return value ends on top.
    if !call.extern_results.is_empty() {
        let local_idx = |name: &Ident| -> u32 {
            match locals.get(name) {
                Some(LocalInfo::Scalar { index, .. }) => *index,
                other => panic!(
                    "internal compiler error: extern result scratch is not a scalar \
                     local: {other:?}"
                ),
            }
        };
        if let Some(ret) = &call.extern_ret_scratch {
            func.instruction(&Instruction::LocalSet(local_idx(ret)));
        }
        for bind in call.extern_results.iter().rev() {
            func.instruction(&Instruction::LocalSet(local_idx(&bind.scratch)));
        }
        for bind in &call.extern_results {
            let Some(dest) = &bind.dest else { continue };
            // A wider destination converts on the way out of the scratch.
            let (cast, store_ty) = match (&bind.ty, bind.target_lane) {
                (mir::types::MirType::Elementary(from), Some(to)) => (
                    crate::mir_cast::emit_cast_instructions(*from, to),
                    mir::types::MirType::Elementary(to),
                ),
                _ => (Vec::new(), bind.ty.clone()),
            };
            match dest {
                mir::expr::MirPlace::Local(name)
                    if matches!(locals.get(name), Some(LocalInfo::Scalar { .. })) =>
                {
                    func.instruction(&Instruction::LocalGet(local_idx(&bind.scratch)));
                    for instr in &cast {
                        func.instruction(instr);
                    }
                    func.instruction(&Instruction::LocalSet(local_idx(name)));
                }
                _ => {
                    emit_addr_of(func, dest, locals, fn_indices);
                    func.instruction(&Instruction::LocalGet(local_idx(&bind.scratch)));
                    for instr in &cast {
                        func.instruction(instr);
                    }
                    crate::emit_stmt::emit_typed_mem_store_pub(func, &store_ty);
                }
            }
        }
        if let Some(ret) = &call.extern_ret_scratch {
            func.instruction(&Instruction::LocalGet(local_idx(ret)));
        }
    }
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
        MirBinOp::Shl => {
            if is_64 {
                func.instruction(&Instruction::I64Shl);
            } else {
                func.instruction(&Instruction::I32Shl);
            }
        }
        MirBinOp::Shr => {
            if is_64 {
                func.instruction(&Instruction::I64ShrU);
            } else {
                func.instruction(&Instruction::I32ShrU);
            }
        }
        MirBinOp::Power => {
            // TODO: Power operator not yet supported
        }
    }

    // Sub-width arithmetic wraps at the IEC type width (`USINT 255 + 1` = 0);
    // comparisons and the logical ops are domain-closed.
    if matches!(
        op,
        MirBinOp::Add | MirBinOp::Sub | MirBinOp::Mul | MirBinOp::Div | MirBinOp::Mod
    ) && !is_float
    {
        normalize_subwidth(func, ty);
    }
}

/// Re-normalize an i32-lane arithmetic result into a sub-width type's
/// domain; no-op for BOOL and 32/64-bit types.
pub(crate) fn normalize_subwidth(func: &mut wasm_encoder::Function, ty: MirElementary) {
    let mut instrs = Vec::new();
    crate::mir_cast::append_subwidth_normalization(ty, &mut instrs);
    for instr in &instrs {
        func.instruction(instr);
    }
}

fn emit_unaryop(func: &mut wasm_encoder::Function, op: MirUnaryOp, ty: MirElementary) {
    match op {
        // Two's-complement negation `-x = (x ^ -1) + 1` works on the operand
        // already on the stack.
        MirUnaryOp::Neg => {
            if ty.is_float() {
                if ty.is_64bit() {
                    func.instruction(&Instruction::F64Neg);
                } else {
                    func.instruction(&Instruction::F32Neg);
                }
            } else if ty.is_64bit() {
                func.instruction(&Instruction::I64Const(-1));
                func.instruction(&Instruction::I64Xor);
                func.instruction(&Instruction::I64Const(1));
                func.instruction(&Instruction::I64Add);
            } else {
                func.instruction(&Instruction::I32Const(-1));
                func.instruction(&Instruction::I32Xor);
                func.instruction(&Instruction::I32Const(1));
                func.instruction(&Instruction::I32Add);
                // Sub-width wrap: -(SINT#-128) is -128, not the escaped 128.
                normalize_subwidth(func, ty);
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
                // A BYTE/WORD complement stays within the logical width.
                match ty {
                    MirElementary::Byte => {
                        func.instruction(&Instruction::I32Const(0xFF));
                        func.instruction(&Instruction::I32And);
                    }
                    MirElementary::Word => {
                        func.instruction(&Instruction::I32Const(0xFFFF));
                        func.instruction(&Instruction::I32And);
                    }
                    _ => {}
                }
            }
        }
    }
}

/// Emit a memory load instruction based on type.
pub(crate) fn emit_typed_mem_load(func: &mut wasm_encoder::Function, ty: &MirType) {
    // An enum loads at its declared storage lane, a subrange at its base.
    let resolved;
    let ty = match ty {
        MirType::Enum(e) => {
            resolved = MirType::Elementary(e.storage);
            &resolved
        }
        MirType::Subrange(s) => {
            resolved = MirType::Elementary(s.base);
            &resolved
        }
        other => other,
    };
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
        // An 8- or 16-bit value has a four-byte slot, and only its own bytes
        // are read: whatever the rest holds, the value is the same. A host
        // or a debugger writing a part of a located INT replaces bits, and
        // leaves the sign extension above them stale.
        MirType::Elementary(e) if e.rk_bits() == 8 => {
            func.instruction(&if e.is_signed() {
                Instruction::I32Load8S(mem_arg(0, 0))
            } else {
                Instruction::I32Load8U(mem_arg(0, 0))
            });
        }
        MirType::Elementary(e) if e.rk_bits() == 16 => {
            func.instruction(&if e.is_signed() {
                Instruction::I32Load16S(mem_arg(0, 1))
            } else {
                Instruction::I32Load16U(mem_arg(0, 1))
            });
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
