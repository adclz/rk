use db::WorkspaceDataBase;
use ide_diagnostic::IdeDiagnostic;
use rustc_hash::FxHashMap;

use crate::{
    HirNodeInfo,
    check::errors::{
        ToIdeDiagnostic,
        e1_duplicates::DuplicateError,
        e2_resolve::{ResolveError, UnschedulableReason, UnsupportedConfigKind},
    },
    hir_def::{
        config::{ConfigDecl, ProgConfig, ResourceDecl, TaskConfig},
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

/// The resolved execution model of a CONFIGURATION.
///
/// Every RESOURCE, the tasks it declares that can actually run, and the
/// program instances bound to each — with every value already resolved:
/// intervals in nanoseconds, priorities as numbers, program types as
/// declarations. Consumers walk this; they do not re-derive from it.
///
/// The maps beside it answer questions about a single node (what task is this
/// program bound to?), which is what the IDE asks. This answers the whole
/// question at once — what runs, in what order, under which resource — which
/// is what lowering asks. Building it here is what keeps a RESOURCE from
/// being flattened away by whoever needed a task list.
#[derive(Debug, PartialEq, Eq, salsa::Update, Default)]
pub struct ResolvedSchedule<'db> {
    pub resources: Vec<ResolvedResource<'db>>,
}

#[derive(Debug, PartialEq, Eq, salsa::Update)]
pub struct ResolvedResource<'db> {
    pub decl: ResourceDecl<'db>,
    pub name: Ident,
    /// The `ON <type>` token: names the execution unit this group runs on.
    pub cpu_type: Ident,
    /// Runnable tasks, most urgent first (lowest PRIORITY number); tasks with
    /// no PRIORITY sort last, ties keep declaration order.
    pub tasks: Vec<ResolvedTask<'db>>,
}

#[derive(Debug, PartialEq, Eq, salsa::Update)]
pub struct ResolvedTask<'db> {
    pub decl: TaskConfig<'db>,
    pub name: Ident,
    /// Scan period in nanoseconds. A task that cannot run is absent from this
    /// model entirely — see `unschedulable` for why.
    pub interval_ns: u64,
    pub priority: Option<u32>,
    pub programs: Vec<ResolvedProgram<'db>>,
}

#[derive(Debug, PartialEq, Eq, salsa::Update)]
pub struct ResolvedProgram<'db> {
    pub decl: ProgConfig<'db>,
    pub instance_name: Ident,
    pub program: ProgramDecl<'db>,
    /// Config-level RETAIN/NON_RETAIN qualifier, when written.
    pub retain: Option<bool>,
}

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

    /// What actually runs — see [`ResolvedSchedule`].
    pub schedule: ResolvedSchedule<'db>,

    /// Resolved PRIORITY per TASK. Absent when PRIORITY was omitted (E0035) or
    /// unusable (E0241) — either way consumers get a number or nothing
    pub task_priority: FxHashMap<TaskConfig<'db>, u32>,

    /// Scan period in nanoseconds for each TASK that can actually be scheduled.
    /// A task missing from this map cannot run; consumers skip it without
    /// needing to re-derive why.
    pub task_interval_ns: FxHashMap<TaskConfig<'db>, u64>,

    /// Why each unschedulable TASK cannot run. Held rather than reported on
    /// sight: a task nothing is bound to harms nobody, so only the ones a
    /// PROGRAM actually depends on become diagnostics.
    pub unschedulable: FxHashMap<TaskConfig<'db>, UnschedulableReason>,

    pub errors: Vec<IdeDiagnostic>,
}

