// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! A `{wasm}` statement emits the instruction it names on the operands it
//! names and writes the parameter, local or return slot its `(result)`
//! names. A body may hold several, in order.

use compact_str::CompactString;
use hir::hir_def::{
    expressions::statement::Stmt, extern_decl::WasmDecl, interned::identifier::Ident,
    pous::pou::Pou, scope::ScopeKind, semantic_index::get_scope,
};

use super::lower_expr::ExprLowerCtx;
use super::lower_func::lower_var_type;
use super::lower_type::{LowerTypeError, lower_spec};
use crate::expr::{MirExpr, MirPlace};
use crate::stmt::MirStmt;
use crate::types::{MirElementary, MirType};

pub(crate) fn lower_wasm_pragma<'db>(
    ctx: &ExprLowerCtx<'db>,
    stmt: Stmt<'db>,
    decl: &WasmDecl<'db>,
) -> Result<Option<MirStmt>, LowerTypeError> {
    let db = ctx.db;
    let scope = stmt.scope_id(db);
    let function = match get_scope(db, scope).kind {
        ScopeKind::Pou(Pou::Function(f)) => Some(f),
        _ => None,
    };

    let operand_var = |ident: Ident| {
        let def_map = scope.def_map(db);
        def_map
            .local_variables
            .get(&ident)
            .or_else(|| def_map.global_variables.get(&ident))
            .copied()
            .ok_or_else(|| {
                LowerTypeError::UnsupportedType(format!(
                    "'{}' is not an operand of this FUNCTION",
                    ident.text(db)
                ))
            })
    };
    // An operand is resolved to the ident it was DECLARED under: the MIR
    // locals are keyed by that spelling, and the pragma may use another case.
    let operand = |ident: Ident| -> Result<(Ident, MirType), LowerTypeError> {
        if let Some(f) = function
            && f.name(db) == ident
        {
            let ty = f
                .return_type(db)
                .map(|spec| lower_spec(db, *spec))
                .transpose()?
                .ok_or_else(|| {
                    LowerTypeError::UnsupportedType(format!(
                        "'{}' returns nothing to write",
                        ident.text(db)
                    ))
                })?;
            return Ok((f.name(db), ty));
        }
        let var = operand_var(ident)?;
        // An `ARRAY[*]` is the array this copy is specialized for.
        let ty = match ctx.shape_of(hir::hir_ty::ty::Type::new_var(db, var)) {
            Some(shape) => shape,
            None => lower_var_type(db, var)?,
        };
        Ok((var.name(db), ty))
    };
    if hir::check::wasm_instructions::ARRAY.contains(&decl.instruction.as_str()) {
        return lower_array_intrinsic(ctx, decl, &operand).map(Some);
    }
    let elem_of = |ty: &MirType| match ty {
        MirType::Elementary(e) => Some(*e),
        _ => None,
    };

    // `{wasm IN 'op'}`: the basis variable's lane and width name the
    // instruction, by the same function the check resolved it with.
    let instruction: CompactString = match &decl.type_ref {
        Some(basis) => match elem_of(&operand(basis.ident(db))?.1) {
            Some(e) => hir::check::wasm_instructions::resolve_type_basis(
                &decl.instruction,
                lane_of(e),
                e.rk_bits(),
            ),
            None => decl.instruction.clone(),
        },
        None => decl.instruction.clone(),
    };

    // A single-operand conversion with no type basis is a cast: `mir_cast.rs`
    // picks the op and the sub-width normalization. Reinterprets stay out;
    // `nop` and `cast` are the library's placeholders for "the types decide".
    let is_conversion = matches!(decl.instruction.as_str(), "nop" | "cast")
        || [
            ".convert_",
            ".trunc_",
            ".extend",
            ".wrap_",
            ".promote_",
            ".demote_",
        ]
        .iter()
        .any(|op| decl.instruction.contains(op));
    if decl.type_ref.is_none()
        && is_conversion
        && let [param] = decl.params.as_slice()
        && let Some(result) = &decl.result
    {
        let (p_name, p_ty) = operand(param.ident(db))?;
        let (r_name, r_ty) = operand(result.ident(db))?;
        if let (Some(from), Some(to)) = (elem_of(&p_ty), elem_of(&r_ty)) {
            let load = MirExpr::Load(MirPlace::Local(p_name), MirType::Elementary(from));
            let value = if from == to {
                load
            } else {
                MirExpr::Cast {
                    expr: Box::new(load),
                    from,
                    to,
                }
            };
            return Ok(Some(MirStmt::Assign {
                target: MirPlace::Local(r_name),
                value,
            }));
        }
    }

    // A bare `nop` has nothing to emit.
    if decl.instruction == "nop" && decl.params.is_empty() && decl.result.is_none() {
        return Ok(None);
    }

    let params = decl
        .params
        .iter()
        .map(|p| operand(p.ident(db)).map(|(name, _)| name))
        .collect::<Result<Vec<_>, _>>()?;
    let result = decl
        .result
        .as_ref()
        .map(|r| operand(r.ident(db)).map(|(name, _)| name))
        .transpose()?;
    Ok(Some(MirStmt::WasmIntrinsic {
        instruction,
        params,
        result,
    }))
}

