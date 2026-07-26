//! Partial (bit / byte / word) access — `b.1`, `w.%X3`, `d.%B2`, `l.%W1`.
//!
//! IEC 61131-3 §6.5.5 lets a variable be addressed one slice at a time. HIR
//! owns the *meaning* of such an access: [`multibits_slice`] decodes the
//! `%X/%B/%W/%D/%L` size character into a bit offset and a result type, and
//! body inference range-checks the offset against the base type's width
//! (`E0229`). This module only turns HIR's answer into shift/mask arithmetic
//! — it never re-decides what an access denotes.
//!
//! A read is `(base >> shift) & mask`; a write is the matching
//! read-modify-write, `base := (base & !(mask << shift)) | ((v & mask) << shift)`.
//! Both are computed at the *base* value's wasm width, so a slice of an
//! `LWORD` shifts in i64 and only narrows to i32 once the slice is isolated.

use hir::{
    hir_def::expressions::expression::{Expr, VariableAccess},
    hir_ty::{
        infer::{Infer, normalize::multibits_slice},
        ty::Type,
    },
};

use crate::{
    expr::{MirBinOp, MirConstant, MirExpr, MirPlace},
    lower::{lower_expr::ExprLowerCtx, lower_type::LowerTypeError},
    types::{MirElementary, MirType},
};

/// The base value a partial access slices, and the slice itself.
struct Sliced {
    /// Type of the whole variable being sliced (`BYTE` in `b.1`).
    base: MirElementary,
    /// Bit position of the slice's low bit.
    shift: u32,
    /// Type the slice reads back as (`BOOL` in `b.1`).
    elem: MirElementary,
}

/// All-ones mask for the low `bits` bits, as an `i64` bit pattern.
fn low_mask(bits: u32) -> i64 {
    if bits >= 64 {
        -1
    } else {
        ((1u64 << bits) - 1) as i64
    }
}

/// An integer constant of `elem`'s wasm width.
fn const_of(elem: MirElementary, value: i64) -> MirExpr {
    if elem.is_64bit() {
        MirExpr::Constant(MirConstant::I64(value))
    } else {
        MirExpr::Constant(MirConstant::I32(value as i32))
    }
}

fn binop(op: MirBinOp, lhs: MirExpr, rhs: MirExpr, ty: MirElementary) -> MirExpr {
    MirExpr::BinOp {
        op,
        lhs: Box::new(lhs),
        rhs: Box::new(rhs),
        ty,
    }
}

impl<'db> ExprLowerCtx<'db> {
    /// Resolve the base type and slice geometry; `hir_ty` is the
    /// unnormalized type, which still carries the base width.
    fn resolve_slice(
        &self,
        var_access: VariableAccess<'db>,
        hir_ty: Type<'db>,
    ) -> Result<Sliced, LowerTypeError> {
        let Some(multibits) = var_access.multibits(self.db) else {
            return Err(LowerTypeError::UnsupportedType(
                "partial access lowering called on a plain variable access".to_string(),
            ));
        };
        let Some(slice) = multibits_slice(self.db, multibits) else {
            return Err(LowerTypeError::UnsupportedType(
                "unrecognized partial access size (expected %X, %B, %W, %D or %L)".to_string(),
            ));
        };

        // The base type comes from the declaration HIR bound the access to.
        let base_ty = match hir_ty {
            Type::Variable((var, _)) => var.spec(self.db).infer(self.db),
            Type::DirectVariable(_) => {
                return Err(LowerTypeError::UnsupportedType(
                    "partial access on a directly represented variable is not supported"
                        .to_string(),
                ));
            }
            other => other,
        };
        let base = self.type_to_mir_elementary_pub(base_ty)?;

        // HIR reports an out-of-range offset (E0229) but lowering can still be
        // asked to run on a rejected body; refuse rather than emit a shift
        // that silently wraps modulo the wasm operand width.
        if slice.shift + slice.width > base.rk_bits() as usize {
            return Err(LowerTypeError::UnsupportedType(format!(
                "partial access reaches bit {} of a {}-bit value",
                slice.shift + slice.width,
                base.rk_bits()
            )));
        }

        Ok(Sliced {
            base,
            shift: slice.shift as u32,
            elem: self.type_to_mir_elementary_pub(Type::Elementary(slice.spec))?,
        })
    }

    /// Lower a read of a partial access to `(base >> shift) & mask`.
    pub(crate) fn lower_multibit_read(
        &self,
        place: MirPlace,
        var_access: VariableAccess<'db>,
        parent_expr: Expr<'db>,
    ) -> Result<MirExpr, LowerTypeError> {
        let Sliced { base, shift, elem } =
            self.resolve_slice(var_access, parent_expr.infer(self.db))?;

        let mut value = MirExpr::Load(place, MirType::Elementary(base));
        if shift > 0 {
            value = binop(MirBinOp::Shr, value, const_of(base, shift as i64), base);
        }
        // Narrow before masking so the mask constant matches the operand width.
        if base.is_64bit() && !elem.is_64bit() {
            value = MirExpr::Cast {
                expr: Box::new(value),
                from: base,
                to: MirElementary::DWord,
            };
        }
        let width = elem.rk_bits();
        let operand = if elem.is_64bit() {
            elem
        } else {
            MirElementary::DWord
        };
        if width < operand.rk_bits() {
            value = binop(
                MirBinOp::And,
                value,
                const_of(operand, low_mask(width)),
                operand,
            );
        }
        Ok(value)
    }

    /// Lower `place.<slice> := value` as a read-modify-write of the whole
    /// base: `base := (base & !(mask << shift)) | ((value & mask) << shift)`,
    /// inside the base's own bit width.
    pub(crate) fn lower_multibit_write(
        &self,
        place: MirPlace,
        var_access: VariableAccess<'db>,
        hir_ty: Type<'db>,
        value: MirExpr,
    ) -> Result<MirExpr, LowerTypeError> {
        let Sliced { base, shift, elem } = self.resolve_slice(var_access, hir_ty)?;

        // Widen the incoming slice value to the base's wasm width so the mask
        // and shift below operate on one operand type.
        let value = if base.is_64bit() && !elem.is_64bit() {
            MirExpr::Cast {
                expr: Box::new(value),
                from: elem,
                to: base,
            }
        } else {
            value
        };

        let slice_mask = low_mask(elem.rk_bits());
        let base_mask = low_mask(base.rk_bits());
        // Confined to the base's width, so a sub-width base keeps a clean lane.
        let keep_mask = base_mask & !(slice_mask << shift);

        let mut incoming = binop(MirBinOp::And, value, const_of(base, slice_mask), base);
        if shift > 0 {
            incoming = binop(MirBinOp::Shl, incoming, const_of(base, shift as i64), base);
        }
        let kept = binop(
            MirBinOp::And,
            MirExpr::Load(place, MirType::Elementary(base)),
            const_of(base, keep_mask),
            base,
        );
        Ok(binop(MirBinOp::Or, kept, incoming, base))
    }
}
