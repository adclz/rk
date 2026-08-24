use db::WorkspaceDataBase;

use crate::{
    HirNodeInfo, hir_def::{
        expressions::{expression::MultibitsPart, spec::ElementarySpec},
        pous::variable::DirectVariable,
    }, hir_ty::{
        head::signature::infer_signature, infer::Infer, ty::{CallableType, Type},
    },
};

/*
    Normalizes a type into its identity data type.
    This is an eager, lossy operation: information about how a value was
    reached (variable access, path steps, call origin, etc.) is discarded.
*/

impl<'db> Type<'db> {
    /// The declared subrange behind this type, resolving alias and variable
    /// indirection but NOT peeling — the one consumer shape [`Self::normalize`]
    /// no longer returns.
    ///
    /// This is for the callers to whom the bounds ARE the point: the
    /// assignment bounds check (`subrange_violation`) and MIR's type lowering,
    /// which carries them into the debug type table.
    pub fn as_subrange(
        &self,
        db: &'db dyn WorkspaceDataBase,
    ) -> Option<crate::hir_def::expressions::spec::SubRange<'db>> {
        match self.normalize_keep_subrange(db) {
            Type::SubRange(sub) => Some(sub),
            _ => None,
        }
    }

    /// Normalize, resolving a subrange to its BASE type.
    ///
    /// Operators, coercion and lowering all act on the base: `INT (0..100)`
    /// adds, compares and stores like an `INT`. Subranges used to survive
    /// normalization, so every such decision needed its own `peel_subrange`
    /// call — and each site that forgot one rejected or mis-lowered subrange
    /// values. The declared bounds still constrain what may be ASSIGNED; the
    /// consumers that need them go through [`Self::as_subrange`].
    pub fn normalize(&self, db: &'db dyn WorkspaceDataBase) -> Type<'db> {
        match self.normalize_keep_subrange(db) {
            Type::SubRange(sub) => sub._type(db).infer(db).normalize(db),
            other => other,
        }
    }

    fn normalize_keep_subrange(&self, db: &'db dyn WorkspaceDataBase) -> Type<'db> {
        match self {
            Type::DataType(dt) => {
                infer_signature(db, dt.get_scope_id(db)).type_of_specs[&dt.spec(db)].normalize_keep_subrange(db)
            }
            Type::Variable((var, multibits)) => {
                if let Some(multibits) = multibits {
                    return multibits_to_type(db, *multibits);
                }
                infer_signature(db, var.get_scope_id(db)).type_of_specs[&var.spec(db)].normalize_keep_subrange(db)
            }
            Type::CallableType(typ) => match typ {
                CallableType::Function(f) => match f.return_type(db) {
                    Some(ret_ty) => {
                        infer_signature(db, f.get_scope_id(db)).type_of_specs[ret_ty].normalize_keep_subrange(db)
                    }
                    _ => Type::Void,
                },
                CallableType::MethodDecl(m) => match m.return_type(db) {
                    Some(ret_ty) => {
                        infer_signature(db, m.get_scope_id(db)).type_of_specs[ret_ty].normalize_keep_subrange(db)
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
                .normalize_keep_subrange(db),
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
    // `%IX1.0`: the location prefix, then the size character.
    match dv.adress(db).text(db).chars().nth(1).and_then(access_size) {
        Some((spec, _)) => Type::Elementary(spec),
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

/// The slice a size character names, in either case, or `None` if it names
/// none. The one place the valid set is written down: the check that refuses
/// an unknown character reads it from here rather than repeating it.
pub fn access_size(c: char) -> Option<(ElementarySpec, usize)> {
    match c.to_ascii_uppercase() {
        'X' => Some((ElementarySpec::Bool, 1)),
        'B' => Some((ElementarySpec::Byte, 8)),
        'W' => Some((ElementarySpec::Word, 16)),
        'D' => Some((ElementarySpec::DWord, 32)),
        'L' => Some((ElementarySpec::LWord, 64)),
        _ => None,
    }
}

/// Decode a `MultibitsPart` into the slice it names, or `None` when the
/// offset is not a plain decimal integer or the size character is unknown.
///
/// The size character is matched in either case, as every other keyword is.
/// The grammar cannot narrow it for us: `adress_identifier` is shared with
/// direct variables, where the address runs to `IX`, `QW`, `MD` and the rest,
/// so anything it accepts arrives here to be decoded.
pub fn multibits_slice(
    db: &dyn WorkspaceDataBase,
    multibits: MultibitsPart,
) -> Option<MultibitsSlice> {
    let (offset, spec, width) = match multibits {
        // A bare offset is a bit access.
        MultibitsPart::Offset(offset) => (offset, ElementarySpec::Bool, 1),
        MultibitsPart::AccessOffset { access, offset } => {
            let (spec, width) = access_size(access.text(db).chars().next()?)?;
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

/// The `N` a spec declares for a STRING, seen through whatever names it.
///
/// [`Type::normalize`] collapses `STRING[N]` and plain `STRING` onto the same
/// type, so the length only survives on the SPEC — this walk is the
/// compensation, kept beside the collapse that makes it necessary. It does
/// not always survive on the spec at hand either: `s : Alias10` where
/// `TYPE Alias10 : STRING[10]` carries a `Target`, and the `SizedString` sits
/// on the data type's own spec one hop away. Following that hop is the
/// difference between a 10-character string and an 80-character one.
pub fn declared_string_capacity<'db>(
    db: &'db dyn WorkspaceDataBase,
    spec: crate::hir_def::expressions::spec::Spec<'db>,
) -> Option<u32> {
    declared_string_capacity_inner(db, spec, 0)
}

fn declared_string_capacity_inner<'db>(
    db: &'db dyn WorkspaceDataBase,
    spec: crate::hir_def::expressions::spec::Spec<'db>,
    depth: u32,
) -> Option<u32> {
    use crate::hir_def::expressions::spec::SpecKind;
    // A cyclic alias is rejected separately (E09xx); stop regardless so a
    // consumer terminates on a body that was compiled anyway.
    if depth > 16 {
        return None;
    }
    match spec.kind(db) {
        SpecKind::SizedString(length_expr) => length_expr.as_range(db).map(|n| n as u32),
        SpecKind::Ref(inner) => declared_string_capacity_inner(db, *inner, depth + 1),
        // Named: ask the data type it resolves to for its own spec. Using the
        // inferred type rather than re-resolving the name keeps the binding
        // resolution's decision.
        SpecKind::Target(_) => match spec.infer(db) {
            Type::DataType(dt) => declared_string_capacity_inner(db, dt.spec(db), depth + 1),
            _ => None,
        },
        _ => None,
    }
}