/// Returns the resolved config inference result for a given CONFIGURATION.
#[salsa::tracked(returns(ref))]
pub fn infer_config_result<'db>(
    db: &'db dyn WorkspaceDataBase,
    config: ConfigDecl<'db>,
) -> ConfigInferenceResult<'db> {
    let mut result = ConfigInferenceResult {
        schedule: ResolvedSchedule::default(),
        task_priority: FxHashMap::default(),
        task_interval_ns: FxHashMap::default(),
        unschedulable: FxHashMap::default(),
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

    for r in config.resources(db).iter() {
        check_or_insert(&mut seen_resources, r.name(db), |first, second| {
            errors.push(
                DuplicateError::Resource {
                    res1: second,
                    res2: first,
                }
                .to_diagnostic(db, config.get_scope_id(db).file(db)),
            );
        });
        check_resource_duplicates(db, r, errors);
    }

    // Phase 2: resolve each resource's tasks and validate the programs bound
    // to them. Tasks and programs only exist inside a RESOURCE.
    for r in config.resources(db).iter() {
        // Tasks are scoped to the RESOURCE that declares them.
        let resource_tasks: FxHashMap<Ident, TaskConfig<'db>> =
            r.tasks(db).iter().map(|t| (t.name(db).ident, *t)).collect();
        resolve_task_intervals(db, &resource_tasks, result);
        for p in r.programs(db).iter() {
            validate_prog_config(db, p, &resource_tasks, result);
            report_unsupported_conf_elements(db, p, result);
            resolve_prog_instance(db, p, &mut result.prog_instance);
        }
    }

    // Phase 3: assemble what actually runs. Everything above resolved single
    // nodes; this is the whole shape, so lowering never has to rebuild it (and
    // never flattens a RESOURCE away doing so).
    build_resolved_schedule(db, config, result);

    // Phase 2b: a PROGRAM bound to a task that cannot run would never run.
    report_unschedulable_bound_tasks(db, result);

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
                .to_diagnostic(db, r.get_scope_id(db).file(db)),
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
                .to_diagnostic(db, r.get_scope_id(db).file(db)),
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
        && let Some(prog) = program_index(db, target.path.target.ident)
    {
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
    match p.task(db) {
        Some(task_ref) => match known_tasks.get(&task_ref.ident) {
            Some(task) => {
                result.task_of_prog.insert(*p, *task);
            }
            None => {
                result.errors.push(
                    ResolveError::UnknownTaskRef { task: task_ref }
                        .to_diagnostic(db, p.get_scope_id(db).file(db)),
                );
            }
        },
        // No WITH clause at all. The scheduler used to drop the instance in
        // silence, so the program compiled and simply never ran.
        None => {
            result.errors.push(
                ResolveError::ProgramWithoutTask {
                    instance: p.name(db),
                }
                .to_diagnostic(db, p.get_scope_id(db).file(db)),
            );
        }
    }
}

/// Report the parsed-but-inert elements of a `PROGRAM ... (...)` clause.
///
/// `ProgConfig::conf_elements` is written by the builder and read by nobody:
/// both element kinds and all three data-source forms are parsed and then
/// discarded. No name resolution, no type check, no copy emitted — so
/// `(inp := src, outp => snk, ghost := nosuch)` compiled clean while doing
/// nothing at all, unknown names included.
fn report_unsupported_conf_elements<'db>(
    db: &'db dyn WorkspaceDataBase,
    p: &ProgConfig<'db>,
    result: &mut ConfigInferenceResult<'db>,
) {
    use crate::hir_def::config::{ProgCnxn, ProgConfElement};

    for element in p.conf_elements(db) {
        let (expr, kind) = match element {
            ProgConfElement::Connection(ProgCnxn::Source { path, .. })
            | ProgConfElement::Connection(ProgCnxn::Sink { path, .. }) => {
                (*path, UnsupportedConfigKind::ProgramConnection)
            }
            ProgConfElement::FbTask(fb) => {
                (fb.path, UnsupportedConfigKind::FbTaskAssociation)
            }
        };
        result.errors.push(
            ResolveError::UnsupportedConfigElement { expr, kind }
                .to_diagnostic(db, p.get_scope_id(db).file(db)),
        );
    }
}

