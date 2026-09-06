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
use crate::types::MirType;

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

    // `{wasm IN 'op'}`: the type basis prefixes the raw op with the wasm
    // value type of the named variable (IN:REAL -> f32.sqrt, IN:BYTE ->
    // i32.shl).
    let instruction: CompactString = match &decl.type_ref {
        Some(basis) => {
            let elem = elem_of(&operand(basis.ident)?.1);
            match elem {
                // Shifts/rotates get IEC-width semantics (`rk.shl8`,
                // `rk.rotl16`, …), not raw wasm ops: a raw op works at the
                // i32/i64 lane width, so on sub-width types shifted-out bits
                // leak past the type width and rotates wrap at bit 31/63
                // instead of the type's MSB; wasm also masks the count mod
                // lane width, so shift-by-32 on a DWORD would be a no-op
                // instead of 0. emit_stmt.rs lowers each pseudo-op to a
                // mask/guard sequence. 32/64-bit rotates keep the native op:
                // count mod lane width == count mod type width there.
                Some(e)
                    if matches!(decl.instruction.as_str(), "shl" | "shr_u" | "rotl" | "rotr")
                        && !e.is_float() =>
                {
                    let bits = e.rk_bits();
                    match (decl.instruction.as_str(), bits) {
                        ("shl", b) => CompactString::from(format!("rk.shl{}", b)),
                        ("shr_u", b) => CompactString::from(format!("rk.shr{}", b)),
                        ("rotl", b) if b <= 16 => CompactString::from(format!("rk.rotl{}", b)),
                        ("rotr", b) if b <= 16 => CompactString::from(format!("rk.rotr{}", b)),
                        // Full-width rotates are correct natively.
                        ("rotl", 64) => CompactString::from("i64.rotl"),
                        ("rotl", _) => CompactString::from("i32.rotl"),
                        ("rotr", 64) => CompactString::from("i64.rotr"),
                        ("rotr", _) => CompactString::from("i32.rotr"),
                        _ => unreachable!(),
                    }
                }
                Some(e) => {
                    let prefix = if e.is_float() {
                        if e.is_64bit() { "f64" } else { "f32" }
                    } else if e.is_64bit() {
                        "i64"
                    } else {
                        "i32"
                    };
                    CompactString::from(format!("{}.{}", prefix, decl.instruction))
                }
                None => decl.instruction.clone(),
            }
        }
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
