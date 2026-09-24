use crate::hir_def::pous::variable::{LocatedAddress, VariableDecl};
use db::WorkspaceDataBase;
use ide_diagnostic::IdeDiagnostic;
use rustc_hash::FxHashMap;

use crate::check::errors::e14_config::ConfigError;
use crate::check::errors::e14_config::UnschedulableReason;
use crate::{
    HasName, HirNodeInfo,
    check::errors::{ToIdeDiagnostic, e01_duplicates::DuplicateError},
    hir_def::{
        config::{ConfigDecl, ProgConfig, ResourceDecl, TaskConfig},
        expressions::{expression::PathExpr, spec::SpecKind},
        interned::identifier::{CaselessIdent, Ident, SpanIdent},
        program::ProgramDecl,
    },
    hir_ty::{expr_store::PathExprWalkStep, index_graphs::program_index, infer::Infer, ty::Type},
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
    /// Function blocks this task runs instead of the programs holding them.
    pub function_blocks: Vec<ResolvedTaskFb<'db>>,
}

/// A VAR_CONFIG entry that locates a variable declared `AT %I*`, `%Q*` or
/// `%M*`: which instance's variable, and at what address.
#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub struct ConfigLocation<'db> {
    /// The PROGRAM instance the path starts at.
    pub instance: Ident,
    /// The members walked from the instance, the located one last: `[fb, x]`
    /// for `Res.P1.fb.x`.
    pub members: Vec<VariableDecl<'db>>,
    /// The address it is given, complete.
    pub address: LocatedAddress,
}

#[derive(Debug, PartialEq, Eq, salsa::Update)]
pub struct ResolvedProgram<'db> {
    pub decl: ProgConfig<'db>,
    pub instance_name: Ident,
    pub program: ProgramDecl<'db>,
    /// Config-level RETAIN/NON_RETAIN qualifier, when written.
    pub retain: Option<bool>,
    /// The instance's connections: each input fed before its body runs, and
    /// each output copied out after.
    pub connections: ResolvedProgElements<'db>,
}

/// A function block a task runs instead of the program that holds it.
#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub struct ResolvedTaskFb<'db> {
    /// The PROGRAM instance holding it.
    pub instance_name: Ident,
    /// The program's member it is.
    pub member: VariableDecl<'db>,
}

/// An element list of a program configuration, resolved: `PROGRAM P1 WITH T
/// : F(x1 := src, y1 => snk, fb1 WITH T2)`.
#[derive(Debug, Clone, Default, PartialEq, Eq, salsa::Update)]
pub struct ResolvedProgElements<'db> {
    /// Each input with its source.
    pub inputs: Vec<(VariableDecl<'db>, ConnectionEnd<'db>)>,
    /// Each output with its sink, which is never a constant.
    pub outputs: Vec<(VariableDecl<'db>, ConnectionEnd<'db>)>,
    /// Each function block a task runs, with that task and the element.
    pub function_blocks: Vec<(VariableDecl<'db>, TaskConfig<'db>, PathExpr<'db>)>,
}

/// What a connection's other end is.
#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum ConnectionEnd<'db> {
    /// A constant, typed with the configuration's initializers.
    Constant(crate::hir_def::expressions::expression::Expr<'db>),
    /// A VAR_GLOBAL.
    Global(VariableDecl<'db>),
    /// An address.
    Address(LocatedAddress),
}

/// Every program configuration's element list in a CONFIGURATION fragment.
#[derive(Debug, PartialEq, Eq, salsa::Update)]
pub struct ProgElements<'db> {
    pub per_program: Vec<(ProgConfig<'db>, ResolvedProgElements<'db>)>,
    /// The variable each name in an element list resolved to: a variable of
    /// the program, or a VAR_GLOBAL a connection names. Kept for an element
    /// refused for another reason too, which still names it.
    pub names: FxHashMap<PathExpr<'db>, VariableDecl<'db>>,
    /// The task each `fb WITH task` names, by the element's path.
    pub tasks: FxHashMap<PathExpr<'db>, TaskConfig<'db>>,
    pub errors: Vec<IdeDiagnostic>,
}

/// A VAR_CONFIG entry that gives a variable its starting value in one
/// instance: which instance's variable, the value, and the channel it goes to
/// when the variable is one VAR_CONFIG locates.
#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub struct ConfigValue<'db> {
    /// The PROGRAM instance the path starts at.
    pub instance: Ident,
    /// The members walked from the instance, the valued one last.
    pub members: Vec<VariableDecl<'db>>,
    /// Resolved with the configuration's initializers.
    pub init: crate::hir_def::expressions::expression::InitExpr<'db>,
    /// The address the variable is located at, for one declared `AT %I*`,
    /// `%Q*` or `%M*`: the value is its channel's.
    pub channel: Option<LocatedAddress>,
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
    pub prog_instance: FxHashMap<CaselessIdent, ProgramDecl<'db>>,

    /// What actually runs — see [`ResolvedSchedule`].
    pub schedule: ResolvedSchedule<'db>,

    /// Resolved PRIORITY per TASK. Absent when PRIORITY was omitted (E1405) or
    /// unusable (E1406) — either way consumers get a number or nothing
    pub task_priority: FxHashMap<TaskConfig<'db>, u32>,

    /// Scan period in nanoseconds for each TASK that can actually be scheduled.
    /// A task missing from this map cannot run; consumers skip it without
    /// needing to re-derive why.
    pub task_interval_ns: FxHashMap<TaskConfig<'db>, u64>,

    /// Why each unschedulable TASK cannot run. Held rather than reported on
    /// sight: a task nothing is bound to harms nobody, so only the ones a
    /// PROGRAM actually depends on become diagnostics.
    pub unschedulable: FxHashMap<TaskConfig<'db>, UnschedulableReason>,

    /// The VAR_CONFIG entries that locate a variable, each checked (E1424):
    /// what `__init` binds each instance's variable to.
    pub locations: Vec<ConfigLocation<'db>>,

    /// The VAR_CONFIG entries that give a variable a value, each checked:
    /// what `__init` starts each instance's variable at.
    pub values: Vec<ConfigValue<'db>>,

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
        locations: Vec::new(),
        values: Vec::new(),
        errors: Vec::new(),
    };
    infer_config(db, config, &mut result);
    result
}

