use auto_lsp::default::db::BaseDatabase;
use ide_diagnostic::{IdeDiagnostic, Related};

use crate::hir_ty::ty::Ty;

pub fn get_decl_and_def_for_ty(db: &dyn BaseDatabase, ty: Ty<'_>, diag: &mut IdeDiagnostic) {
    get_decl_for_ty(db, ty, diag);

    if let Some(span) = ty.def(db).get_span(db) {
        diag.with_related(Related::new(
            "type defined here".to_string(),
            ty.def(db)
                .get_scope_id(db)
                .expect("A ty definition with a span always has a scope id")
                .file(db),
            span,
        ));
    }
}

pub fn get_decl_for_ty(db: &dyn BaseDatabase, ty: Ty<'_>, diag: &mut IdeDiagnostic) {
    diag.with_related(Related::new(
        format!("'{}' is declared here", ty.decl(db).name(db).text(db),),
        ty.decl(db).scope_id(db).file(db),
        ty.decl(db).name_span(db),
    ));
}
