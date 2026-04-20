//! Derived generic parameter list for FBs/Classes.
//!
//! IEC 61131-3 keeps `ANY_*` as specs on `VAR_INPUT` (etc.). For FBs and
//! Classes - which, unlike functions, have *storage* and can appear inside
//! structs - we require explicit type arguments at every use site. The
//! parameter list itself stays implicit: it's derived from the ordered,
//! *deduplicated* set of `ANY_*` specs appearing in the POU's top-level
//! variable sections.
//!
//! Only the POU's own fields are counted - method `ANY_*` specs remain
//! method-local (handled by the existing call-site inference path).

use db::WorkspaceDataBase;

use crate::hir_def::{
    expressions::spec::{ElementarySpec, Spec, SpecKind},
    interned::identifier::Ident,
    pous::{class::Class, function_block::FunctionBlock, pou::Pou, variable::VariableDecl},
};

/// A single implicit generic parameter derived from an FB/Class declaration.
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct GenericParam {
    /// The underlying `ANY_*` bound that concrete arguments must satisfy.
    pub bound: ElementarySpec,
    /// The first variable whose spec introduced this bound - used for span
    /// annotations in error reports.
    pub origin: Ident,
}

/// Extract the ordered, deduplicated list of `ANY_*` bounds from a POU's
/// top-level variable decls. Returns an empty vec for non-generic POUs.
pub fn derive_generic_params<'db>(
    db: &'db dyn WorkspaceDataBase,
    pou: &Pou<'db>,
) -> Vec<GenericParam> {
    let vars: &[VariableDecl<'db>] = match pou {
        Pou::FunctionBlock(fb) => fb.variables(db),
        Pou::Class(c) => c.variables(db),
        _ => return Vec::new(),
    };
    collect_any_bounds(db, vars)
}

/// Same as [`derive_generic_params`], but on a [`FunctionBlock`] directly -
/// avoids wrapping in a [`Pou`] for the common case.
pub fn fb_generic_params<'db>(
    db: &'db dyn WorkspaceDataBase,
    fb: FunctionBlock<'db>,
) -> Vec<GenericParam> {
    collect_any_bounds(db, fb.variables(db))
}

/// Same but for a [`Class`].
pub fn class_generic_params<'db>(
    db: &'db dyn WorkspaceDataBase,
    c: Class<'db>,
) -> Vec<GenericParam> {
    collect_any_bounds(db, c.variables(db))
}

fn collect_any_bounds<'db>(
    db: &'db dyn WorkspaceDataBase,
    vars: &[VariableDecl<'db>],
) -> Vec<GenericParam> {
    let mut out: Vec<GenericParam> = Vec::new();
    for var in vars {
        if let Some(bound) = spec_any_bound(db, var.spec(db))
            && !out.iter().any(|p| p.bound == bound)
        {
            out.push(GenericParam {
                bound,
                origin: var.name(db),
            });
        }
    }
    out
}

/// If a spec resolves to a single `ANY_*` elementary, return its kind.
/// Returns `None` for concrete types or for wrapped specs (phase 2 keeps
/// this narrow - we can widen later to see through `Ref(ANY_*)` etc. if
/// the need arises).
fn spec_any_bound(db: &dyn WorkspaceDataBase, spec: Spec<'_>) -> Option<ElementarySpec> {
    match spec.kind(db) {
        SpecKind::Simple(e) if e.is_any() => Some(*e),
        _ => None,
    }
}
