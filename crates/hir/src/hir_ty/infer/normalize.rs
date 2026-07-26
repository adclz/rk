use db::WorkspaceDataBase;

use crate::{
    HirNodeInfo,
    hir_def::{
        expressions::{expression::MultibitsPart, spec::ElementarySpec},
        pous::variable::DirectVariable,
    },
    hir_ty::{
        head::signature::infer_signature,
        ty::{CallableType, Type},
    },
};

/*
    Normalizes a type into its identity data type.
    This is an eager, lossy operation: information about how a value was
    reached (variable access, path steps, call origin, etc.) is discarded.
*/

impl<'db> Type<'db> {
    /// The base type behind a subrange; any other type is returned normalized.
    ///
    /// Operators act on the BASE type: `INT (0..100)` adds and compares like an
    /// `INT`. A subrange is not `Type::Elementary`, so operator support
    /// (`is_numeric`) and the widening lattice would otherwise reject it —
    /// `a := a + 5` on a subrange variable failed with "operator '+' cannot be
    /// applied". The declared bounds still constrain what may be ASSIGNED to
    /// the variable; they do not restrict the arithmetic itself.
    pub fn peel_subrange(&self, db: &'db dyn WorkspaceDataBase) -> Type<'db> {
        match self.normalize(db) {
            Type::SubRange(sub) => {
                crate::hir_ty::infer::Infer::infer(&sub._type(db), db).normalize(db)
            }
            other => other,
        }
    }

    pub fn normalize(&self, db: &'db dyn WorkspaceDataBase) -> Type<'db> {
        match self {
            Type::DataType(dt) => {
                infer_signature(db, dt.get_scope_id(db)).type_of_specs[&dt.spec(db)].normalize(db)
            }
            Type::Variable((var, multibits)) => {
                if let Some(multibits) = multibits {
                    return multibits_to_type(db, *multibits);
                }
                infer_signature(db, var.get_scope_id(db)).type_of_specs[&var.spec(db)].normalize(db)
            }
            Type::CallableType(typ) => match typ {
                CallableType::Function(f) => match f.return_type(db) {
                    Some(ret_ty) => {
                        infer_signature(db, f.get_scope_id(db)).type_of_specs[ret_ty].normalize(db)
                    }
                    _ => Type::Void,
                },
                CallableType::MethodDecl(m) => match m.return_type(db) {
                    Some(ret_ty) => {
                        infer_signature(db, m.get_scope_id(db)).type_of_specs[ret_ty].normalize(db)
                    }
                    _ => Type::Void,
                },
                _ => *self,
            },
            Type::DirectVariable((dv, multibits)) => {
                if let Some(multibits) = multibits {
                    return multibits_to_type(db, *multibits);
                }
                direct_variable_to_type(db, *dv, *multibits)
            }
            Type::StructElement(element) => infer_signature(db, element.get_scope_id(db))
                .type_of_specs[&element.spec(db)]
                .normalize(db),
            _ => *self,
        }
    }
}

// Table 16 – Directly represented variables
pub fn direct_variable_to_type<'db>(
    db: &'db dyn WorkspaceDataBase,
    dv: DirectVariable,
    multibits: Option<MultibitsPart>,
) -> Type<'db> {
    let chars = dv.adress(db).text(db).chars();
    // XBWDL
    match dv.adress(db).text(db).chars().nth(1) {
        // BIT
        Some('X') => Type::new_bool(),
        // BYTE
        Some('B') => Type::Elementary(ElementarySpec::Byte),
        // WORD
        Some('W') => Type::Elementary(ElementarySpec::Word),
        // DWORD
        Some('D') => Type::Elementary(ElementarySpec::DWord),
        // LWORD
        Some('L') => Type::Elementary(ElementarySpec::LWord),
        _ => {
            /* err: invalid location type */
            Type::Never
        }
    }
}

// Table 17 – Partial access of ANY_BIT variables
/// A partial (bit / byte / word) access resolved to the slice of the base
/// value it names.
///
/// IEC 61131-3 §6.5.5: `v.%<size><n>` selects the `n`-th slice of `<size>`
/// bits, so the slice's low bit sits at `n * width`. A bare `v.<n>` means
/// `v.%X<n>`.
///
/// This is the single decoder for `%X/%B/%W/%D/%L`: the type of the access,
/// its bounds check and its codegen all read the same answer from here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MultibitsSlice {
    /// Bit position of the slice's low bit within the base value.
    pub shift: usize,
    /// Slice width in bits - 1, 8, 16, 32 or 64.
    pub width: usize,
    /// The type the slice reads back as.
    pub spec: ElementarySpec,
    /// `n` as written, for diagnostics.
    pub index: usize,
}

/// Decode a `MultibitsPart` into the slice it names, or `None` when the
/// offset is not a plain decimal integer or the size character is unknown
/// (both already rejected upstream by the grammar).
pub fn multibits_slice(
    db: &dyn WorkspaceDataBase,
    multibits: MultibitsPart,
) -> Option<MultibitsSlice> {
    let (offset, spec, width) = match multibits {
        // A bare offset is a bit access.
        MultibitsPart::Offset(offset) => (offset, ElementarySpec::Bool, 1),
        MultibitsPart::AccessOffset { access, offset } => {
            let (spec, width) = match access.text(db).chars().next()? {
                'X' => (ElementarySpec::Bool, 1),
                'B' => (ElementarySpec::Byte, 8),
                'W' => (ElementarySpec::Word, 16),
                'D' => (ElementarySpec::DWord, 32),
                'L' => (ElementarySpec::LWord, 64),
                _ => return None,
            };
            (offset, spec, width)
        }
    };
    let index = offset.ident(db).text(db).parse::<usize>().ok()?;
    Some(MultibitsSlice {
        shift: index * width,
        width,
        spec,
        index,
    })
}

pub fn multibits_to_type<'db>(
    db: &'db dyn WorkspaceDataBase,
    multibits: MultibitsPart,
) -> Type<'db> {
    // An undecodable access falls back to BOOL, the bare-offset reading.
    multibits_slice(db, multibits)
        .map(|slice| Type::Elementary(slice.spec))
        .unwrap_or_else(Type::new_bool)
}
