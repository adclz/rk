//! Partial (bit / byte / word) access: `b.1`, `w.%X3`, `d.%B2`, `l.%W1`.
//! HIR owns the meaning ([`multibits_slice`], E0808); this module turns its
//! answer into shift/mask arithmetic at the base value's wasm width. A read
//! is `(base >> shift) & mask`; a write is the matching read-modify-write.

use hir::{
    hir_def::{
        expressions::expression::{Expr, VariableAccess, VariableAccessKind},
        interned::identifier::Ident,
        pous::variable::LocatedAddress,
    },
    hir_ty::{
        index_graphs::{effective_location, located_declaration, located_view},
        infer::{Infer, normalize::multibits_slice},
        ty::Type,
    },
};

use crate::{
    expr::{MirBinOp, MirConstant, MirExpr, MirPlace},
    located::width_elementary,
    lower::{lower_expr::ExprLowerCtx, lower_type::LowerTypeError},
    types::{MirElementary, MirType},
};

/// The base value a partial access slices, and the slice itself.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Sliced {
    /// Type of the whole variable being sliced (`BYTE` in `b.1`).
    pub base: MirElementary,
    /// Bit position of the slice's low bit.
    pub shift: u32,
    /// Type the slice reads back as (`BOOL` in `b.1`).
    pub elem: MirElementary,
}

/// The elementary type a lowered place holds, when the place records it:
/// a slice reached through a field or an index arrives with its HIR type
/// collapsed to the slice's, and the place keeps the base width.
fn place_elementary(place: &MirPlace) -> Option<MirElementary> {
    match place {
        MirPlace::Field {
            field_type: MirType::Elementary(e),
            ..
        }
        | MirPlace::Index {
            element_type: MirType::Elementary(e),
            ..
        } => Some(*e),
        _ => None,
    }
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
        place: &MirPlace,
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
        let base = match hir_ty {
            Type::Variable((var, _)) => {
                self.type_to_mir_elementary_pub(var.spec(self.db).infer(self.db))?
            }
            Type::DirectVariable(_) => {
                return Err(LowerTypeError::UnsupportedType(
                    "partial access on a directly represented variable is not supported"
                        .to_string(),
                ));
            }
            // Through a field or an index HIR hands over the slice's own type, so
            // the width comes from the place.
            other => match place_elementary(place) {
                Some(base) => base,
                None => self.type_to_mir_elementary_pub(other)?,
            },
        };

        // Lowering can run on a rejected body (E0808): refuse rather than wrap.
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

    /// When `var_access` is a VIEW — an address stored inside a wider one the
    /// workspace mentions — where in the owner's cell it is. `hir_ty` is the
    /// access's type before normalizing, which for a variable still names the
    /// declaration.
    ///
    /// The owner is named by its address, and lowering gives that name the
    /// owner's cell: the declared one when a VAR_GLOBAL is located there, a
    /// cell of its own otherwise. A view never gets one, so a nest of
    /// addresses is one cell however many of them the program mentions.
    pub(crate) fn view(
        &self,
        var_access: VariableAccess<'db>,
        hir_ty: Type<'db>,
    ) -> Result<Option<View>, LowerTypeError> {
        let address = match var_access.kind(self.db) {
            VariableAccessKind::Direct(dv) => LocatedAddress::of(self.db, dv),
            VariableAccessKind::Symbolic(_) => match hir_ty {
                Type::Variable((var, _)) => {
                    effective_location(self.db, var).and_then(|dv| LocatedAddress::of(self.db, dv))
                }
                _ => None,
            },
        };
        let Some(address) = address else {
            return Ok(None);
        };
        let Some(view) = located_view(self.db, &address) else {
            return Ok(None);
        };
        let owner_name = Ident::new(
            self.db,
            compact_str::CompactString::from(view.owner.text.as_str()),
        );
        let cell = MirPlace::Global {
            name: Some(owner_name),
            address: 0,
            ty: MirType::Elementary(width_elementary(view.owner.width)),
        };
        // What the view reads as: its declared type, or the width's own bit
        // string for a bare address.
        let ty = match hir_ty {
            Type::Variable((var, _)) => {
                self.type_to_mir_elementary_pub(var.spec(self.db).infer(self.db))?
            }
            _ => width_elementary(address.width),
        };

        // Four bytes of an eight-byte cell, which memory holds little-endian
        // like the image: the view is those bytes, read and written as its own
        // type. Only a 32-bit part is reached this way, because rk keeps every
        // narrower scalar in a four-byte slot.
        if address.width == 32 && var_access.multibits(self.db).is_none() {
            return Ok(Some(View::Bytes(MirPlace::Field {
                base: Box::new(cell),
                field_name: owner_name,
                field_offset: view.shift / 8,
                field_type: MirType::Elementary(ty),
            })));
        }

        // The cell is sliced as an integer of its width. That is its declared
        // type when it is one, so a signed owner is rebuilt signed
        // (`slice_write`); otherwise the bit string, which a REAL or a TIME
        // is in memory, reached as a field at offset 0 so the loads and
        // stores are integer ones.
        let declared = located_declaration(self.db, &view.owner).and_then(|v| {
            self.type_to_mir_elementary_pub(v.spec(self.db).infer(self.db))
                .ok()
        });
        let (owner, base) = match declared {
            Some(e) if e.is_integer() => (cell, e),
            None => (cell, width_elementary(view.owner.width)),
            Some(_) => {
                let bits = width_elementary(view.owner.width);
                let place = MirPlace::Field {
                    base: Box::new(cell),
                    field_name: owner_name,
                    field_offset: 0,
                    field_type: MirType::Elementary(bits),
                };
                (place, bits)
            }
        };
        let mut sliced = Sliced {
            base,
            shift: view.shift,
            elem: width_elementary(address.width),
        };
        let mut ty = ty;
        // A partial access of a view is a narrower slice of the same owner.
        if var_access.multibits(self.db).is_some() {
            let inner = self.resolve_slice(&owner, var_access, hir_ty)?;
            sliced = Sliced {
                base,
                shift: view.shift + inner.shift,
                elem: inner.elem,
            };
            ty = inner.elem;
        }
        Ok(Some(View::Slice { owner, sliced, ty }))
    }

    /// Lower a read of a partial access to `(base >> shift) & mask`.
    pub(crate) fn lower_multibit_read(
        &self,
        place: MirPlace,
        var_access: VariableAccess<'db>,
        parent_expr: Expr<'db>,
    ) -> Result<MirExpr, LowerTypeError> {
        let sliced = self.resolve_slice(&place, var_access, parent_expr.infer(self.db))?;
        Ok(slice_read(place, sliced))
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
        let sliced = self.resolve_slice(&place, var_access, hir_ty)?;
        Ok(slice_write(place, sliced, value))
    }
}