/// Validates a single CONFIGURATION declaration:
///
/// **Phase 1 — Duplicate detection** (scoped per config / per resource):
/// - Duplicate RESOURCE names within the config (E0115)
/// - Duplicate TASK names at config level / within each resource (E0114)
/// - Duplicate PROGRAM instance names at config level / within each resource (E0113)
///
/// **Phase 2 — Reference validation**:
/// - Every `PROGRAM ... : <ProgType>` must reference a known PROGRAM declaration (E0203)
/// - Every `PROGRAM ... WITH <task>` must reference a TASK declared in the same scope (E1411)
///
/// **Phase 3 — VAR_CONFIG validation**:
/// - Each `VAR_CONFIG` path is resolved against program instances (E1413, E1414)
/// - Init expressions are type-checked against the resolved variable type
fn infer_config<'db>(
    db: &'db dyn WorkspaceDataBase,
    config: ConfigDecl<'db>,
    result: &mut ConfigInferenceResult<'db>,
) {
    let errors = &mut result.errors;

    let mut seen_resources: FxHashMap<CaselessIdent, SpanIdent<'db>> = FxHashMap::default();

    for r in config.resources(db).iter() {
        check_or_insert(db, &mut seen_resources, r.name(db), |first, second| {
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
        let resource_tasks: FxHashMap<CaselessIdent, TaskConfig<'db>> = r
            .tasks(db)
            .iter()
            .map(|t| (t.name(db).ident.caseless(db), *t))
            .collect();
        resolve_task_intervals(db, &resource_tasks, result);
        for p in r.programs(db).iter() {
            validate_prog_config(db, p, &resource_tasks, result);
            resolve_prog_instance(db, p, &mut result.prog_instance);
        }
    }

    // Phase 2c: the program configurations' element lists.
    result
        .errors
        .extend(prog_elements(db, config).errors.iter().cloned());
    report_called_by_program(db, config, result);

    // Phase 3: assemble what actually runs. Everything above resolved single
    // nodes; this is the whole shape, so lowering never has to rebuild it (and
    // never flattens a RESOURCE away doing so).
    build_resolved_schedule(db, config, result);

    // Phase 2b: a PROGRAM or function block bound to a task that cannot run
    // would never run.
    report_unschedulable_bound_tasks(db, config, result);

    // Phase 3: validate VAR_CONFIG entries.
    check_config_entries(
        db,
        config,
        &mut result.locations,
        &mut result.values,
        &mut result.errors,
    );

    // Phase 4: every instance's variable declared `AT %I*` is located.
    check_partly_located_coverage(db, config, result);
}

/// Checks for duplicate task and program instance names within a RESOURCE block.
fn check_resource_duplicates<'db>(
    db: &'db dyn WorkspaceDataBase,
    r: &ResourceDecl<'db>,
    errors: &mut Vec<IdeDiagnostic>,
) {
    let mut seen_tasks: FxHashMap<CaselessIdent, SpanIdent<'db>> = FxHashMap::default();
    for t in r.tasks(db).iter() {
        check_or_insert(db, &mut seen_tasks, t.name(db), |first, second| {
            errors.push(
                DuplicateError::Task {
                    task1: second,
                    task2: first,
                }
                .to_diagnostic(db, r.get_scope_id(db).file(db)),
            );
        });
    }

    let mut seen_progs: FxHashMap<CaselessIdent, SpanIdent<'db>> = FxHashMap::default();
    for p in r.programs(db).iter() {
        check_or_insert(db, &mut seen_progs, p.name(db), |first, second| {
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
    db: &'db dyn WorkspaceDataBase,
    seen: &mut FxHashMap<CaselessIdent, SpanIdent<'db>>,
    name: SpanIdent<'db>,
    mut on_duplicate: impl FnMut(SpanIdent<'db>, SpanIdent<'db>),
) {
    let key = name.ident.caseless(db);
    if let Some(first) = seen.get(&key) {
        on_duplicate(*first, name);
    } else {
        seen.insert(key, name);
    }
}

/// Resolves a ProgConfig's prog_type to a ProgramDecl and inserts into the instance map.
fn resolve_prog_instance<'db>(
    db: &'db dyn WorkspaceDataBase,
    p: &ProgConfig<'db>,
    instances: &mut FxHashMap<CaselessIdent, ProgramDecl<'db>>,
) {
    if let SpecKind::Target(target) = p.prog_type(db).kind(db)
        && target.path.namespace.is_none()
        && let Some(prog) = program_index(db, target.path.target.ident)
    {
        instances.insert(p.name(db).ident.caseless(db), prog);
    }
}

fn validate_prog_config<'db>(
    db: &'db dyn WorkspaceDataBase,
    p: &ProgConfig<'db>,
    known_tasks: &FxHashMap<CaselessIdent, TaskConfig<'db>>,
    result: &mut ConfigInferenceResult<'db>,
) {
    // Program type resolution is now handled by infer_config_resources in signature inference.
    // Unknown program types are reported as E0203 (NoNamespaceItemFound) by infer_spec.

    // Resolve the WITH <task> reference if present.
    match p.task(db) {
        Some(task_ref) => match known_tasks.get(&task_ref.ident.caseless(db)) {
            Some(task) => {
                result.task_of_prog.insert(*p, *task);
            }
            None => {
                result.errors.push(
                    ConfigError::UnknownTaskRef { task: task_ref }
                        .to_diagnostic(db, p.get_scope_id(db).file(db)),
                );
            }
        },
        // No WITH clause at all. The scheduler used to drop the instance in
        // silence, so the program compiled and simply never ran.
        None => {
            result.errors.push(
                ConfigError::ProgramWithoutTask {
                    instance: p.name(db),
                }
                .to_diagnostic(db, p.get_scope_id(db).file(db)),
            );
        }
    }
}

/// Report each function block a task runs that its program's body calls too:
/// it would run twice, once under each task.
fn report_called_by_program<'db>(
    db: &'db dyn WorkspaceDataBase,
    config: ConfigDecl<'db>,
    result: &mut ConfigInferenceResult<'db>,
) {
    use crate::check::errors::e14_config::ProgElementRefusal;
    for (p, elements) in &prog_elements(db, config).per_program {
        if elements.function_blocks.is_empty() {
            continue;
        }
        let Some(program) = result
            .prog_instance
            .get(&p.name(db).ident.caseless(db))
            .copied()
        else {
            continue;
        };
        let body = crate::hir_ty::body::infer_body(db, program.scope_id(db));
        for (member, _, path) in &elements.function_blocks {
            // The first call the body makes to it.
            let call = body
                .resolved_calls
                .keys()
                .filter_map(|call| call.path(db).expr(db))
                .filter(|callee| body.variable_of_path_expr.get(callee) == Some(member))
                .min_by_key(|callee| callee.get_span(db).start_byte);
            if let Some(call) = call {
                result.errors.push(
                    ConfigError::ProgElementRefused {
                        expr: *path,
                        why: ProgElementRefusal::CalledByProgram {
                            var: member.name(db).text(db).clone(),
                            program: program.name(db).text(db).clone(),
                            call: crate::CallSite::from_scoped(db, &call),
                        },
                    }
                    .to_diagnostic(db, p.get_scope_id(db).file(db)),
                );
            }
        }
    }
}

/// Report each unschedulable TASK that a PROGRAM or a function block is
/// actually bound to.
///
/// Reported here rather than at the task's declaration so an unused TASK — one
/// declared for later, or an event task nothing depends on yet — does not fail
/// the build. What must never be silent is a PROGRAM that cannot run.
fn report_unschedulable_bound_tasks<'db>(
    db: &'db dyn WorkspaceDataBase,
    config: ConfigDecl<'db>,
    result: &mut ConfigInferenceResult<'db>,
) {
    let mut reported: Vec<TaskConfig<'db>> = Vec::new();
    // A function block a task runs is bound to it too.
    let function_blocks = prog_elements(db, config)
        .per_program
        .iter()
        .flat_map(|(_, e)| e.function_blocks.iter().map(|(_, task, _)| *task));
    let bound: Vec<TaskConfig<'db>> = result
        .task_of_prog
        .values()
        .copied()
        .chain(function_blocks)
        .collect();
    for task in bound {
        if reported.contains(&task) {
            continue; // one message per task, however many programs bind to it
        }
        let Some(&reason) = result.unschedulable.get(&task) else {
            continue;
        };
        reported.push(task);
        result.errors.push(
            ConfigError::UnschedulableTask {
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
    let elements = prog_elements(db, config);
    let mut resources = Vec::new();
    for r in config.resources(db).iter() {
        // Group instances under the task they are bound to, keeping the order
        // they were declared in.
        let mut tasks: Vec<ResolvedTask<'db>> = Vec::new();
        // The task's entry, made the first time something runs under it.
        let task_entry =
            |tasks: &mut Vec<ResolvedTask<'db>>, task: TaskConfig<'db>, interval_ns: u64| {
                tasks
                    .iter()
                    .position(|t| t.decl == task)
                    .unwrap_or_else(|| {
                        tasks.push(ResolvedTask {
                            decl: task,
                            name: task.name(db).ident,
                            interval_ns,
                            priority: result.task_priority.get(&task).copied(),
                            programs: Vec::new(),
                            function_blocks: Vec::new(),
                        });
                        tasks.len() - 1
                    })
            };
        for p in r.programs(db).iter() {
            let Some(task) = result.task_of_prog.get(p).copied() else {
                continue; // no resolvable WITH <task> — already diagnosed
            };
            let Some(program) = result
                .prog_instance
                .get(&p.name(db).ident.caseless(db))
                .copied()
            else {
                continue; // program type did not resolve — already diagnosed
            };
            // A task that cannot run contributes nothing to run.
            let Some(&interval_ns) = result.task_interval_ns.get(&task) else {
                continue;
            };
            let connections = elements
                .per_program
                .iter()
                .find(|(q, _)| q == p)
                .map(|(_, e)| e.clone())
                .unwrap_or_default();
            // Each function block it holds that a task of its own runs.
            let function_blocks: Vec<_> = connections
                .function_blocks
                .iter()
                .filter_map(|(member, fb_task, _)| {
                    let &interval_ns = result.task_interval_ns.get(fb_task)?;
                    Some((*member, *fb_task, interval_ns))
                })
                .collect();
            let resolved = ResolvedProgram {
                decl: *p,
                instance_name: p.name(db).ident,
                program,
                retain: p.retain(db),
                connections,
            };
            let at = task_entry(&mut tasks, task, interval_ns);
            tasks[at].programs.push(resolved);
            for (member, fb_task, interval_ns) in function_blocks {
                let at = task_entry(&mut tasks, fb_task, interval_ns);
                tasks[at].function_blocks.push(ResolvedTaskFb {
                    instance_name: p.name(db).ident,
                    member,
                });
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
    tasks: &FxHashMap<CaselessIdent, TaskConfig<'db>>,
    result: &mut ConfigInferenceResult<'db>,
) {
    for task in tasks.values() {
        // PRIORITY is held as source text by the declaration; resolve it here
        // so nothing downstream has to parse, and an unusable value is a
        // diagnostic rather than a silent "no priority".
        if let Some(text) = task.priority(db) {
            // `1_0` is ten: IEC allows digit separators in integer literals, and
            // the grammar's unsigned_int accepts them, so they must be stripped
            // before parsing exactly as every other integer literal does.
            match text.text(db).replace('_', "").parse::<u32>() {
                Ok(p) => {
                    result.task_priority.insert(*task, p);
                }
                Err(_) => result.errors.push(
                    ConfigError::InvalidPriority {
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
            // Only a CONSTANT can be trusted: an ordinary VAR_GLOBAL may be
            // written at runtime, and the schedule cannot follow it. Which
            // declaration is fixed, and where its value lives, is the same
            // question a CASE label asks — so it is asked in one place.
            let var = crate::hir_ty::index_graphs::external_var_lookup(db, path.ident(db).ident)?;
            let init = crate::hir_ty::infer::const_eval::constant_init(db, var)?;
            time_literal_nanos(db, init)
        }
        // `%MW0` and friends: a period read from process memory is not a
        // compile-time fact at all.
        DataSource::Direct(_) => None,
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

/// Where a VAR_CONFIG location entry stands after the checks that do not
/// look at the workspace's other addresses.
#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum EntryLocation {
    /// A complete address in the variable's area, as wide as its type.
    Given(LocatedAddress),
    Refused(crate::check::errors::e14_config::ConfigLocationRefusal),
}

/// A VAR_CONFIG entry whose path resolves.
#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub struct ConfigEntry<'db> {
    pub path: crate::hir_def::expressions::expression::PathExpr<'db>,
    /// The PROGRAM instance the path starts at.
    pub instance: Ident,
    /// The members walked from the instance, the named variable last; empty
    /// for an entry naming the instance itself.
    pub members: Vec<VariableDecl<'db>>,
    /// The `AT` it gives, with how it stands.
    pub location: Option<(
        crate::hir_def::pous::variable::DirectVariable<'db>,
        EntryLocation,
    )>,
    pub init: Option<crate::hir_def::expressions::expression::InitExpr<'db>>,
}

/// A fragment's VAR_CONFIG entries that resolve, and what is wrong with the
/// others and with what they write (E1413, E1414, E1426).
#[derive(Debug, PartialEq, Eq, salsa::Update)]
pub struct ConfigEntries<'db> {
    pub entries: Vec<ConfigEntry<'db>>,
    /// What the first steps of each path name: the resource, then the
    /// program instance.
    pub steps: FxHashMap<PathExpr<'db>, ConfigPathStep<'db>>,
    pub errors: Vec<IdeDiagnostic>,
}

/// A step of a VAR_CONFIG path that names no variable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, salsa::Update)]
pub enum ConfigPathStep<'db> {
    Resource(ResourceDecl<'db>),
    Instance(ProgConfig<'db>),
}

/// Each VAR_CONFIG entry of `config`, resolved against the instances every
/// fragment of its configuration declares, with the location checks that
/// look at nothing but the entry and its variable.
///
/// A query of its own, apart from [`infer_config_result`], so that what
/// needs every fragment's entries (coverage, duplicates) and what needs every
/// entry's address (the views) read it without a cycle. Nothing here infers
/// an expression, which could reach the views.
#[salsa::tracked(returns(ref))]
pub fn resolve_config_entries<'db>(
    db: &'db dyn WorkspaceDataBase,
    config: ConfigDecl<'db>,
) -> ConfigEntries<'db> {
    use crate::check::errors::e14_config::ConfigEntryRefusal;
    let mut out = ConfigEntries {
        entries: Vec::new(),
        steps: FxHashMap::default(),
        errors: Vec::new(),
    };
    let decls = config.config_init(db);
    if decls.is_empty() {
        return out;
    }
    let file = config.get_scope_id(db).file(db);
    // Instances and resources of every fragment: a VAR_CONFIG may name an
    // instance another block of the same configuration declares.
    let fragments = crate::hir_ty::index_graphs::config_fragments(db, config.get_name_ident(db));
    let mut instances = FxHashMap::default();
    for fragment in &fragments {
        for r in fragment.resources(db).iter() {
            for p in r.programs(db).iter() {
                resolve_prog_instance(db, p, &mut instances);
            }
        }
    }
    let resource_named = |ident: &SpanIdent<'db>| {
        let name = ident.ident.caseless(db);
        fragments
            .iter()
            .flat_map(|f| f.resources(db).iter())
            .find(|r| r.name(db).ident.caseless(db) == name)
            .copied()
    };
    let instance_named = |ident: &SpanIdent<'db>| {
        let name = ident.ident.caseless(db);
        fragments
            .iter()
            .flat_map(|f| f.resources(db).iter())
            .flat_map(|r| r.programs(db).iter())
            .find(|p| p.name(db).ident.caseless(db) == name)
            .copied()
    };

    for decl in config.config_init(db) {
        let steps = decl.path.flatten(db);
        if steps.is_empty() {
            continue;
        }
        // The standard writes the path RESOURCE.PROGRAM.VARIABLE; a leading
        // segment naming one of the configuration's resources is skipped.
        let instance_at = match &steps[0] {
            PathExprWalkStep::Field { ident, expr } if steps.len() > 1 => {
                match resource_named(ident) {
                    Some(resource) => {
                        out.steps.insert(*expr, ConfigPathStep::Resource(resource));
                        1
                    }
                    None => 0,
                }
            }
            _ => 0,
        };
        let first_ident = match &steps[instance_at] {
            PathExprWalkStep::Field { ident, expr } => {
                if let Some(instance) = instance_named(ident) {
                    out.steps.insert(*expr, ConfigPathStep::Instance(instance));
                }
                *ident
            }
            _ => {
                out.errors.push(
                    ConfigError::ConfigEntryRefused {
                        expr: decl.path,
                        why: ConfigEntryRefusal::PathStep,
                    }
                    .to_diagnostic(db, file),
                );
                continue;
            }
        };
        let Some(prog) = instances.get(&first_ident.ident.caseless(db)).copied() else {
            out.errors.push(
                ConfigError::ConfigInstInitUnknownInstance {
                    instance_name: first_ident,
                }
                .to_diagnostic(db, file),
            );
            continue;
        };

        // Each step names a member of the instance reached so far, inherited
        // ones included: `instance_members` is what an instance holds.
        let mut current_type = Type::Program(prog);
        let mut members: Vec<VariableDecl<'db>> = Vec::new();
        let mut resolved = true;
        for step in &steps[instance_at + 1..] {
            let PathExprWalkStep::Field { ident, .. } = step else {
                out.errors.push(
                    ConfigError::ConfigEntryRefused {
                        expr: decl.path,
                        why: ConfigEntryRefusal::PathStep,
                    }
                    .to_diagnostic(db, file),
                );
                resolved = false;
                break;
            };
            match instance_member(db, current_type, ident.ident) {
                Some(var) => {
                    current_type = var.spec(db).infer(db).normalize(db);
                    members.push(var);
                }
                None => {
                    out.errors.push(
                        ConfigError::ConfigInstInitFieldNotFound {
                            field: *ident,
                            parent_type: current_type,
                        }
                        .to_diagnostic(db, file),
                    );
                    resolved = false;
                    break;
                }
            }
        }
        if !resolved {
            continue;
        }

        // The entry repeats the variable's type; one that says another is
        // wrong about the variable, whatever else it gives it.
        if let (Some(var), Some(written)) = (members.last(), decl.spec) {
            let written = Type::resolve_spec(db, written);
            let declared = Type::resolve_spec(db, var.spec(db));
            if !written.is_never()
                && !declared.is_never()
                && !crate::hir_ty::head::checks::variables::same_storage_type(db, written, declared)
            {
                out.errors.push(
                    ConfigError::ConfigEntryRefused {
                        expr: decl.path,
                        why: ConfigEntryRefusal::TypeMismatch {
                            var: var.name(db).text(db).clone(),
                            written: compact_str::CompactString::from(written.type_name(db)),
                            declared: compact_str::CompactString::from(declared.type_name(db)),
                        },
                    }
                    .to_diagnostic(db, file),
                );
            }
        }

        let location = match (decl.located_at, members.last()) {
            (Some(dv), Some(var)) => Some((
                dv,
                match check_config_location(db, *var, dv) {
                    Ok(address) => EntryLocation::Given(address),
                    Err(why) => EntryLocation::Refused(why),
                },
            )),
            _ => None,
        };
        out.entries.push(ConfigEntry {
            path: decl.path,
            instance: first_ident.ident,
            members,
            location,
            init: decl.init,
        });
    }
    out
}

/// The member `name` among [`members_of`] `ty`.
fn instance_member<'db>(
    db: &'db dyn WorkspaceDataBase,
    ty: Type<'db>,
    name: Ident,
) -> Option<VariableDecl<'db>> {
    let fold = name.caseless(db);
    members_of(db, ty)
        .into_iter()
        .find(|v| v.name(db).caseless(db) == fold)
}

/// Every member an instance of `ty` holds: a PROGRAM's instance state, or
/// an FB's or CLASS's [`instance_members`], inherited ones included.
///
/// [`instance_members`]: crate::hir_ty::head::inheritance::instance_members
pub fn members_of<'db>(db: &'db dyn WorkspaceDataBase, ty: Type<'db>) -> Vec<VariableDecl<'db>> {
    match ty {
        Type::Program(p) => p
            .variables(db)
            .iter()
            .copied()
            .filter(|v| !v.is_temp(db) && !v.is_external(db))
            .collect(),
        _ => match crate::hir_ty::head::inheritance::pou_of_type(db, ty) {
            Some(pou) => crate::hir_ty::head::inheritance::instance_members(db, pou)
                .iter()
                .map(|m| m.var)
                .collect(),
            None => Vec::new(),
        },
    }
}

/// What the first steps of a VAR_CONFIG path reach, for the IDE to complete
/// the next one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigPathPrefix<'db> {
    /// Nothing yet: a resource or a program instance comes next.
    Start,
    /// A resource: one of its program instances comes next.
    Resource(ResourceDecl<'db>),
    /// A program instance, or a member of one: what an instance of `ty`
    /// holds comes next. `member` is the last step when it is a member.
    Holder {
        ty: Type<'db>,
        member: Option<VariableDecl<'db>>,
    },
}

/// Resolve the steps of a VAR_CONFIG path written so far in `config`, as
/// [`resolve_config_entries`] resolves a whole one: an optional resource, a
/// program instance, then members. `None` when a step names nothing.
pub fn config_path_prefix<'db>(
    db: &'db dyn WorkspaceDataBase,
    config: ConfigDecl<'db>,
    steps: &[&str],
) -> Option<ConfigPathPrefix<'db>> {
    let steps: Vec<Ident> = steps
        .iter()
        .map(|step| Ident::new(db, compact_str::CompactString::from(*step)))
        .collect();
    let fragments = crate::hir_ty::index_graphs::config_fragments(db, config.get_name_ident(db));
    let resources: Vec<ResourceDecl<'db>> = fragments
        .iter()
        .flat_map(|f| f.resources(db).iter().copied())
        .collect();
    let mut steps = steps.iter();
    let Some(first) = steps.next() else {
        return Some(ConfigPathPrefix::Start);
    };
    let resource = resources
        .iter()
        .find(|r| r.name(db).ident.caseless(db) == first.caseless(db))
        .copied();
    let instance = match resource {
        Some(r) => match steps.next() {
            None => return Some(ConfigPathPrefix::Resource(r)),
            Some(name) => r
                .programs(db)
                .iter()
                .find(|p| p.name(db).ident.caseless(db) == name.caseless(db))
                .copied(),
        },
        None => resources
            .iter()
            .flat_map(|r| r.programs(db).iter())
            .find(|p| p.name(db).ident.caseless(db) == first.caseless(db))
            .copied(),
    }?;
    let mut ty = instance.prog_type(db).infer(db);
    let mut member = None;
    for name in steps {
        let var = instance_member(db, ty, *name)?;
        ty = var.spec(db).infer(db).normalize(db);
        member = Some(var);
    }
    Some(ConfigPathPrefix::Holder { ty, member })
}

/// Each program configuration's element list in `config`, resolved against
/// the program it instantiates and the tasks of its resource: `PROGRAM P1
/// WITH T : F(x1 := %IX1.1, y1 => total, fb1 WITH T2)`.
///
/// A query of its own, apart from [`infer_config_result`], so the
/// configuration's initialization types the constant sources without
/// reading the whole configuration. Nothing here infers an expression.
#[salsa::tracked(returns(ref))]
pub fn prog_elements<'db>(
    db: &'db dyn WorkspaceDataBase,
    config: ConfigDecl<'db>,
) -> ProgElements<'db> {
    use crate::check::errors::e14_config::ProgElementRefusal;
    use crate::hir_def::config::{ProgCnxn, ProgConfElement};
    let file = config.get_scope_id(db).file(db);
    let mut out = ProgElements {
        per_program: Vec::new(),
        names: FxHashMap::default(),
        tasks: FxHashMap::default(),
        errors: Vec::new(),
    };
    for r in config.resources(db).iter() {
        for p in r.programs(db).iter() {
            if p.conf_elements(db).is_empty() {
                continue;
            }
            let mut instance = FxHashMap::default();
            resolve_prog_instance(db, p, &mut instance);
            let Some(program) = instance.into_values().next() else {
                continue; // the program type did not resolve: already diagnosed
            };
            let mut resolved = ResolvedProgElements::default();
            // The inputs and function blocks each element named, so one named
            // twice is reported at both.
            let mut named: Vec<(VariableDecl<'db>, PathExpr<'db>, bool)> = Vec::new();
            for element in p.conf_elements(db) {
                let path = match element {
                    ProgConfElement::Connection(
                        ProgCnxn::Source { path, .. } | ProgCnxn::Sink { path, .. },
                    ) => *path,
                    ProgConfElement::FbTask(fb) => fb.path,
                };
                let Some(var) = program_member(db, program, path, &mut out.errors) else {
                    continue;
                };
                out.names.insert(path, var);
                let name = var.name(db).text(db).clone();
                let refusal = match element {
                    ProgConfElement::Connection(ProgCnxn::Source { source, .. }) => {
                        if !var.is_input(db) {
                            Some(ProgElementRefusal::NotAnInput { var: name })
                        } else {
                            match check_source(db, var, source, path, &mut out.names) {
                                Ok(end) => {
                                    named.push((var, path, false));
                                    resolved.inputs.push((var, end));
                                    None
                                }
                                Err(diagnostic) => {
                                    out.errors.push(diagnostic);
                                    None
                                }
                            }
                        }
                    }
                    ProgConfElement::Connection(ProgCnxn::Sink { sink, .. }) => {
                        if !var.is_output(db) {
                            Some(ProgElementRefusal::NotAnOutput { var: name })
                        } else {
                            match check_sink(db, var, sink, path, &mut out.names) {
                                Ok(end) => {
                                    resolved.outputs.push((var, end));
                                    None
                                }
                                Err(diagnostic) => {
                                    out.errors.push(diagnostic);
                                    None
                                }
                            }
                        }
                    }
                    ProgConfElement::FbTask(fb) => {
                        let task = r
                            .tasks(db)
                            .iter()
                            .find(|t| t.name(db).ident.caseless(db) == fb.task.ident.caseless(db));
                        if let Some(task) = task {
                            out.tasks.insert(path, *task);
                        }
                        if !matches!(var.spec(db).infer(db).normalize(db), Type::FunctionBlock(_)) {
                            Some(ProgElementRefusal::NotAFunctionBlock { var: name })
                        } else if let Some(task) = task {
                            named.push((var, path, true));
                            resolved.function_blocks.push((var, *task, path));
                            None
                        } else {
                            Some(ProgElementRefusal::UnknownTask {
                                task: fb.task.ident.text(db).clone(),
                            })
                        }
                    }
                };
                if let Some(why) = refusal {
                    out.errors.push(
                        ConfigError::ProgElementRefused { expr: path, why }.to_diagnostic(db, file),
                    );
                }
            }
            // An input has one source, and a function block one task.
            let twice = |var: &VariableDecl<'db>, fb: bool| {
                named
                    .iter()
                    .filter(|(v, _, f)| v == var && *f == fb)
                    .count()
                    > 1
            };
            for (var, path, fb) in &named {
                if twice(var, *fb) {
                    let var = var.name(db).text(db).clone();
                    let why = if *fb {
                        ProgElementRefusal::AssociatedTwice { var }
                    } else {
                        ProgElementRefusal::ConnectedTwice { var }
                    };
                    out.errors.push(
                        ConfigError::ProgElementRefused { expr: *path, why }
                            .to_diagnostic(db, file),
                    );
                }
            }
            resolved.inputs.retain(|(var, _)| !twice(var, false));
            resolved
                .function_blocks
                .retain(|(var, ..)| !twice(var, true));
            out.per_program.push((*p, resolved));
        }
    }
    out
}

