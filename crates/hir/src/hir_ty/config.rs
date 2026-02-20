use db::WorkspaceDataBase;
use ide_diagnostic::IdeDiagnostic;
use rustc_hash::FxHashMap;

use crate::{
    check::errors::{
        analysis_error::ToIdeDiagnostic,
        e1_duplicates::DuplicateError,
        e2_resolve::ResolveError,
    },
    hir_def::{
        config::{ConfigDecl, ConfigResource, ProgConfig, ResourceDecl},
        interned::identifier::{Ident, SpanIdent},
    },
    hir_ty::name_res::program_index,
};

/// Validates a single CONFIGURATION declaration:
///
/// **Phase 1 — Duplicate detection** (scoped per config / per resource):
/// - Duplicate RESOURCE names within the config (E0116)
/// - Duplicate TASK names at config level / within each resource (E0114)
/// - Duplicate PROGRAM instance names at config level / within each resource (E0115)
///
/// **Phase 2 — Reference validation**:
/// - Every `PROGRAM ... : <ProgType>` must reference a known PROGRAM declaration (E0218)
/// - Every `PROGRAM ... WITH <task>` must reference a TASK declared in the same scope (E0219)
pub fn infer_config<'db>(
    db: &'db dyn WorkspaceDataBase,
    config: ConfigDecl<'db>,
    errors: &mut Vec<IdeDiagnostic>,
) {
    let mut seen_resources: FxHashMap<Ident, SpanIdent<'db>> = FxHashMap::default();
    // Config-level task map (for validating top-level PROGRAM WITH references).
    let mut config_tasks: FxHashMap<Ident, SpanIdent<'db>> = FxHashMap::default();
    // Config-level program instance map.
    let mut config_progs: FxHashMap<Ident, SpanIdent<'db>> = FxHashMap::default();

    for res in config.resources(db).iter() {
        match res {
            ConfigResource::Task(t) => {
                check_or_insert(&mut config_tasks, t.name, |first, second| {
                    errors.push(
                        DuplicateError::Task { task1: second, task2: first }.to_diagnostic(db),
                    );
                });
            }
            ConfigResource::Program(p) => {
                check_or_insert(&mut config_progs, p.name, |first, second| {
                    errors.push(
                        DuplicateError::ProgInstance { prog1: second, prog2: first }
                            .to_diagnostic(db),
                    );
                });
            }
            ConfigResource::Resource(r) => {
                check_or_insert(&mut seen_resources, r.name, |first, second| {
                    errors.push(
                        DuplicateError::Resource { res1: second, res2: first }.to_diagnostic(db),
                    );
                });
                check_resource_duplicates(db, r, errors);
            }
        }
    }

    // Phase 2: validate top-level PROGRAM references.
    for res in config.resources(db).iter() {
        match res {
            ConfigResource::Program(p) => {
                validate_prog_config(db, p, &config_tasks, errors);
            }
            ConfigResource::Resource(r) => {
                // Tasks visible inside a resource are scoped to that resource only.
                let resource_tasks: FxHashMap<Ident, SpanIdent<'db>> =
                    r.tasks.iter().map(|t| (t.name.ident, t.name)).collect();
                for p in r.programs.iter() {
                    validate_prog_config(db, p, &resource_tasks, errors);
                }
            }
            ConfigResource::Task(_) => {}
        }
    }
}

/// Checks for duplicate task and program instance names within a RESOURCE block.
fn check_resource_duplicates<'db>(
    db: &'db dyn WorkspaceDataBase,
    r: &ResourceDecl<'db>,
    errors: &mut Vec<IdeDiagnostic>,
) {
    let mut seen_tasks: FxHashMap<Ident, SpanIdent<'db>> = FxHashMap::default();
    for t in r.tasks.iter() {
        check_or_insert(&mut seen_tasks, t.name, |first, second| {
            errors.push(
                DuplicateError::Task { task1: second, task2: first }.to_diagnostic(db),
            );
        });
    }

    let mut seen_progs: FxHashMap<Ident, SpanIdent<'db>> = FxHashMap::default();
    for p in r.programs.iter() {
        check_or_insert(&mut seen_progs, p.name, |first, second| {
            errors.push(
                DuplicateError::ProgInstance { prog1: second, prog2: first }
                    .to_diagnostic(db),
            );
        });
    }
}

/// Inserts `name` into `seen`.  If a previous entry exists, calls `on_duplicate(first, second)`
/// where `first` is the previously-seen entry and `second` is the new duplicate.
fn check_or_insert<'db>(
    seen: &mut FxHashMap<Ident, SpanIdent<'db>>,
    name: SpanIdent<'db>,
    mut on_duplicate: impl FnMut(SpanIdent<'db>, SpanIdent<'db>),
) {
    if let Some(first) = seen.get(&name.ident) {
        on_duplicate(*first, name);
    } else {
        seen.insert(name.ident, name);
    }
}

fn validate_prog_config<'db>(
    db: &'db dyn WorkspaceDataBase,
    p: &ProgConfig<'db>,
    known_tasks: &FxHashMap<Ident, SpanIdent<'db>>,
    errors: &mut Vec<IdeDiagnostic>,
) {
    // Only validate simple (non-namespace-qualified) program types.
    if p.prog_type.path.namespace.is_none() {
        let name = p.prog_type.path.target.ident;
        if program_index(db, name).is_none() {
            errors.push(
                ResolveError::UnknownProgType {
                    prog_type: p.prog_type.clone(),
                }
                .to_diagnostic(db),
            );
        }
    }

    // Validate the WITH <task> reference if present.
    if let Some(task_ref) = &p.task {
        if !known_tasks.contains_key(&task_ref.ident) {
            errors.push(ResolveError::UnknownTaskRef { task: *task_ref }.to_diagnostic(db));
        }
    }
}
