use db::WorkspaceDataBase;

use crate::{
    CallSite, Visibility,
    check::errors::{ToIdeDiagnostic, e10_visibility::VisibilityError},
    hir_def::{pous::pou::Pou, scope::ScopeKind, semantic_index::get_scope},
    hir_ty::head::init_inference::InitInference,
};

impl<'db> InitInference<'db> {
    /// E1006: a FUNCTION header may carry PRIVATE (or PUBLIC, the default
    /// made explicit); PROTECTED and INTERNAL mean nothing there.
    pub(crate) fn check_function_specifier(&mut self, db: &'db dyn WorkspaceDataBase) {
        let ScopeKind::Pou(Pou::Function(func)) = get_scope(db, self.scope).kind else {
            return;
        };
        let visibility = func.visibility(db);
        let Some(spec_id) = func.spec_id(db) else {
            return;
        };
        self.errors.push(
            VisibilityError::SpecifierNotOnFunction {
                site: CallSite::new(func.scope_id(db), spec_id),
                keyword: if visibility.contains(Visibility::PROTECTED) {
                    "PROTECTED"
                } else if visibility.contains(Visibility::INTERNAL) {
                    "INTERNAL"
                } else {
                    return;
                },
            }
            .to_diagnostic(db, self.scope.file(db)),
        );
    }
}
