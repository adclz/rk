use db::WorkspaceDataBase;
use ide_diagnostic::IdeDiagnostic;

use crate::{
    HasName,
    check::errors::{analysis_error::ToIdeDiagnostic, e1_duplicates::DuplicateError},
    hir_def::{namespace::NamespaceDecl, pous::pou::Pou, program::ProgramDecl},
    hir_ty::name_res::{namespace_pou_index, pou_index, program_index},
};

/// Check for duplicate global POU names.
/// A POU is a duplicate if it differs from the one in the index.
pub fn check_duplicate_pous<'db>(
    db: &'db dyn WorkspaceDataBase,
    pou: Pou<'db>,
    errors: &mut Vec<IdeDiagnostic>,
) {
    let indexed = match pou_index(db, pou.get_name_ident(db)) {
        Some(indexed) => indexed,
        // If a POU is not in the index, we can't say it's a duplicate
        None => return,
    };

    // If this POU is not the indexed one, it's a duplicate
    if pou != indexed {
        errors.push(
            DuplicateError::Pou {
                pou1: pou,
                pou2: indexed,
            }
            .to_diagnostic(db),
        )
    };
}

/// Check for duplicate global POU names.
/// A POU is a duplicate if it differs from the one in the index.
pub fn check_duplicate_programs<'db>(
    db: &'db dyn WorkspaceDataBase,
    program: ProgramDecl<'db>,
    errors: &mut Vec<IdeDiagnostic>,
) {
    let indexed = match program_index(db, program.get_name_ident(db)) {
        Some(indexed) => indexed,
        // If a POU is not in the index, we can't say it's a duplicate
        None => return,
    };

    // If this POU is not the indexed one, it's a duplicate
    if program != indexed {
        errors.push(
            DuplicateError::Program {
                prog1: program,
                prog2: indexed,
            }
            .to_diagnostic(db),
        )
    };
}

/// Check for duplicate POU names within a namespace.
/// A POU is a duplicate if it differs from the one in the namespace index.
pub fn check_duplicate_namespaces<'db>(
    db: &'db dyn WorkspaceDataBase,
    namespace: NamespaceDecl<'db>,
    errors: &mut Vec<IdeDiagnostic>,
) {
    for pou in namespace.pous(db).iter() {
        match namespace_pou_index(db, *namespace.path(db), pou.get_name_ident(db)) {
            Some(indexed) => {
                // If this POU is not the indexed one, it's a duplicate
                if *pou != indexed {
                    errors.push(
                        DuplicateError::Pou {
                            pou1: *pou,
                            pou2: indexed,
                        }
                        .to_diagnostic(db),
                    );
                }
            }
            // If a POU is not in the index, we can't say it's a duplicate
            _ => (),
        }
    }
}
