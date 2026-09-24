use db::WorkspaceDataBase;

use crate::{
    CallSite, Visibility,
    check::errors::{ToIdeDiagnostic, e10_visibility::VisibilityError},
    hir_def::{pous::pou::Pou, scope::ScopeKind, semantic_index::get_scope},
    hir_ty::{head::init_inference::InitInference, infer::Infer, ty::Type},
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

    /// What a FUNCTION or METHOD returns is made afresh by each call, so it
    /// cannot hold a variable declared `AT %I*`, whose pointer nothing would
    /// bind (E1425).
    pub(crate) fn check_return_type(&mut self, db: &'db dyn WorkspaceDataBase) {
        use crate::check::errors::e14_config::{ConfigError, PartlyUnlocated};
        let callable = match get_scope(db, self.scope).kind {
            ScopeKind::Pou(Pou::Function(_)) => "FUNCTION",
            ScopeKind::MethodDecl(_) | ScopeKind::MethodProt(_) => "METHOD",
            _ => return,
        };
        let Some(ret) = self.scope.return_type(db) else {
            return;
        };
        let returned = ret.infer(db);
        let mut element = returned.normalize(db);
        while let Type::Array(array) = element {
            element = array.of_type(db).infer(db).normalize(db);
        }
        let held = match crate::hir_ty::head::inheritance::pou_of_type(db, element) {
            Some(pou) => crate::hir_ty::head::inheritance::partly_located_members(db, pou)
                .first()
                .and_then(|path| {
                    Some((
                        path.iter().map(|m| m.name(db)).collect::<Vec<_>>(),
                        *path.last()?,
                    ))
                }),
            None => crate::hir_ty::head::inheritance::partly_located_in_struct(
                db,
                element,
                &mut Vec::new(),
            ),
        };
        let Some((names, member)) = held else {
            return;
        };
        let member_path = names
            .iter()
            .map(|n| n.text(db).to_string())
            .collect::<Vec<_>>()
            .join(".");
        self.errors.push(
            ConfigError::PartlyLocatedUnlocated(PartlyUnlocated::Returned {
                ret: *ret,
                callable,
                ty: compact_str::CompactString::from(returned.type_name(db)),
                member: compact_str::CompactString::from(member_path),
                address: compact_str::CompactString::from(
                    member
                        .location(db)
                        .map(|dv| dv.to_address(db))
                        .unwrap_or_default(),
                ),
            })
            .to_diagnostic(db, self.scope.file(db)),
        );
    }
}