/// What each path a configuration writes names, recorded as a body records
/// its own: a variable of the program an element names, a VAR_GLOBAL a
/// connection names, and each member a VAR_CONFIG path goes through. What
/// the IDE features and the linter read.
pub(crate) fn record_config_paths<'db>(
    db: &'db dyn WorkspaceDataBase,
    config: ConfigDecl<'db>,
    result: &mut crate::hir_ty::body::BodyInferenceResult<'db>,
) {
    let mut named: Vec<(PathExpr<'db>, VariableDecl<'db>)> = prog_elements(db, config)
        .names
        .iter()
        .map(|(path, var)| (*path, *var))
        .collect();
    for entry in &resolve_config_entries(db, config).entries {
        // The members are the path's last steps, one each.
        let steps: Vec<_> = entry
            .path
            .flatten(db)
            .iter()
            .filter_map(|step| match step {
                PathExprWalkStep::Field { expr, .. } => Some(*expr),
                _ => None,
            })
            .collect();
        named.extend(
            steps
                .into_iter()
                .rev()
                .zip(entry.members.iter().rev().copied()),
        );
    }
    // A task's INTERVAL or SINGLE may name a VAR_GLOBAL.
    for task in config.resources(db).iter().flat_map(|r| r.tasks(db).iter()) {
        for source in [task.interval(db), task.single(db)].into_iter().flatten() {
            if let crate::hir_def::config::DataSource::Path(path) = source
                && let Some(global) =
                    crate::hir_ty::index_graphs::external_var_lookup(db, path.ident(db).ident)
            {
                named.push((path, global));
            }
        }
    }
    for (path, var) in named {
        result
            .type_of_path_expr
            .insert(path, Type::Variable((var, None)));
        result.variable_of_path_expr.insert(path, var);
        result.variables_used.insert(var);
    }
}