/// `array.dimensions`, `array.lower_bound` or `array.upper_bound`: what the
/// array's type says, read in this copy of the FUNCTION, a constant. A bound
/// is one per dimension, which the `DIM` operand picks. One outside them
/// leaves the result as it was: the library's FUNCTIONs check `DIM` first. A
/// value the result's integer type cannot hold raises, as a store into a
/// subrange does.
fn lower_array_intrinsic<'db>(
    ctx: &ExprLowerCtx<'db>,
    decl: &WasmDecl<'db>,
    operand: &dyn Fn(Ident) -> Result<(Ident, MirType), LowerTypeError>,
) -> Result<MirStmt, LowerTypeError> {
    use crate::expr::{MirBinOp, MirConstant};
    let db = ctx.db;
    let checked = || {
        LowerTypeError::UnsupportedType(format!(
            "`{}` was refused by the check, and still lowered",
            decl.instruction
        ))
    };
    let integer = |(name, ty): (Ident, MirType)| match ty {
        MirType::Elementary(lane) if lane.is_integer() => Ok((name, lane)),
        _ => Err(checked()),
    };
    let constant = |lane: MirElementary, value: i64| match lane.is_64bit() {
        true => MirConstant::I64(value),
        false => MirConstant::I32(value as i32),
    };
    let array = match decl.params.first().map(|p| operand(p.ident(db))) {
        Some(Ok((_, MirType::Array(array)))) => array,
        Some(Err(err)) => return Err(err),
        _ => return Err(checked()),
    };
    let (result, lane) = integer(operand(
        decl.result.as_ref().ok_or_else(checked)?.ident(db),
    )?)?;
    let store = |value: i64| match holds(lane, value) {
        true => MirStmt::Assign {
            target: MirPlace::Local(result),
            value: MirExpr::Constant(constant(lane, value)),
        },
        false => {
            let (id, len) = ctx
                .string_pool
                .borrow_mut()
                .intern(b"array bound out of range of the result");
            MirStmt::Raise {
                message: MirExpr::StringLiteral { id, len },
            }
        }
    };
    if decl.instruction == "array.dimensions" {
        return Ok(store(array.dimensions.len() as i64));
    }
    let upper = decl.instruction == "array.upper_bound";
    let (dim, dim_lane) = integer(operand(decl.params.get(1).ok_or_else(checked)?.ident(db))?)?;
    let mut arms = array
        .dimensions
        .iter()
        .enumerate()
        .map(|(i, (lower, bound))| {
            let condition = MirExpr::BinOp {
                op: MirBinOp::Eq,
                lhs: Box::new(MirExpr::Load(
                    MirPlace::Local(dim),
                    MirType::Elementary(dim_lane),
                )),
                rhs: Box::new(MirExpr::Constant(constant(dim_lane, i as i64 + 1))),
                ty: dim_lane,
            };
            (condition, vec![store(if upper { *bound } else { *lower })])
        });
    let (condition, then_body) = arms.next().ok_or_else(checked)?;
    Ok(MirStmt::If {
        condition,
        then_body,
        else_ifs: arms.collect(),
        else_body: None,
    })
}

/// Whether an integer of type `lane` holds `value`.
fn holds(lane: MirElementary, value: i64) -> bool {
    let bits = lane.rk_bits();
    match (lane.is_signed(), bits >= 64) {
        (true, true) => true,
        (true, false) => (-(1i64 << (bits - 1))..1i64 << (bits - 1)).contains(&value),
        (false, true) => value >= 0,
        (false, false) => (0..1i64 << bits).contains(&value),
    }
}

/// The wasm lane of a MIR scalar.
fn lane_of(e: MirElementary) -> hir::check::wasm_instructions::Lane {
    use hir::check::wasm_instructions::Lane;
    match (e.is_float(), e.is_64bit()) {
        (true, true) => Lane::F64,
        (true, false) => Lane::F32,
        (false, true) => Lane::I64,
        (false, false) => Lane::I32,
    }
}

/// The check derives lanes and IEC widths from `ElementarySpec`, the MIR
/// from `MirElementary`: pinned equal here for every scalar.
#[cfg(test)]
mod lane_parity {
    use hir::check::wasm_instructions::{Lane, lane_of, rk_bits_of};
    use hir::hir_def::expressions::spec::ElementarySpec;

    #[test]
    fn every_scalar_agrees_on_lane_and_width() {
        use ElementarySpec::*;
        for spec in [
            Bool,
            Byte,
            Word,
            DWord,
            LWord,
            SInt,
            USInt,
            UInt,
            Int,
            DInt,
            UDInt,
            LInt,
            ULInt,
            Real,
            LReal,
            Char,
            Date,
            LDate,
            DateAndTime,
            LDateTime,
            Time,
            LTime,
            Tod,
            LTod,
        ] {
            let mir = crate::lower::lower_type::elementary_spec_to_mir(spec)
                .unwrap_or_else(|e| panic!("{spec:?} has no MIR scalar: {e:?}"));
            let lane = lane_of(spec).unwrap_or_else(|| panic!("{spec:?} has no lane"));
            let expected = match (mir.is_float(), mir.is_64bit()) {
                (true, true) => Lane::F64,
                (true, false) => Lane::F32,
                (false, true) => Lane::I64,
                (false, false) => Lane::I32,
            };
            assert_eq!(lane, expected, "{spec:?}: lane");
            assert_eq!(rk_bits_of(spec), mir.rk_bits(), "{spec:?}: IEC width");
        }
    }
}
