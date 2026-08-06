use db::WorkspaceDataBase;
use ide_diagnostic::IdeDiagnostic;

use crate::{
    HasName, HirNodeInfo,
    check::errors::{ToIdeDiagnostic, e1_duplicates::DuplicateError, e2_resolve::ResolveError},
    hir_def::{config::ConfigDecl, namespace::NamespaceDecl, pous::pou::Pou, program::ProgramDecl},
    hir_ty::{
        head::signature::function_signature,
        index_graphs::{config_index, namespace_pou_candidates, pou_candidates, program_index},
    },
};

/// Whether two same-named POUs collide (a real duplicate) rather than form a
/// legal FUNCTION overload set.
///
/// FUNCTIONs may share a name as long as they differ by their overload
/// signature — the ordered list of parameter types (see [`function_signature`]).
/// Equal signatures are a duplicate; any difference is a legal overload. Every
/// other combination — two same-named FBs/classes/interfaces/data-types, or a
/// FUNCTION colliding with a non-FUNCTION — is always a duplicate, since only
/// FUNCTIONs participate in overloading.
fn pous_collide<'db>(db: &'db dyn WorkspaceDataBase, a: Pou<'db>, b: Pou<'db>) -> bool {
    match (a, b) {
        (Pou::Function(fa), Pou::Function(fb)) => {
            function_signature(db, fa) == function_signature(db, fb)
        }
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
/// A workspace declares one CONFIGURATION.
///
/// Not an arbitrary limit: a POU is a type, usable by any configuration, so
/// with two of them "which globals are in scope in this POU" has no answer —
/// the same reason a RESOURCE holds no variables. One workspace describes one
/// PLC; a second PLC is a second workspace.
///
/// Counts DISTINCT names, since two configurations sharing a name are a
/// duplicate ([`check_duplicate_configs`]) and saying so twice describes one
/// mistake as two.
///
/// Reported at every configuration rather than at "the extras": the file maps
/// have no order, so there is no first, and picking one would make the message
/// depend on which file happened to be walked first.
pub fn check_single_configuration<'db>(
    db: &'db dyn WorkspaceDataBase,
    config: ConfigDecl<'db>,
    errors: &mut Vec<IdeDiagnostic>,
) {
    let all = crate::hir_ty::index_graphs::declared_configs(db);
    let names: rustc_hash::FxHashSet<_> = all.iter().map(|c| c.get_name_ident(db)).collect();
    if names.len() > 1 {
        // Carry the others so they can be reached from here: deciding which to
        // keep means looking at all of them.
        let mut others: Vec<_> = all
            .iter()
            .filter(|c| c.get_name_ident(db) != config.get_name_ident(db))
            .copied()
            .collect();
        // The file maps have no order, so sort for a stable list.
        others.sort_by_key(|c| c.get_name_ident(db).text(db).to_string());
        errors.push(
            ResolveError::MultipleConfigurations { config, others }
                .to_diagnostic(db, config.get_scope_id(db).file(db)),
        );
    }
}

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