/// The variable of `program` an element names, by its name alone.
fn program_member<'db>(
    db: &'db dyn WorkspaceDataBase,
    program: ProgramDecl<'db>,
    path: PathExpr<'db>,
    errors: &mut Vec<IdeDiagnostic>,
) -> Option<VariableDecl<'db>> {
    use crate::check::errors::e14_config::ProgElementRefusal;
    let file = path.get_scope_id(db).file(db);
    let [PathExprWalkStep::Field { ident, .. }] = path.flatten(db).as_slice() else {
        errors.push(
            ConfigError::ProgElementRefused {
                expr: path,
                why: ProgElementRefusal::NotAVariable,
            }
            .to_diagnostic(db, file),
        );
        return None;
    };
    let var = instance_member(db, Type::Program(program), ident.ident);
    if var.is_none() {
        errors.push(
            ConfigError::ConfigInstInitFieldNotFound {
                field: *ident,
                parent_type: Type::Program(program),
            }
            .to_diagnostic(db, file),
        );
    }
    var
}

/// The VAR_GLOBAL a source or sink names, by its name alone.
fn connected_global<'db>(
    db: &'db dyn WorkspaceDataBase,
    global: PathExpr<'db>,
    names: &mut FxHashMap<PathExpr<'db>, VariableDecl<'db>>,
) -> Result<VariableDecl<'db>, crate::check::errors::e14_config::ProgElementRefusal<'db>> {
    use crate::check::errors::e14_config::ProgElementRefusal;
    let [PathExprWalkStep::Field { ident, .. }] = global.flatten(db).as_slice() else {
        return Err(ProgElementRefusal::NotAVariable);
    };
    let var =
        crate::hir_ty::index_graphs::external_var_lookup(db, ident.ident).ok_or_else(|| {
            ProgElementRefusal::NoSuchGlobal {
                name: ident.ident.text(db).clone(),
            }
        })?;
    names.insert(global, var);
    Ok(var)
}