/// Report each unschedulable TASK that a PROGRAM is actually bound to.
///
/// Reported here rather than at the task's declaration so an unused TASK — one
/// declared for later, or an event task nothing depends on yet — does not fail
/// the build. What must never be silent is a PROGRAM that cannot run.
fn report_unschedulable_bound_tasks<'db>(
    db: &'db dyn WorkspaceDataBase,
    result: &mut ConfigInferenceResult<'db>,
) {
    let mut reported: Vec<TaskConfig<'db>> = Vec::new();
    let bound: Vec<TaskConfig<'db>> = result.task_of_prog.values().copied().collect();
    for task in bound {
        if reported.contains(&task) {
            continue; // one message per task, however many programs bind to it
        }
        let Some(&reason) = result.unschedulable.get(&task) else {
            continue;
        };
        reported.push(task);
        result.errors.push(
            ResolveError::UnschedulableTask {
                task: task.name(db),
                reason,
            }
            .to_diagnostic(db, task.get_scope_id(db).file(db)),
        );
    }
}

/// Resolve each TASK's scan period, recording the ones that cannot be honoured.
///
/// Only a cyclic task with a literal, non-zero INTERVAL is schedulable. Every
/// other shape used to be dropped by a bare `continue` in MIR with no
/// diagnostic anywhere: `rk check` reported nothing, `rk compile` succeeded,
/// and the core came out with no schedule — which the runtime then rejected
/// with an unrelated complaint about the program's body export.
///
/// Resolving it here rather than in MIR also means the answer is published
/// once: the scheduler reads `task_interval_ns` instead of parsing intervals
/// a second time.
/// Assembles [`ResolvedSchedule`] from the per-node resolutions above.
///
/// Order is meaning here: resources and programs keep declaration order, and
/// tasks are sorted most-urgent-first so a consumer dispatching in slice order
/// is correct by construction rather than by remembering to sort.
fn build_resolved_schedule<'db>(
    db: &'db dyn WorkspaceDataBase,
    config: ConfigDecl<'db>,
    result: &mut ConfigInferenceResult<'db>,
) {
    let mut resources = Vec::new();
    for r in config.resources(db).iter() {
        // Group instances under the task they are bound to, keeping the order
        // they were declared in.
        let mut tasks: Vec<ResolvedTask<'db>> = Vec::new();
        for p in r.programs(db).iter() {
            let Some(task) = result.task_of_prog.get(p).copied() else {
                continue; // no resolvable WITH <task> — already diagnosed
            };
            let Some(program) = result.prog_instance.get(&p.name(db).ident).copied() else {
                continue; // program type did not resolve — already diagnosed
            };
            // A task that cannot run contributes nothing to run.
            let Some(&interval_ns) = result.task_interval_ns.get(&task) else {
                continue;
            };
            let resolved = ResolvedProgram {
                decl: *p,
                instance_name: p.name(db).ident,
                program,
                retain: p.retain(db),
            };
            match tasks.iter_mut().find(|t| t.decl == task) {
                Some(t) => t.programs.push(resolved),
                None => tasks.push(ResolvedTask {
                    decl: task,
                    name: task.name(db).ident,
                    interval_ns,
                    priority: result.task_priority.get(&task).copied(),
                    programs: vec![resolved],
                }),
            }
        }
        // Most urgent first; no PRIORITY sorts last; stable, so ties keep
        // declaration order.
        tasks.sort_by_key(|t| t.priority.unwrap_or(u32::MAX));

        if !tasks.is_empty() {
            resources.push(ResolvedResource {
                decl: *r,
                name: r.name(db).ident,
                cpu_type: r.resource_type_name(db),
                tasks,
            });
        }
    }
    result.schedule = ResolvedSchedule { resources };
}

fn resolve_task_intervals<'db>(
    db: &'db dyn WorkspaceDataBase,
    tasks: &FxHashMap<Ident, TaskConfig<'db>>,
    result: &mut ConfigInferenceResult<'db>,
) {
    for task in tasks.values() {
        // PRIORITY is held as source text by the declaration; resolve it here
        // so nothing downstream has to parse, and an unusable value is a
        // diagnostic rather than a silent "no priority".
        if let Some(text) = task.priority(db) {
            match text.text(db).parse::<u32>() {
                Ok(p) => {
                    result.task_priority.insert(*task, p);
                }
                Err(_) => result.errors.push(
                    ResolveError::InvalidPriority {
                        task: task.name(db),
                        value: text,
                    }
                    .to_diagnostic(db, task.get_scope_id(db).file(db)),
                ),
            }
        }

        let reason = match (task.interval(db), task.single(db)) {
            (Some(ds), _) => match interval_nanos(db, &ds) {
                None => Some(UnschedulableReason::NonLiteralInterval),
                Some(0) => Some(UnschedulableReason::ZeroInterval),
                Some(ns) => {
                    result.task_interval_ns.insert(*task, ns);
                    None
                }
            },
            (None, Some(_)) => Some(UnschedulableReason::EventDriven),
            (None, None) => Some(UnschedulableReason::NoTrigger),
        };
        if let Some(reason) = reason {
            result.unschedulable.insert(*task, reason);
        }
    }
}

