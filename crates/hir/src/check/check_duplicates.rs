use db::WorkspaceDataBase;
use ide_diagnostic::IdeDiagnostic;

use crate::{
    HasName, HirNodeInfo,
    check::errors::{ToIdeDiagnostic, e1_duplicates::DuplicateError},
    hir_def::{config::ConfigDecl, namespace::NamespaceDecl, pous::pou::Pou, program::ProgramDecl},
    hir_ty::index_graphs::{
        config_index, namespace_pou_candidates, pou_candidates, program_index,
    },
};

/// Whether two same-named POUs collide (a real duplicate) rather than form a
/// legal FUNCTION overload set.
///
/// FUNCTIONs may share a name as long as they differ by the overload
/// discriminant (currently the parameter count — see [`Function::param_count`]).
/// Every other combination — two same-named FBs/classes/interfaces/data-types,
/// or a FUNCTION colliding with a non-FUNCTION — is always a duplicate, since
/// only FUNCTIONs participate in overloading.
fn pous_collide<'db>(db: &'db dyn WorkspaceDataBase, a: Pou<'db>, b: Pou<'db>) -> bool {
    match (a, b) {
        (Pou::Function(fa), Pou::Function(fb)) => fa.param_count(db) == fb.param_count(db),
        _ => true,
    }
}

/// Check for duplicate global POU names.
///
/// `pou` is a duplicate when an EARLIER same-name candidate collides with it
/// (see [`pous_collide`]). The first occurrence in discovery order is the
/// canonical one and is never flagged; a later colliding declaration is.
pub fn check_duplicate_pous<'db>(
    db: &'db dyn WorkspaceDataBase,
    pou: Pou<'db>,
    errors: &mut Vec<IdeDiagnostic>,
) {
    for other in pou_candidates(db, pou.get_name_ident(db)) {
        // Reached `pou` itself before any collision → it is the canonical decl.
        if other == pou {
            return;
        }
        if pous_collide(db, pou, other) {
            errors.push(
                DuplicateError::Pou {
                    pou1: pou,
                    pou2: other,
                }
                .to_diagnostic(db, pou.get_scope_id(db).file(db)),
            );
            return;
        }
    }
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
            .to_diagnostic(db, program.get_scope_id(db).file(db)),
        )
    };
}

/// Check for duplicate CONFIGURATION names.
/// A config is a duplicate if it differs from the one in the workspace index.
pub fn check_duplicate_configs<'db>(
    db: &'db dyn WorkspaceDataBase,
    config: ConfigDecl<'db>,
    errors: &mut Vec<IdeDiagnostic>,
) {
    let indexed = match config_index(db, config.get_name_ident(db)) {
        Some(indexed) => indexed,
        None => return,
    };

    if config != indexed {
        errors.push(
            DuplicateError::Config {
                config1: config,
                config2: indexed,
            }
            .to_diagnostic(db, config.get_scope_id(db).file(db)),
        )
    };
}

/// Check for duplicate POU names within a namespace. Same overload-aware rule as
/// [`check_duplicate_pous`], scoped to the namespace's candidate set.
pub fn check_duplicate_namespaces<'db>(
    db: &'db dyn WorkspaceDataBase,
    namespace: NamespaceDecl<'db>,
    errors: &mut Vec<IdeDiagnostic>,
) {
    for pou in namespace.pous(db).iter() {
        for other in namespace_pou_candidates(db, *namespace.path(db), pou.get_name_ident(db)) {
            if other == *pou {
                break;
            }
            if pous_collide(db, *pou, other) {
                errors.push(
                    DuplicateError::Pou {
                        pou1: *pou,
                        pou2: other,
                    }
                    .to_diagnostic(db, pou.get_scope_id(db).file(db)),
                );
                break;
            }
        }
    }
}
