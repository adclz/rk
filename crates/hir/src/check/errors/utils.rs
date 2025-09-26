use auto_lsp::default::db::BaseDatabase;
use ide_diagnostic::{IdeDiagnostic, Related};

use crate::{check::recovery::pou::FuzzyResult, hir_ty::ty::Ty};

pub fn get_decl_and_def_for_ty(db: &dyn BaseDatabase, ty: Ty<'_>, diag: &mut IdeDiagnostic) {
    get_decl_for_ty(db, ty, diag);
    get_def_for_ty(db, ty, diag);
}

pub fn get_def_for_ty(db: &dyn BaseDatabase, ty: Ty<'_>, diag: &mut IdeDiagnostic) {
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

pub fn get_candidates(candidates: &FuzzyResult) -> String {
    let mut result = String::new();

    if !candidates.variables.is_empty() {
        let mut note = "local variable(s) with similar(s) name exist:\n".to_string();
        let display_count = candidates.variables.len().min(5);

        for (i, candidate) in candidates.variables.iter().take(display_count).enumerate() {
            if i > 0 {
                note.push('\n');
            }
            note.push_str(&format!("- {}", candidate.name));
        }

        if candidates.variables.len() > 5 {
            note.push_str("\n  ...");
        }

        result.push_str(&note);
    };

    if !candidates.struct_fields.is_empty() {
        let mut note = "STRUCT field(s) with similar name(s) exist:\n".to_string();
        let display_count = candidates.struct_fields.len().min(5);

        for (i, candidate) in candidates
            .struct_fields
            .iter()
            .take(display_count)
            .enumerate()
        {
            if i > 0 {
                note.push('\n');
            }
            note.push_str(&format!("- {}", candidate.name));
        }

        if candidates.struct_fields.len() > 5 {
            note.push_str("\n  ...");
        }

        result.push_str(&note);
    };

    if !candidates.pou.is_empty() {
        let mut note = "POU(s) with similar name exist:\n".to_string();
        let display_count = candidates.pou.len().min(5);

        for (i, candidate) in candidates.pou.iter().take(display_count).enumerate() {
            if i > 0 {
                note.push('\n');
            }
            note.push_str(&format!("- {}", candidate.name));
        }

        if candidates.pou.len() > 5 {
            note.push_str("\n  ...");
        }

        result.push_str(&note);
    };
    result
}
