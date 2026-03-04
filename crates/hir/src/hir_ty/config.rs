use db::WorkspaceDataBase;
use ide_diagnostic::IdeDiagnostic;
use rustc_hash::FxHashMap;

use crate::{
    check::errors::{ToIdeDiagnostic, e1_duplicates::DuplicateError, e2_resolve::ResolveError},
    hir_def::{
        config::{ConfigDecl, ConfigResource, ProgConfig, ResourceDecl, TaskConfig},
        expressions::spec::SpecKind,
        interned::identifier::{Ident, SpanIdent},
        program::ProgramDecl,
    },
    hir_ty::{
        body::BodyInferenceResult, expr_store::PathExprWalkStep,
        head::init_inference::InitExprInferenceResult, index_graphs::program_index, infer::Infer,
        ty::Type,
    },
};

/// Resolved references within a CONFIGURATION declaration.
///
/// Built during `infer_config` and accessible via `infer_config_result` query.
/// Stores resolved mappings for IDE features (go-to-definition, hover).
#[derive(Debug, PartialEq, Eq, salsa::Update)]
pub struct ConfigInferenceResult<'db> {
    /// Maps each ProgConfig to its resolved `WITH <task>` TaskConfig (if valid).
    pub task_of_prog: FxHashMap<ProgConfig<'db>, TaskConfig<'db>>,

    /// Maps each program instance name to the resolved PROGRAM declaration.
    pub prog_instance: FxHashMap<Ident, ProgramDecl<'db>>,

    pub errors: Vec<IdeDiagnostic>,
}

/// Returns the resolved config inference result for a given CONFIGURATION.
#[salsa::tracked(returns(ref))]
pub fn infer_config_result<'db>(
    db: &'db dyn WorkspaceDataBase,
    config: ConfigDecl<'db>,
) -> ConfigInferenceResult<'db> {
    let mut result = ConfigInferenceResult {
        task_of_prog: FxHashMap::default(),
        prog_instance: FxHashMap::default(),
        errors: Vec::new(),
    };
    infer_config(db, config, &mut result);
    result
}

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
fn infer_config<'db>(
    db: &'db dyn WorkspaceDataBase,
    config: ConfigDecl<'db>,
    result: &mut ConfigInferenceResult<'db>,
) {
    let errors = &mut result.errors;

    let mut seen_resources: FxHashMap<Ident, SpanIdent<'db>> = FxHashMap::default();
    // Config-level task map (for validating top-level PROGRAM WITH references).
    let mut config_tasks: FxHashMap<Ident, TaskConfig<'db>> = FxHashMap::default();
    // Config-level program instance map.
    let mut config_progs: FxHashMap<Ident, SpanIdent<'db>> = FxHashMap::default();

    for res in config.resources(db).iter() {
        match res {
            ConfigResource::Task(t) => {
                if let Some(first) = config_tasks.get(&t.name(db).ident) {
                    errors.push(
                        DuplicateError::Task {
                            task1: t.name(db),
                            task2: first.name(db),
                        }
                        .to_diagnostic(db),
                    );
                } else {
                    config_tasks.insert(t.name(db).ident, *t);
                }
            }
            ConfigResource::Program(p) => {
                check_or_insert(&mut config_progs, p.name(db), |first, second| {
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
                check_or_insert(&mut seen_resources, r.name(db), |first, second| {
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

    // Phase 2: validate top-level PROGRAM references, resolve tasks, and build instance map.
    for res in config.resources(db).iter() {
        match res {
            ConfigResource::Program(p) => {
                validate_prog_config(db, p, &config_tasks, result);
                resolve_prog_instance(db, p, &mut result.prog_instance);
            }
            ConfigResource::Resource(r) => {
                // Tasks visible inside a resource are scoped to that resource only.
                let resource_tasks: FxHashMap<Ident, TaskConfig<'db>> =
                    r.tasks(db).iter().map(|t| (t.name(db).ident, *t)).collect();
                for p in r.programs(db).iter() {
                    validate_prog_config(db, p, &resource_tasks, result);
                    resolve_prog_instance(db, p, &mut result.prog_instance);
                }
            }
            ConfigResource::Task(_) => {}
        }
    }

    // Phase 3: validate VAR_CONFIG entries.
    validate_config_inst_inits(db, config, &result.prog_instance, &mut result.errors);
}

/// Checks for duplicate task and program instance names within a RESOURCE block.
fn check_resource_duplicates<'db>(
    db: &'db dyn WorkspaceDataBase,
    r: &ResourceDecl<'db>,
    errors: &mut Vec<IdeDiagnostic>,
) {
    let mut seen_tasks: FxHashMap<Ident, SpanIdent<'db>> = FxHashMap::default();
    for t in r.tasks(db).iter() {
        check_or_insert(&mut seen_tasks, t.name(db), |first, second| {
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
    for p in r.programs(db).iter() {
        check_or_insert(&mut seen_progs, p.name(db), |first, second| {
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

/// Resolves a ProgConfig's prog_type to a ProgramDecl and inserts into the instance map.
fn resolve_prog_instance<'db>(
    db: &'db dyn WorkspaceDataBase,
    p: &ProgConfig<'db>,
    instances: &mut FxHashMap<Ident, ProgramDecl<'db>>,
) {
    if let SpecKind::Target(target) = p.prog_type(db).kind(db)
        && target.path.namespace.is_none()
            && let Some(prog) = program_index(db, target.path.target.ident) {
                instances.insert(p.name(db).ident, prog);
            }
}

fn validate_prog_config<'db>(
    db: &'db dyn WorkspaceDataBase,
    p: &ProgConfig<'db>,
    known_tasks: &FxHashMap<Ident, TaskConfig<'db>>,
    result: &mut ConfigInferenceResult<'db>,
) {
    // Program type resolution is now handled by infer_config_resources in signature inference.
    // Unknown program types are reported as E0210 (NoNamespaceItemFound) by infer_spec.

    // Resolve the WITH <task> reference if present.
    if let Some(task_ref) = p.task(db) {
        match known_tasks.get(&task_ref.ident) {
            Some(task) => {
                result.task_of_prog.insert(*p, *task);
            }
            None => {
                result
                    .errors
                    .push(ResolveError::UnknownTaskRef { task: task_ref }.to_diagnostic(db));
            }
        }
    }
}

/// Validates VAR_CONFIG entries: resolves each path against program instances
/// and type-checks the init expression against the resolved variable type.
fn validate_config_inst_inits<'db>(
    db: &'db dyn WorkspaceDataBase,
    config: ConfigDecl<'db>,
    instances: &FxHashMap<Ident, ProgramDecl<'db>>,
    errors: &mut Vec<IdeDiagnostic>,
) {
    let config_inits = config.config_init(db);
    if config_inits.is_empty() {
        return;
    }

    // Walk each VAR_CONFIG path and validate init expressions.
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