/// Whether `var` and what it is connected to hold one type: a VAR_GLOBAL of
/// its type, or an address as wide as it is.
fn check_connected<'db>(
    db: &'db dyn WorkspaceDataBase,
    var: VariableDecl<'db>,
    end: &ConnectionEnd<'db>,
) -> Result<(), crate::check::errors::e14_config::ProgElementRefusal<'db>> {
    use crate::check::errors::e14_config::ProgElementRefusal;
    let declared = var.spec(db).infer(db);
    if declared.is_never() {
        return Ok(()); // already diagnosed
    }
    let name = var.name(db).text(db).clone();
    let declared_name = compact_str::CompactString::from(declared.type_name(db));
    match end {
        ConnectionEnd::Constant(_) => Ok(()),
        ConnectionEnd::Global(global) => {
            let ty = global.spec(db).infer(db);
            if ty.is_never() || global_connects(db, var, *global) {
                return Ok(());
            }
            Err(ProgElementRefusal::TypeMismatch {
                var: name,
                declared: declared_name,
                global: global.name(db).text(db).clone(),
                ty: compact_str::CompactString::from(ty.type_name(db)),
            })
        }
        ConnectionEnd::Address(address) => {
            let width =
                crate::hir_ty::head::checks::variables::located_width(declared.normalize(db));
            if width == Some(address.width as usize) {
                return Ok(());
            }
            Err(ProgElementRefusal::WidthMismatch {
                var: name,
                declared: declared_name,
                address: address.text.clone(),
                bits: address.width,
            })
        }
    }
}