/// Where a view is in its owner's cell (`ExprLowerCtx::view`).
#[derive(Debug, Clone)]
pub(crate) enum View {
    /// Whole bytes of the cell, with an address of their own.
    Bytes(MirPlace),
    /// Bits of the cell, read as `ty`: the slice's own type, or one of the
    /// same width and lane (a SINT part is read as a BYTE, then
    /// sign-extended).
    Slice {
        owner: MirPlace,
        sliced: Sliced,
        ty: MirElementary,
    },
}

impl View {
    /// The type the view reads and is written as.
    pub(crate) fn ty(&self) -> MirElementary {
        match self {
            View::Bytes(MirPlace::Field {
                field_type: MirType::Elementary(e),
                ..
            }) => *e,
            View::Bytes(_) => unreachable!("a view's bytes are a field of its owner's cell"),
            View::Slice { ty, .. } => *ty,
        }
    }

    pub(crate) fn read(self) -> MirExpr {
        let ty = self.ty();
        match self {
            View::Bytes(place) => MirExpr::Load(place, MirType::Elementary(ty)),
            View::Slice { owner, sliced, ty } => {
                let bits = slice_read(owner, sliced);
                if ty == sliced.elem {
                    bits
                } else {
                    MirExpr::Cast {
                        expr: Box::new(bits),
                        from: sliced.elem,
                        to: ty,
                    }
                }
            }
        }
    }

    /// The lane `write`'s store is made at: the view's own, or its owner's.
    pub(crate) fn store_lane(&self) -> MirElementary {
        match self {
            View::Bytes(_) => self.ty(),
            View::Slice { sliced, .. } => sliced.base,
        }
    }

    /// The store that puts `value` in the view: into its bytes, or into the
    /// whole owner with the slice replaced.
    pub(crate) fn write(self, value: MirExpr) -> (MirPlace, MirExpr) {
        match self {
            View::Bytes(place) => (place, value),
            View::Slice { owner, sliced, .. } => {
                let value = slice_write(owner.clone(), sliced, value);
                (owner, value)
            }
        }
    }
}

/// `(base >> shift) & mask`, read from `place`.
pub(crate) fn slice_read(place: MirPlace, sliced: Sliced) -> MirExpr {
    let Sliced { base, shift, elem } = sliced;
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
    value
}

/// The whole of `place` with the slice replaced by `value`:
/// `(base & !(mask << shift)) | ((value & mask) << shift)`.
pub(crate) fn slice_write(place: MirPlace, sliced: Sliced, value: MirExpr) -> MirExpr {
    let Sliced { base, shift, elem } = sliced;
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
    let rebuilt = binop(MirBinOp::Or, kept, incoming, base);
    // Rebuilt inside the base's own width, the value is zero-extended above
    // it, while a signed 8- or 16-bit value is held sign-extended in its lane.
    // The cast puts it back in its domain: -8 with bit 0 set is -7, not 65529.
    if base.is_signed() && base.rk_bits() < 32 {
        MirExpr::Cast {
            expr: Box::new(rebuilt),
            from: MirElementary::DWord,
            to: base,
        }
    } else {
        rebuilt
    }
}
