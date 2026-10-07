// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use db::WorkspaceDataBase;

use crate::{
    CallSite, HasName,
    check::errors::{ToIdeDiagnostic, e03_type::TypeError},
    hir_def::{
        expressions::spec::{Spec, SpecKind},
        pous::pou::Pou,
        scope::ScopeKind,
        semantic_index::get_scope,
    },
    hir_ty::{head::init_inference::InitInference, layout},
};

impl<'db> InitInference<'db> {
    /// E0322 on a declaration whose storage passes what a module addresses,
    /// at the innermost part that does: a STRUCT holding an array too large
    /// is reported at the array, and a variable of a TYPE too large at the
    /// TYPE, not again. A STRUCT too large as a whole is reported at `name`,
    /// the declaration's, rather than across its lines.
    pub(crate) fn check_storage(
        &mut self,
        db: &'db dyn WorkspaceDataBase,
        spec: Spec<'db>,
        name: CallSite<'db>,
    ) {
        if let Some((part, size)) = oversized(db, spec) {
            let (what, site) = match part.kind(db) {
                SpecKind::Struct(_) if part == spec => ("this STRUCT", name),
                SpecKind::Struct(_) => ("this STRUCT", CallSite::from_scoped(db, &part)),
                SpecKind::SizedString(_) => ("this STRING", CallSite::from_scoped(db, &part)),
                _ => ("this array", CallSite::from_scoped(db, &part)),
            };
            self.errors.push(
                TypeError::StorageTooLarge {
                    site,
                    what: what.to_string(),
                    size,
                }
                .to_diagnostic(db, self.scope.file(db)),
            );
        }
    }

    /// E0322 on an FB, a CLASS or a PROGRAM whose instance passes it while
    /// each of its members fits: the members together are too large.
    pub(crate) fn check_instance_storage(&mut self, db: &'db dyn WorkspaceDataBase) {
        let (instance, what, site) = match get_scope(db, self.scope).kind {
            ScopeKind::Pou(pou @ (Pou::FunctionBlock(_) | Pou::Class(_))) => (
                layout::instance_layout(db, pou).as_ref(),
                format!("an instance of '{}'", pou.get_name_with_case(db).text(db)),
                CallSite::new(self.scope, pou.get_name_id(db)),
            ),
            ScopeKind::Program(program) => (
                layout::program_layout(db, program).as_ref(),
                format!("the PROGRAM '{}'", program.get_name_with_case(db).text(db)),
                CallSite::new(self.scope, program.get_name_id(db)),
            ),
            _ => return,
        };
        let Some(instance) = instance else {
            return;
        };
        if instance.whole.fits() || instance.fields.iter().any(|field| !field.layout.fits()) {
            return;
        }
        self.errors.push(
            TypeError::StorageTooLarge {
                site,
                what,
                size: instance.whole.size,
            }
            .to_diagnostic(db, self.scope.file(db)),
        );
    }
}

/// The innermost part of `spec` whose storage passes what a module
/// addresses, and its size. A named type is reported where it is declared,
/// so neither it nor what holds it is reported here.
fn oversized<'db>(db: &'db dyn WorkspaceDataBase, spec: Spec<'db>) -> Option<(Spec<'db>, u64)> {
    let parts: Vec<Spec<'db>> = match spec.kind(db) {
        SpecKind::Struct(strukt) => strukt.elements(db).iter().map(|e| e.spec(db)).collect(),
        SpecKind::Array(array) => vec![array.of_type(db)],
        SpecKind::SizedString(_) => Vec::new(),
        _ => return None,
    };
    for part in &parts {
        if let Some(found) = oversized(db, *part) {
            return Some(found);
        }
    }
    let fits = |spec| layout::of_spec(db, spec).is_none_or(layout::Layout::fits);
    if !parts.iter().all(|part| fits(*part)) {
        return None;
    }
    let whole = layout::of_spec(db, spec)?;
    (!whole.fits()).then_some((spec, whole.size))
}