/// Whether a connection may join `var` and the VAR_GLOBAL `global`: they
/// hold one type.
pub fn global_connects<'db>(
    db: &'db dyn WorkspaceDataBase,
    var: VariableDecl<'db>,
    global: VariableDecl<'db>,
) -> bool {
    crate::hir_ty::head::checks::variables::same_storage_type(
        db,
        var.spec(db).infer(db),
        global.spec(db).infer(db),
    )
}

/// An address a connection names, or why it names none (E1417).
fn connected_address<'db>(
    db: &'db dyn WorkspaceDataBase,
    dv: crate::hir_def::pous::variable::DirectVariable<'db>,
    path: PathExpr<'db>,
) -> Result<LocatedAddress, IdeDiagnostic> {
    use crate::check::errors::e14_config::UnlocatableAddress;
    LocatedAddress::of(db, dv).ok_or_else(|| {
        ConfigError::DirectVariableUnsupported {
            site: crate::CallSite::from_scoped(db, &path),
            address: dv.to_address(db).into(),
            why: if dv.partly(db) {
                UnlocatableAddress::Incomplete
            } else {
                UnlocatableAddress::Malformed
            },
        }
        .to_diagnostic(db, path.get_scope_id(db).file(db))
    })
}

/// A source feeding the input `var`. A constant is typed with the
/// configuration's initializers.
fn check_source<'db>(
    db: &'db dyn WorkspaceDataBase,
    var: VariableDecl<'db>,
    source: &crate::hir_def::config::DataSource<'db>,
    path: PathExpr<'db>,
    names: &mut FxHashMap<PathExpr<'db>, VariableDecl<'db>>,
) -> Result<ConnectionEnd<'db>, IdeDiagnostic> {
    use crate::hir_def::config::DataSource;
    let refused = |why| {
        ConfigError::ProgElementRefused { expr: path, why }
            .to_diagnostic(db, path.get_scope_id(db).file(db))
    };
    let end = match source {
        DataSource::Constant(expr) => ConnectionEnd::Constant(*expr),
        DataSource::Path(global) => {
            ConnectionEnd::Global(connected_global(db, *global, names).map_err(refused)?)
        }
        DataSource::Direct(dv) => ConnectionEnd::Address(connected_address(db, *dv, path)?),
    };
    check_connected(db, var, &end).map_err(refused)?;
    Ok(end)
}

