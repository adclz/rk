// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use db::WorkspaceDataBase;

use crate::{
    HirNodeInfo,
    hir_def::{
        expressions::{expression::MultibitsPart, spec::ElementarySpec},
        pous::variable::DirectVariable,
    },
    hir_ty::{
        head::signature::infer_signature,
        infer::Infer,
        ty::{CallableType, Type},
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

    /// Through every name to the identity behind it: a variable's declared
    /// type, an alias's target, a callable's return type. The chain is
    /// walked twice over, the second walker two names at a time: they meet
    /// only on a cycle of aliases, `TYPE A : B; B : A;`, which is E1302
    /// where it is declared and resolves to [`Type::Never`] here, the type
    /// of what is already reported. Followed name by name, such a cycle
    /// took every feature that asked for the type's identity down with
    /// the stack, semantic tokens on the declaration first.
    fn normalize_keep_subrange(&self, db: &'db dyn WorkspaceDataBase) -> Type<'db> {
        let mut slow = *self;
        let mut fast = *self;
        loop {
            for _ in 0..2 {
                fast = match fast.behind_name(db) {
                    Ok(next) => next,
                    Err(identity) => return identity,
                };
            }
            slow = match slow.behind_name(db) {
                Ok(next) => next,
                Err(identity) => return identity,
            };
            if slow == fast {
                return Type::Never;
            }
        }
    }

    /// One name of [`Self::normalize_keep_subrange`]'s chain: `Ok` with
    /// what the name stands for, `Err` with the identity when this is no
    /// name to go through.
    fn behind_name(&self, db: &'db dyn WorkspaceDataBase) -> Result<Type<'db>, Type<'db>> {
        match self {
            Type::DataType(dt) => {
                Ok(infer_signature(db, dt.get_scope_id(db)).type_of_specs[&dt.spec(db)])
            }
            Type::Variable((var, multibits)) => {
                if let Some(multibits) = multibits {
                    return Err(multibits_to_type(db, *multibits));
                }
                Ok(infer_signature(db, var.get_scope_id(db)).type_of_specs[&var.spec(db)])
            }
            Type::ReturnValue((_, Some(multibits))) => Err(multibits_to_type(db, *multibits)),
            Type::CallableType(typ) | Type::ReturnValue((typ, None)) => match typ {
                CallableType::Function(f) => match f.return_type(db) {
                    Some(ret_ty) => {
                        Ok(infer_signature(db, f.get_scope_id(db)).type_of_specs[ret_ty])
                    }
                    _ => Err(Type::Void),
                },
                CallableType::MethodDecl(m) => match m.return_type(db) {
                    Some(ret_ty) => {
                        Ok(infer_signature(db, m.get_scope_id(db)).type_of_specs[ret_ty])
                    }
                    _ => Err(Type::Void),
                },
                _ => Err(*self),
            },
            Type::DirectVariable((dv, multibits)) => {
                if let Some(multibits) = multibits {
                    return Err(multibits_to_type(db, *multibits));
                }
                Err(direct_variable_to_type(db, *dv, *multibits))
            }
            Type::StructElement(element) => {
                Ok(infer_signature(db, element.get_scope_id(db)).type_of_specs[&element.spec(db)])
            }
            _ => Err(*self),
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
    match dv.size_letter(db).and_then(access_size) {
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

/// Whether an address names one of the three bands: an area letter (`I`, `Q`
/// or `M`), a width letter or none (a bit, `%I1`), and a complete offset.
/// Everything else — `%Z0`, `%IZ0`, `%I*`, whose binding would come from
/// VAR_CONFIG — has no storage to be given, and is refused with E1417.
///
/// MIR decodes the same address from its text (`crate::located` in the `mir`
/// crate) and must agree: this is the check that keeps lowering from being
/// handed something it has no band for.
pub fn names_a_band(db: &dyn WorkspaceDataBase, dv: DirectVariable) -> bool {
    if dv.partly(db) {
        return false;
    }
    dv.area(db).is_some() && dv.size_letter(db).and_then(access_size).is_some()
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
            // One letter, or none for a bit (`%1` is `%X1`): `%BX1` is no
            // size, not `%B1`.
            let mut letters = access.text(db).chars();
            let (spec, width) = match (letters.next(), letters.next()) {
                (None, _) => (ElementarySpec::Bool, 1),
                (Some(c), None) => access_size(c)?,
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

/// Declared capacity, in bytes, of a plain `STRING` written without an
/// explicit `[N]`. The standard leaves it implementation-defined; 80 is
/// the most common choice and keeps header plus buffer at 84 bytes, cheap
/// to allocate per variable.
///
/// Every layer that has to pick a layout, or refuse a value that will not
/// fit one, reads it from here.
pub const DEFAULT_STRING_CAPACITY: u32 = 80;

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
    declared_string_length(db, spec).and_then(|n| u32::try_from(n).ok())
}

/// The `N` a STRING spec declares, followed through aliases, however large:
/// the layout reports one too large for a module (E0322) where
/// [`declared_string_capacity`] has no capacity to give.
pub fn declared_string_length<'db>(
    db: &'db dyn WorkspaceDataBase,
    spec: crate::hir_def::expressions::spec::Spec<'db>,
) -> Option<u64> {
    declared_string_capacity_inner(db, spec, 0)
}

/// The capacity a STRING spec lays out: its `N`, or
/// [`DEFAULT_STRING_CAPACITY`] for a plain `STRING`. `None` for a spec that
/// is no STRING. Where two layouts must be one — the elements of two arrays,
/// the target of a reference, a VAR_EXTERNAL and its global — two capacities
/// are two types.
pub fn string_capacity<'db>(
    db: &'db dyn WorkspaceDataBase,
    spec: crate::hir_def::expressions::spec::Spec<'db>,
) -> Option<u32> {
    use crate::hir_ty::infer::Infer;
    matches!(
        spec.infer(db).normalize(db),
        Type::Elementary(ElementarySpec::String)
    )
    .then(|| declared_string_capacity(db, spec).unwrap_or(DEFAULT_STRING_CAPACITY))
}

/// [`string_capacity`] of the declaration a value's type still names: a
/// variable, a field, a type, a result. `None` once the type no longer says
/// where it was declared.
pub fn declared_capacity_of<'db>(db: &'db dyn WorkspaceDataBase, ty: Type<'db>) -> Option<u32> {
    let spec = match ty {
        Type::Variable((var, None)) => var.spec(db),
        Type::StructElement(element) => element.spec(db),
        Type::DataType(dt) => dt.spec(db),
        Type::ReturnValue((callable, None)) => *callable.return_type(db)?,
        _ => return None,
    };
    string_capacity(db, spec)
}

fn declared_string_capacity_inner<'db>(
    db: &'db dyn WorkspaceDataBase,
    spec: crate::hir_def::expressions::spec::Spec<'db>,
    depth: u32,
) -> Option<u64> {
    use crate::hir_def::expressions::spec::SpecKind;
    // A cyclic alias is rejected separately (E09xx); stop regardless so a
    // consumer terminates on a body that was compiled anyway.
    if depth > 16 {
        return None;
    }
    match spec.kind(db) {
        // `spec_bound`, not `as_range`: the length may name a CONSTANT
        // (`STRING[SIZE]`), and a literal-only fold would answer None there —
        // which the caller reads as "unsized" and lowers at the default 80,
        // a wrong layout with nothing said. A length that truly does not fold
        // is refused by the check (E0812), so None here means the program was
        // already rejected.
        SpecKind::SizedString(length_expr) => {
            crate::hir_ty::infer::const_eval::spec_bound(db, *length_expr)
                .and_then(|n| u64::try_from(n).ok())
        }
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