/// A TASK's INTERVAL in nanoseconds.
///
/// The period is baked into the emitted schedule, so it must be known at
/// compile time — but "known at compile time" is wider than "written as a
/// literal". A `VAR_GLOBAL CONSTANT period : TIME := T#10ms` is just as fixed
/// as `T#10ms`, and rejecting it would be stating a language rule to cover a
/// missing resolution. A directly represented variable (`%MW0`) is the only
/// source that genuinely cannot supply one.
fn interval_nanos<'db>(
    db: &'db dyn WorkspaceDataBase,
    ds: &crate::hir_def::config::DataSource<'db>,
) -> Option<u64> {
    use crate::hir_def::config::DataSource;

    match ds {
        DataSource::Constant(expr) => time_literal_nanos(db, *expr),
        DataSource::Path(path) => {
            let var = crate::hir_ty::index_graphs::external_var_lookup(db, path.ident(db).ident)?;
            // Only a CONSTANT can be trusted: an ordinary VAR_GLOBAL may be
            // written at runtime, and the schedule cannot follow it.
            if !var.qualifier(db).contains(crate::Qualifier::CONSTANT) {
                return None;
            }
            let init = var.init(db)?;
            time_literal_nanos(db, constant_init_expr(db, init)?)
        }
        // `%MW0` and friends: a period read from process memory is not a
        // compile-time fact at all.
        DataSource::Direct(_) => None,
    }
}

/// The single expression behind a scalar initializer, if it is one.
fn constant_init_expr<'db>(
    db: &'db dyn WorkspaceDataBase,
    init: crate::hir_def::expressions::expression::InitExpr<'db>,
) -> Option<crate::hir_def::expressions::expression::Expr<'db>> {
    use crate::hir_def::expressions::expression::InitExprKind;
    match init.kind(db) {
        InitExprKind::ConstantExpr(expr) => Some(expr),
        _ => None,
    }
}

/// A TIME/LTIME literal expression in nanoseconds.
fn time_literal_nanos<'db>(
    db: &'db dyn WorkspaceDataBase,
    expr: crate::hir_def::expressions::expression::Expr<'db>,
) -> Option<u64> {
    use crate::hir_def::expressions::expression::{Elementary, ExprKind, PrimaryExpr};

    let ExprKind::PrimaryExpr(PrimaryExpr::Literal(elem)) = expr.expr(db) else {
        return None;
    };
    let dur = match elem {
        Elementary::Time(id) => id.as_time(db).ok()?,
        Elementary::LTime(id) => id.as_ltime(db).ok()?,
        _ => return None,
    };
    u64::try_from(dur.whole_nanoseconds()).ok()
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
        // The validation below is real and useful — it catches an unknown
        // instance, an unknown field, a type mismatch — but the value is then
        // discarded: nothing writes it into the instance, so the field keeps
        // whatever its own declaration gave it. Working diagnostics on a
        // construct that does nothing is more misleading than a plain
        // rejection, so say so.
        errors.push(
            ResolveError::UnsupportedConfigElement {
                expr: decl.path,
                kind: UnsupportedConfigKind::InstanceInit,
            }
            .to_diagnostic(db, config.get_scope_id(db).file(db)),
        );

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
                    .to_diagnostic(db, config.get_scope_id(db).file(db)),
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
                        .to_diagnostic(db, config.get_scope_id(db).file(db)),
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
                        .to_diagnostic(db, config.get_scope_id(db).file(db)),
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