/// A sink the output `var` is copied to, which must take a write.
fn check_sink<'db>(
    db: &'db dyn WorkspaceDataBase,
    var: VariableDecl<'db>,
    sink: &crate::hir_def::config::DataSink<'db>,
    path: PathExpr<'db>,
    names: &mut FxHashMap<PathExpr<'db>, VariableDecl<'db>>,
) -> Result<ConnectionEnd<'db>, IdeDiagnostic> {
    use crate::check::errors::e04_init::InitError;
    use crate::check::errors::e14_config::InputWriteRoute;
    use crate::hir_def::config::DataSink;
    use crate::hir_def::pous::variable::LocationArea;
    let file = path.get_scope_id(db).file(db);
    let refused = |why| ConfigError::ProgElementRefused { expr: path, why }.to_diagnostic(db, file);
    let (end, address) = match sink {
        DataSink::Path(global) => {
            let global = connected_global(db, *global, names).map_err(refused)?;
            if global.qualifier(db).contains(crate::Qualifier::CONSTANT) {
                return Err(InitError::AssignToConstant {
                    access: crate::CallSite::from_scoped(db, &path),
                }
                .to_diagnostic(db, file));
            }
            let address = crate::hir_ty::index_graphs::effective_location(db, global)
                .and_then(|dv| LocatedAddress::of(db, dv));
            (ConnectionEnd::Global(global), address)
        }
        DataSink::Direct(dv) => {
            let address = connected_address(db, *dv, path)?;
            (ConnectionEnd::Address(address.clone()), Some(address))
        }
    };
    if let Some(address) = address
        && address.area == LocationArea::Input
    {
        return Err(ConfigError::WriteToInputLocation {
            site: crate::CallSite::from_scoped(db, &path),
            address: address.text,
            via: InputWriteRoute::Assignment,
        }
        .to_diagnostic(db, file));
    }
    check_connected(db, var, &end).map_err(refused)?;
    Ok(end)
}

/// Every entry of every fragment of `config`'s configuration.
fn configuration_entries<'db>(
    db: &'db dyn WorkspaceDataBase,
    config: ConfigDecl<'db>,
) -> Vec<&'db ConfigEntry<'db>> {
    crate::hir_ty::index_graphs::config_fragments(db, config.get_name_ident(db))
        .into_iter()
        .flat_map(|f| resolve_config_entries(db, f).entries.iter())
        .collect()
}

/// Reports `config`'s VAR_CONFIG entries and keeps the locations `__init`
/// applies: an entry's own errors, then, for a location, the checks that
/// look at the other addresses (a bit of a wider one) and the other entries
/// (the same variable located twice), and, for a value, the checks
/// [`check_config_value`] makes.
fn check_config_entries<'db>(
    db: &'db dyn WorkspaceDataBase,
    config: ConfigDecl<'db>,
    locations: &mut Vec<ConfigLocation<'db>>,
    values: &mut Vec<ConfigValue<'db>>,
    errors: &mut Vec<IdeDiagnostic>,
) {
    use crate::check::errors::e14_config::ConfigLocationRefusal as Refusal;
    let resolved = resolve_config_entries(db, config);
    errors.extend(resolved.errors.iter().cloned());
    if resolved.entries.is_empty() {
        return;
    }
    let file = config.get_scope_id(db).file(db);
    let all = configuration_entries(db, config);
    for entry in &resolved.entries {
        if let (Some((dv, located)), Some(var)) = (&entry.location, entry.members.last()) {
            let refusal = match located {
                EntryLocation::Refused(why) => Some(why.clone()),
                EntryLocation::Given(address) => {
                    // Located twice: every entry is reported, pointing at the
                    // smallest of the others, since fragments have no order.
                    let other = all
                        .iter()
                        .filter(|o| {
                            o.path != entry.path
                                && o.location.is_some()
                                && o.instance.caseless(db) == entry.instance.caseless(db)
                                && o.members == entry.members
                        })
                        .min_by_key(|o| {
                            (
                                o.path.get_scope_id(db).file(db).url(db).to_string(),
                                o.path.get_span(db).start_byte,
                            )
                        });
                    if let Some(other) = other {
                        Some(Refusal::LocatedTwice {
                            other: other
                                .location
                                .as_ref()
                                .map(|(dv, _)| compact_str::CompactString::from(dv.to_address(db)))
                                .unwrap_or_default(),
                        })
                    } else if address.width < 8
                        && let Some(view) = crate::hir_ty::index_graphs::located_view(db, address)
                    {
                        // A bit of a wider address is bits of that address's
                        // cell: nothing a pointer can hold.
                        Some(Refusal::BitOfWider {
                            owner: view.owner.text,
                        })
                    } else {
                        None
                    }
                }
            };
            match (refusal, located) {
                (Some(why), _) => errors.push(
                    ConfigError::ConfigLocationRefused {
                        expr: entry.path,
                        var: var.name(db).text(db).clone(),
                        address: compact_str::CompactString::from(dv.to_address(db)),
                        why,
                    }
                    .to_diagnostic(db, file),
                ),
                (None, EntryLocation::Given(address)) => locations.push(ConfigLocation {
                    instance: entry.instance,
                    members: entry.members.clone(),
                    address: address.clone(),
                }),
                (None, EntryLocation::Refused(_)) => {}
            }
        }

        if let Some(init) = entry.init
            && let Some(value) = check_config_value(db, entry, init, &all, file, errors)
        {
            values.push(value);
        }
    }
}

