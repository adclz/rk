use db::WorkspaceDataBase;
use ide_diagnostic::IdeDiagnostic;
use rustc_hash::FxHashMap;

use crate::{
    check::errors::{
        ToIdeDiagnostic, e1_duplicates::DuplicateError, e2_resolve::ResolveError,
    },
    hir_def::{
        config::{ConfigDecl, ConfigResource, ProgConfig, ResourceDecl},
        interned::identifier::{Ident, SpanIdent},
        program::ProgramDecl,
    },
    hir_ty::{
        body::BodyInferenceResult, expr_store::PathExprWalkStep,
        head::init_inference::InitExprInferenceResult, infer::Infer, index_graphs::program_index,
        ty::Type,
    },
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
///
/// **Phase 3 — VAR_CONFIG validation**:
/// - Each `VAR_CONFIG` path is resolved against program instances (E0222, E0223)
/// - Init expressions are type-checked against the resolved variable type
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
                        DuplicateError::Task {
                            task1: second,
                            task2: first,
                        }
                        .to_diagnostic(db),
                    );
                });
            }
            ConfigResource::Program(p) => {
                check_or_insert(&mut config_progs, p.name, |first, second| {
                    errors.push(
                        DuplicateError::ProgInstance {
                            prog1: second,
                            prog2: first,
                        }
                        .to_diagnostic(db),
                    );
                });
            }
            ConfigResource::Resource(r) => {
                check_or_insert(&mut seen_resources, r.name, |first, second| {
                    errors.push(
                        DuplicateError::Resource {
                            res1: second,
                            res2: first,
                        }
                        .to_diagnostic(db),
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

    // Phase 3: validate VAR_CONFIG entries.
    validate_config_inst_inits(db, config, errors);
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
                DuplicateError::Task {
                    task1: second,
                    task2: first,
                }
                .to_diagnostic(db),
            );
        });
    }

    let mut seen_progs: FxHashMap<Ident, SpanIdent<'db>> = FxHashMap::default();
    for p in r.programs.iter() {
        check_or_insert(&mut seen_progs, p.name, |first, second| {
            errors.push(
                DuplicateError::ProgInstance {
                    prog1: second,
                    prog2: first,
                }
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
    if let Some(task_ref) = &p.task
        && !known_tasks.contains_key(&task_ref.ident)
    {
        errors.push(ResolveError::UnknownTaskRef { task: *task_ref }.to_diagnostic(db));
    }
}

/// Validates VAR_CONFIG entries: resolves each path against program instances
/// and type-checks the init expression against the resolved variable type.
fn validate_config_inst_inits<'db>(
    db: &'db dyn WorkspaceDataBase,
    config: ConfigDecl<'db>,
    errors: &mut Vec<IdeDiagnostic>,
) {
    let config_inits = config.config_init(db);
    if config_inits.is_empty() {
        return;
    }

    // Step A: Build instance map (instance name → ProgramDecl).
    let mut instances: FxHashMap<Ident, ProgramDecl<'db>> = FxHashMap::default();
    for res in config.resources(db).iter() {
        match res {
            ConfigResource::Program(p) => {
                if p.prog_type.path.namespace.is_none()
                    && let Some(prog) = program_index(db, p.prog_type.path.target.ident)
                {
                    instances.insert(p.name.ident, prog);
                }
            }
            ConfigResource::Resource(r) => {
                for p in r.programs.iter() {
                    if p.prog_type.path.namespace.is_none()
                        && let Some(prog) = program_index(db, p.prog_type.path.target.ident)
                    {
                        instances.insert(p.name.ident, prog);
                    }
                }
            }
            ConfigResource::Task(_) => {}
        }
    }

    // Step B & C: Walk each VAR_CONFIG path and validate init expressions.
    for decl in config_inits {
        let steps = decl.path.flatten(db);

        if steps.is_empty() {
            continue;
        }

        // First step: look up program instance.
        let first_ident = match &steps[0] {
            PathExprWalkStep::Field { ident, .. } => *ident,
            _ => continue,
        };

        let prog = match instances.get(&first_ident.ident) {
            Some(prog) => *prog,
            None => {
                errors.push(
                    ResolveError::ConfigInstInitUnknownInstance {
                        instance_name: first_ident,
                    }
                    .to_diagnostic(db),
                );
                continue;
            }
        };

        // Walk remaining steps through the type hierarchy.
        let mut current_type = Type::Program(prog);

        let mut resolved = true;
        for step in &steps[1..] {
            let field_ident = match step {
                PathExprWalkStep::Field { ident, .. } => *ident,
                _ => {
                    resolved = false;
                    break;
                }
            };

            let scope = match type_to_scope(db, &current_type) {
                Some(scope) => scope,
                None => {
                    errors.push(
                        ResolveError::ConfigInstInitFieldNotFound {
                            field: field_ident,
                            parent_type: current_type,
                        }
                        .to_diagnostic(db),
                    );
                    resolved = false;
                    break;
                }
            };

            let def_map = scope.def_map(db);
            match def_map.global_variables.get(&field_ident.ident) {
                Some(var) => {
                    current_type = var.spec(db).infer(db);
                }
                None => {
                    errors.push(
                        ResolveError::ConfigInstInitFieldNotFound {
                            field: field_ident,
                            parent_type: current_type,
                        }
                        .to_diagnostic(db),
                    );
                    resolved = false;
                    break;
                }
            }
        }

        // Step C: validate init expression against the resolved type.
        if resolved && steps.len() > 1 {
            let mut init_result = InitExprInferenceResult::new(config.scope_id(db));
            let mut body_ctx = BodyInferenceResult::new(config.scope_id(db));
            init_result.resolve_init_expr(db, decl.init, &mut body_ctx, current_type);
            errors.extend(init_result.errors);
            errors.extend(body_ctx.errors);
        }
    }
}

/// Extracts the scope from a composite type (Program, FunctionBlock, Class).
fn type_to_scope<'db>(
    db: &'db dyn WorkspaceDataBase,
    ty: &Type<'db>,
) -> Option<crate::hir_def::scope::ScopeId<'db>> {
    match ty {
        Type::Program(p) => Some(p.scope_id(db)),
        Type::FunctionBlock(fb) => Some(fb.scope_id(db)),
        Type::Class(c) => Some(c.scope_id(db)),
        _ => None,
    }
}
