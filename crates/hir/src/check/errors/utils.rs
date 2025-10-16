use auto_lsp::default::db::BaseDatabase;
use ide_diagnostic::{IdeDiagnostic, Related};

use crate::{HirNodeInfo, check::recovery::pou::FuzzyResult, hir_ty::ty::Ty};

pub fn get_def_for_ty(db: &dyn BaseDatabase, ty: Ty<'_>, diag: &mut IdeDiagnostic) {
    
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