/// A VAR_CONFIG value is its variable's starting value in one instance;
/// `__init` writes it after the instance's own initializers. It is refused
/// for the PROGRAM instance itself, when another entry gives the variable or
/// an instance holding it a value too, and when the variable's channel does
/// not start at a value of its own, by the rules a declaration's initial
/// value follows: an input the host writes (E1419), a part of a wider
/// address, whose value is its owner's (E1423), or an address a declaration
/// names, whose value is that declaration's (E1426).
fn check_config_value<'db>(
    db: &'db dyn WorkspaceDataBase,
    entry: &ConfigEntry<'db>,
    init: crate::hir_def::expressions::expression::InitExpr<'db>,
    all: &[&'db ConfigEntry<'db>],
    file: auto_lsp::default::db::file::File,
    errors: &mut Vec<IdeDiagnostic>,
) -> Option<ConfigValue<'db>> {
    use crate::check::errors::e14_config::ConfigEntryRefusal;
    use crate::hir_ty::index_graphs::{located_declaration, located_view};
    let refuse = |why| {
        ConfigError::ConfigEntryRefused {
            expr: entry.path,
            why,
        }
        .to_diagnostic(db, file)
    };
    let Some(var) = entry.members.last() else {
        errors.push(refuse(ConfigEntryRefusal::ProgramValue));
        return None;
    };
    let var_name = var.name(db).text(db).clone();
    let same_instance =
        |o: &ConfigEntry<'db>| o.instance.caseless(db) == entry.instance.caseless(db);
    // Every such entry is reported, since fragments have no order.
    let twice = all.iter().any(|o| {
        o.path != entry.path
            && o.init.is_some()
            && same_instance(o)
            && (o.members.starts_with(&entry.members) || entry.members.starts_with(&o.members))
    });
    if twice {
        errors.push(refuse(ConfigEntryRefusal::ValueTwice { var: var_name }));
        return None;
    }
    // A PROGRAM's VAR located in full is its channel, and its declaration
    // gives that channel its value.
    if var.is_program_located(db) {
        errors.push(refuse(ConfigEntryRefusal::ChannelDeclared {
            var: var_name,
            address: var
                .location(db)
                .map(|dv| compact_str::CompactString::from(dv.to_address(db)))
                .unwrap_or_default(),
        }));
        return None;
    }
    if !var.is_partly_located(db) {
        return Some(ConfigValue {
            instance: entry.instance,
            members: entry.members.clone(),
            init,
            channel: None,
        });
    }
    // A variable VAR_CONFIG locates starts its channel: the address this
    // entry gives it, or another entry's. With none, the variable is a
    // pointer nothing binds, which E1424 or E1425 already reports.
    let channel = all
        .iter()
        .filter(|o| same_instance(o) && o.members == entry.members)
        .find_map(|o| match &o.location {
            Some((_, EntryLocation::Given(address))) => Some(address.clone()),
            _ => None,
        })?;
    if channel.area == crate::hir_def::pous::variable::LocationArea::Input {
        errors.push(
            ConfigError::WriteToInputLocation {
                site: crate::CallSite::from_scoped(db, &entry.path),
                address: channel.text.clone(),
                via: crate::check::errors::e14_config::InputWriteRoute::Initializer,
            }
            .to_diagnostic(db, file),
        );
        return None;
    }
    if let Some(view) = located_view(db, &channel) {
        use crate::check::errors::e14_config::{OwnerDeclaration, WiderAddressUse};
        let owner = if located_declaration(db, &view.owner).is_some() {
            OwnerDeclaration::Declared
        } else if crate::hir_ty::index_graphs::config_located(db).any(|a| *a == view.owner) {
            OwnerDeclaration::Configured
        } else {
            OwnerDeclaration::Bare
        };
        errors.push(
            ConfigError::PartOfWiderAddress {
                site: crate::CallSite::from_scoped(db, &entry.path),
                address: channel.text.clone(),
                owner: view.owner.text,
                usage: WiderAddressUse::Initializer(owner),
            }
            .to_diagnostic(db, file),
        );
        return None;
    }
    if located_declaration(db, &channel).is_some() {
        errors.push(refuse(ConfigEntryRefusal::ChannelDeclared {
            var: var_name,
            address: channel.text.clone(),
        }));
        return None;
    }
    Some(ConfigValue {
        instance: entry.instance,
        members: entry.members.clone(),
        init,
        channel: Some(channel),
    })
}

/// Whether `dv` is an address VAR_CONFIG can give `var`, as far as the entry
/// and the variable alone decide: `var` is declared `AT %I*`, `%Q*` or
/// `%M*`, and `dv` is a complete address in that area, as wide as `var`'s
/// type. Whether a pointer can reach it (a bit of a wider address) needs the
/// other addresses, and [`check_config_entries`] asks.
fn check_config_location<'db>(
    db: &'db dyn WorkspaceDataBase,
    var: VariableDecl<'db>,
    dv: crate::hir_def::pous::variable::DirectVariable<'db>,
) -> Result<LocatedAddress, crate::check::errors::e14_config::ConfigLocationRefusal> {
    use crate::check::errors::e14_config::ConfigLocationRefusal as Refusal;
    let declared_at = match var.location(db) {
        Some(declared) if var.is_partly_located(db) => declared,
        _ => return Err(Refusal::NotPartlyLocated),
    };
    let Some(address) = LocatedAddress::of(db, dv) else {
        return Err(Refusal::Unlocatable {
            incomplete: dv.partly(db),
        });
    };
    if declared_at.area(db) != Some(address.area) {
        return Err(Refusal::AreaMismatch {
            declared: compact_str::CompactString::from(declared_at.to_address(db)),
        });
    }
    let declared = Type::resolve_spec(db, var.spec(db));
    let declared_bits = if declared.as_subrange(db).is_some() {
        None
    } else {
        crate::hir_ty::head::checks::variables::located_width(declared.normalize(db))
    };
    if !declared.is_never() && declared_bits != Some(usize::from(address.width)) {
        return Err(Refusal::Width {
            address_bits: usize::from(address.width),
            declared_bits,
            declared: compact_str::CompactString::from(declared.type_name(db)),
        });
    }
    Ok(address)
}

/// Every instance's variable declared `AT %I*`, `%Q*` or `%M*` has a location
/// in VAR_CONFIG (E1425): the standard makes a missing one an error, and a
/// pointer nothing was bound to would read and write address 0. Any entry of
/// the configuration that names the variable counts, refused or not: a
/// refused one has its own error.
fn check_partly_located_coverage<'db>(
    db: &'db dyn WorkspaceDataBase,
    config: ConfigDecl<'db>,
    result: &mut ConfigInferenceResult<'db>,
) {
    use crate::check::errors::e14_config::PartlyUnlocated;
    let all = configuration_entries(db, config);
    for resource in config.resources(db).iter() {
        for p in resource.programs(db).iter() {
            let instance = p.name(db);
            let Some(program) = result.prog_instance.get(&instance.ident.caseless(db)) else {
                continue;
            };
            let mut paths = Vec::new();
            // Instance state only, as `instance_members` has it for a
            // function block: a VAR_TEMP is made per call (E1425 says so where
            // it is declared) and a VAR_EXTERNAL is a global's.
            crate::hir_ty::head::inheritance::collect_partly_located(
                db,
                &mut program
                    .variables(db)
                    .iter()
                    .copied()
                    .filter(|v| !v.is_temp(db) && !v.is_external(db)),
                &mut Vec::new(),
                &mut paths,
                &mut Vec::new(),
            );
            for path in paths {
                let located = all.iter().any(|e| {
                    e.location.is_some()
                        && e.instance.caseless(db) == instance.ident.caseless(db)
                        && e.members == path
                });
                if located {
                    continue;
                }
                let mut text = instance.ident.text(db).to_string();
                for member in &path {
                    text.push('.');
                    text.push_str(member.name(db).text(db));
                }
                let address = path
                    .last()
                    .and_then(|v| v.location(db))
                    .map(|dv| dv.to_address(db))
                    .unwrap_or_default();
                result.errors.push(
                    ConfigError::PartlyLocatedUnlocated(PartlyUnlocated::Missing {
                        instance,
                        path: compact_str::CompactString::from(text),
                        address: compact_str::CompactString::from(address),
                    })
                    .to_diagnostic(db, config.get_scope_id(db).file(db)),
                );
            }
        }
    }
}
