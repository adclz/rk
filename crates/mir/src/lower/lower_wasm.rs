//! A `{wasm}` statement emits the instruction it names on the operands it
//! names and writes the parameter, local or return slot its `(result)`
//! names. A body may hold several, in order.

use compact_str::CompactString;
use hir::hir_def::{
    expressions::statement::Stmt, extern_decl::WasmDecl, interned::identifier::Ident,
    pous::pou::Pou, scope::ScopeKind, semantic_index::get_scope,
};
use hir::hir_ty::infer::Infer;

use super::lower_expr::ExprLowerCtx;
use super::lower_func::lower_var_type;
use super::lower_type::{LowerTypeError, lower_type};
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

    // An operand is resolved to the ident it was DECLARED under: the MIR
    // locals are keyed by that spelling, and the pragma may use another case.
    let operand = |ident: Ident| -> Result<(Ident, MirType), LowerTypeError> {
        if let Some(f) = function
            && f.name(db).caseless(db) == ident.caseless(db)
        {
            let ty = f
                .return_type(db)
                .map(|spec| lower_type(db, spec.infer(db)))
                .transpose()?
                .ok_or_else(|| {
                    LowerTypeError::UnsupportedType(format!(
                        "'{}' returns nothing to write",
                        ident.text(db)
                    ))
                })?;
            return Ok((f.name(db), ty));
        }
        let def_map = scope.def_map(db);
        let key = ident.caseless(db);
        let var = def_map
            .local_variables
            .get(&key)
            .or_else(|| def_map.global_variables.get(&key))
            .ok_or_else(|| {
                LowerTypeError::UnsupportedType(format!(
                    "'{}' is not an operand of this FUNCTION",
                    ident.text(db)
                ))
            })?;
        Ok((var.name(db), lower_var_type(db, *var)?))
    };
    let elem_of = |ty: &MirType| match ty {
        MirType::Elementary(e) => Some(*e),
        _ => None,
    };

    // `{wasm IN 'op'}`: the basis variable's lane and width name the
    // instruction, by the same function the check resolved it with.
    let instruction: CompactString = match &decl.type_ref {
        Some(basis) => match elem_of(&operand(basis.ident)?.1) {
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
        let (p_name, p_ty) = operand(param.ident)?;
        let (r_name, r_ty) = operand(result.ident)?;
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
        .map(|p| operand(p.ident).map(|(name, _)| name))
        .collect::<Result<Vec<_>, _>>()?;
    let result = decl
        .result
        .as_ref()
        .map(|r| operand(r.ident).map(|(name, _)| name))
        .transpose()?;
    Ok(Some(MirStmt::WasmIntrinsic {
        instruction,
        params,
        result,
    }))
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
    use hir::check::wasm_instructions::{Lane, rk_bits_of, lane_of};
    use hir::hir_def::expressions::spec::ElementarySpec;

    #[test]
    fn every_scalar_agrees_on_lane_and_width() {
        use ElementarySpec::*;
        for spec in [
            Bool,
            REDGEBool,
            FEDGEBool,
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
