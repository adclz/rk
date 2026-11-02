// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

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
        config::{ConfigDecl, DataSource, ProgConfig, ResourceDecl, TaskConfig},
        expressions::{
            expression::{Elementary, Expr, ExprKind, PathExpr, PathExprKind, PrimaryExpr},
            spec::{ElementarySpec, SpecKind},
        },
        interned::identifier::{Ident, SpanIdent},
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
    /// As written, for what is shown; `decl` is its identity.
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
    /// As written, for what is shown; `decl` is its identity.
    pub name: Ident,
    /// What runs it. A task that cannot run is absent from this model
    /// entirely — see `unschedulable` for why.
    pub trigger: TaskTrigger<'db>,
    pub priority: Option<u32>,
    pub programs: Vec<ResolvedProgram<'db>>,
    /// Function blocks this task runs instead of the programs holding them.
    pub function_blocks: Vec<ResolvedTaskFb<'db>>,
}

/// What runs a TASK: a period, a rising edge, or both. It has at least one of
/// them; a task with neither cannot run and has no trigger.
#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub struct TaskTrigger<'db> {
    /// The period in nanoseconds. `None` for no periodic runs, which an
    /// INTERVAL left out or zero both say.
    pub interval_ns: Option<u64>,
    /// The bit whose rising edge runs the task. `None` for no edge, which a
    /// SINGLE left out or FALSE both say.
    pub single: Option<SingleRead<'db>>,
}

/// What a task's SINGLE reads.
#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub struct SingleRead<'db> {
    pub source: SingleSource<'db>,
    /// Where the bit sits in what `source` names: above zero for a bit of a
    /// wider variable, `status.%X3`.
    pub shift: u32,
}

/// The BOOL a task's SINGLE reads, at a place fixed when the module is built:
/// a host reads it without running anything.
#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum SingleSource<'db> {
    /// A VAR_GLOBAL, or what is reached inside one: `go`, `Res.go`, `flags.go`,
    /// `timer.q`.
    Global {
        global: VariableDecl<'db>,
        elements: Vec<SourceStep>,
    },
    /// A VAR_OUTPUT of a program instance, or what is reached inside one:
    /// `P1.done`, `P1.status.done`, `P1.flags[2]`.
    Output {
        /// The instance's name, as written where it is declared.
        instance: Ident,
        output: VariableDecl<'db>,
        elements: Vec<SourceStep>,
    },
    /// An address: `%IX2`, or the one a VAR_GLOBAL named is located at.
    Address(LocatedAddress),
}

/// A step from a variable to what a task parameter reads inside it.
#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum SourceStep {
    /// A STRUCT element or a function block's output, by its name as names
    /// are matched.
    Element(Ident),
    /// One bracket of subscripts, each folded.
    Index(Vec<i64>),
}

/// What a path in a task parameter reaches.
#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub struct ReachedPath<'db> {
    /// A [`SingleSource::Global`] or a [`SingleSource::Output`].
    pub source: SingleSource<'db>,
    pub ty: Type<'db>,
}

/// The paths a fragment's task parameters write, resolved for a SINGLE and an
/// INTERVAL alike.
#[derive(Debug, Default, PartialEq, Eq, salsa::Update)]
pub struct TaskPaths<'db> {
    /// What each path reaches, or why it reaches nothing a task can read.
    pub reached: FxHashMap<PathExpr<'db>, Result<ReachedPath<'db>, UnschedulableReason>>,
    /// The variable each step that names one names.
    pub names: FxHashMap<PathExpr<'db>, VariableDecl<'db>>,
    /// The type of each step inside a variable.
    pub elements: FxHashMap<PathExpr<'db>, Type<'db>>,
    /// The steps that name no variable: a resource, a program instance.
    pub steps: FxHashMap<PathExpr<'db>, ConfigPathStep<'db>>,
    /// The step of a refused path that is at fault, by the whole path, when
    /// it is not the last one.
    pub refused_at: FxHashMap<PathExpr<'db>, PathExpr<'db>>,
    pub errors: Vec<IdeDiagnostic>,
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
    /// As written, for what is shown; `decl` is its identity.
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
    pub prog_instance: FxHashMap<Ident, ProgramDecl<'db>>,

    /// What actually runs — see [`ResolvedSchedule`].
    pub schedule: ResolvedSchedule<'db>,

    /// Resolved PRIORITY per TASK. Absent when PRIORITY was omitted (E1405) or
    /// unusable (E1406) — either way consumers get a number or nothing
    pub task_priority: FxHashMap<TaskConfig<'db>, u32>,

    /// What runs each TASK that can actually be scheduled. A task missing from
    /// this map cannot run; consumers skip it without needing to re-derive why.
    pub task_triggers: FxHashMap<TaskConfig<'db>, TaskTrigger<'db>>,

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
        task_triggers: FxHashMap::default(),
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

    let mut seen_resources: FxHashMap<Ident, SpanIdent<'db>> = FxHashMap::default();

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
    check_instance_global_names(db, config, errors);

    // Phase 2: resolve each resource's tasks and validate the programs bound
    // to them. Tasks and programs only exist inside a RESOURCE.
    for r in config.resources(db).iter() {
        // Tasks are scoped to the RESOURCE that declares them.
        let resource_tasks: FxHashMap<Ident, TaskConfig<'db>> = r
            .tasks(db)
            .iter()
            .map(|t| (t.name(db).ident(db), *t))
            .collect();
        resolve_task_triggers(db, config, &resource_tasks, result);
        for p in r.programs(db).iter() {
            validate_prog_config(db, p, &resource_tasks, result);
            resolve_prog_instance(db, p, &mut result.prog_instance);
        }
    }

    // What the task parameters name.
    result
        .errors
        .extend(task_paths(db, config).errors.iter().cloned());

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

    // A PROGRAM RETAIN instance keeps its program's variables, none of which
    // may hold a reference (E0904). One declared RETAIN is refused where it
    // is declared.
    for p in config
        .resources(db)
        .iter()
        .flat_map(|r| r.programs(db).iter())
    {
        if p.retain(db) != Some(true) {
            continue;
        }
        let Some(program) = result.prog_instance.get(&p.name(db).ident(db)).copied() else {
            continue;
        };
        for var in program.variables(db) {
            if var.qualifier(db).contains(crate::Qualifier::RETAIN)
                || !crate::hir_ty::head::instances::retained_with_instance(db, *var)
            {
                continue;
            }
            if let Some(mut found) = crate::hir_ty::head::instances::retained_reference(
                db,
                var.spec(db).infer(db),
                &mut Vec::new(),
            ) {
                // The variable itself is the reference the instance keeps:
                // the program declaring it is where to mark it.
                if found.last_field().is_none() {
                    found.declared_in = Some(program.get_name_with_case(db));
                }
                result.errors.push(
                    crate::check::errors::e09_reference::ReferenceError::RetainedReference {
                        var: *var,
                        found,
                        instance: Some(p.name(db)),
                    }
                    .to_diagnostic(db, config.scope_id(db).file(db)),
                );
            }
        }
    }
}

/// A program instance and a VAR_GLOBAL share the configuration's names: both
/// root a path in the debug symbols (`Run.v0`), and a monitor asking for one
/// found two. Fragments merge, so each side is compared with every
/// fragment's, and each side reports, pointing at the smallest other by
/// (file URL, span) so the message reads the same on every run.
fn check_instance_global_names<'db>(
    db: &'db dyn WorkspaceDataBase,
    config: ConfigDecl<'db>,
    errors: &mut Vec<IdeDiagnostic>,
) {
    let fragments = crate::hir_ty::index_graphs::config_fragments(db, config.get_name_ident(db));
    let at =
        |file: auto_lsp::default::db::file::File, start: usize| (file.url(db).to_string(), start);
    let mut globals: Vec<VariableDecl<'db>> = fragments
        .iter()
        .flat_map(|c| c.variables(db).iter().copied())
        .collect();
    globals.sort_by_key(|g| at(g.get_scope_id(db).file(db), g.get_name_span(db).start_byte));
    let mut instances: Vec<SpanIdent<'db>> = fragments
        .iter()
        .flat_map(|c| c.resources(db).iter())
        .flat_map(|r| r.programs(db).iter().map(|p| p.name(db)))
        .collect();
    instances.sort_by_key(|i| at(i.get_scope_id(db).file(db), i.get_span(db).start_byte));

    let file = config.get_scope_id(db).file(db);
    for r in config.resources(db) {
        for p in r.programs(db) {
            let instance = p.name(db);
            let name = instance.ident(db);
            if let Some(global) = globals.iter().find(|g| g.get_name_ident(db) == name) {
                errors.push(
                    DuplicateError::InstanceGlobal {
                        instance,
                        global: *global,
                        at_instance: true,
                    }
                    .to_diagnostic(db, file),
                );
            }
        }
    }
    for global in config.variables(db) {
        let name = global.get_name_ident(db);
        if let Some(instance) = instances.iter().find(|i| i.ident(db) == name) {
            errors.push(
                DuplicateError::InstanceGlobal {
                    instance: *instance,
                    global: *global,
                    at_instance: false,
                }
                .to_diagnostic(db, file),
            );
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

    let mut seen_progs: FxHashMap<Ident, SpanIdent<'db>> = FxHashMap::default();
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
    seen: &mut FxHashMap<Ident, SpanIdent<'db>>,
    name: SpanIdent<'db>,
    mut on_duplicate: impl FnMut(SpanIdent<'db>, SpanIdent<'db>),
) {
    let key = name.ident(db);
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
    instances: &mut FxHashMap<Ident, ProgramDecl<'db>>,
) {
    if let SpecKind::Target(target) = p.prog_type(db).kind(db)
        && target.path.namespace.is_none()
        && let Some(prog) = program_index(db, target.path.target.ident(db))
    {
        instances.insert(p.name(db).ident(db), prog);
    }
}

fn validate_prog_config<'db>(
    db: &'db dyn WorkspaceDataBase,
    p: &ProgConfig<'db>,
    known_tasks: &FxHashMap<Ident, TaskConfig<'db>>,
    result: &mut ConfigInferenceResult<'db>,
) {
    // Program type resolution is now handled by infer_config_resources in signature inference.
    // Unknown program types are reported as E0203 (NoNamespaceItemFound) by infer_spec.

    // Resolve the WITH <task> reference if present.
    match p.task(db) {
        Some(task_ref) => match known_tasks.get(&task_ref.ident(db)) {
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
        let Some(program) = result.prog_instance.get(&p.name(db).ident(db)).copied() else {
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
                            var: member.name_with_case(db),
                            program: program.name_with_case(db),
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
        // A name that resolves to nothing is reported where it is written
        // (E0201), whether the task is bound or not.
        if reason == UnschedulableReason::UnknownName {
            continue;
        }
        reported.push(task);
        // At what SINGLE is given when the fault is there.
        let site = match task.single(db).filter(|_| reason.is_in_single()) {
            Some(DataSource::Constant(value)) => value.as_call_site(db),
            Some(DataSource::Path(path, _)) => task_paths(db, config)
                .refused_at
                .get(&path)
                .unwrap_or(&path)
                .as_call_site(db),
            Some(DataSource::Direct(dv)) => dv.as_call_site(db),
            None => task.name(db).as_call_site(db),
        };
        result.errors.push(
            ConfigError::UnschedulableTask {
                task: task.name(db),
                reason,
                site,
            }
            .to_diagnostic(db, task.get_scope_id(db).file(db)),
        );
    }
}

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
        let task_entry = |tasks: &mut Vec<ResolvedTask<'db>>,
                          task: TaskConfig<'db>,
                          trigger: &TaskTrigger<'db>| {
            tasks
                .iter()
                .position(|t| t.decl == task)
                .unwrap_or_else(|| {
                    tasks.push(ResolvedTask {
                        decl: task,
                        name: task.name(db).with_case,
                        trigger: trigger.clone(),
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
            let Some(program) = result.prog_instance.get(&p.name(db).ident(db)).copied() else {
                continue; // program type did not resolve — already diagnosed
            };
            // A task that cannot run contributes nothing to run.
            let Some(trigger) = result.task_triggers.get(&task) else {
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
                    let trigger = result.task_triggers.get(fb_task)?;
                    Some((*member, *fb_task, trigger))
                })
                .collect();
            let resolved = ResolvedProgram {
                decl: *p,
                instance_name: p.name(db).with_case,
                program,
                retain: p.retain(db),
                connections,
            };
            let at = task_entry(&mut tasks, task, trigger);
            tasks[at].programs.push(resolved);
            for (member, fb_task, trigger) in function_blocks {
                let at = task_entry(&mut tasks, fb_task, trigger);
                tasks[at].function_blocks.push(ResolvedTaskFb {
                    instance_name: p.name(db).with_case,
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
                name: r.name(db).with_case,
                cpu_type: r.resource_type_name(db),
                tasks,
            });
        }
    }
    result.schedule = ResolvedSchedule { resources };
}

/// Resolve what runs each TASK, recording the ones that cannot be honoured.
///
/// A task runs on a rising edge of its SINGLE and on its INTERVAL. A shape that
/// gives it neither used to be dropped by a bare `continue` in MIR with no
/// diagnostic anywhere: `rk check` reported nothing, `rk compile` succeeded,
/// and the core came out with no schedule — which the runtime then rejected
/// with an unrelated complaint about the program's body export.
///
/// Resolving it here rather than in MIR also means the answer is published
/// once: the scheduler reads `task_triggers` instead of parsing intervals a
/// second time.
fn resolve_task_triggers<'db>(
    db: &'db dyn WorkspaceDataBase,
    config: ConfigDecl<'db>,
    tasks: &FxHashMap<Ident, TaskConfig<'db>>,
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

        let single = task_single(db, config, *task, &mut result.errors);
        let interval = task_interval(db, config, *task);
        match (single, interval) {
            (Err(reason), _) | (_, Err(reason)) => {
                result.unschedulable.insert(*task, reason);
            }
            // A period, with or without an edge.
            (Ok(single), Ok(Some(ns))) if ns > 0 => {
                let trigger = TaskTrigger {
                    interval_ns: Some(ns),
                    single,
                };
                result.task_triggers.insert(*task, trigger);
            }
            // An edge alone: an INTERVAL of zero is the one the standard
            // gives a task by default, which schedules nothing periodically.
            (Ok(Some(single)), Ok(_)) => {
                let trigger = TaskTrigger {
                    interval_ns: None,
                    single: Some(single),
                };
                result.task_triggers.insert(*task, trigger);
            }
            (Ok(None), Ok(interval)) => {
                let reason = if task.single(db).is_some() {
                    // FALSE, with no period to leave running.
                    UnschedulableReason::ConstantSingle
                } else if interval.is_some() {
                    UnschedulableReason::ZeroInterval
                } else {
                    UnschedulableReason::NoTrigger
                };
                result.unschedulable.insert(*task, reason);
            }
        }
    }
}

/// What a task's SINGLE reads, `None` for no rising edge to wait for: one
/// left out, or a constant FALSE.
///
/// A constant FALSE stands at FALSE, which leaves an INTERVAL running as if
/// nothing had been written. A constant TRUE would run the task once, at its
/// first reading, and stop its INTERVAL, which is no way to schedule a task.
fn task_single<'db>(
    db: &'db dyn WorkspaceDataBase,
    config: ConfigDecl<'db>,
    task: TaskConfig<'db>,
    errors: &mut Vec<IdeDiagnostic>,
) -> Result<Option<SingleRead<'db>>, UnschedulableReason> {
    let Some(source) = task.single(db) else {
        return Ok(None);
    };
    match source {
        DataSource::Constant(value) => match bool_literal(db, value) {
            Some(value) => constant_single(db, task, Some(value)),
            None => Err(UnschedulableReason::SingleNotBool),
        },
        DataSource::Direct(dv) => {
            let address = LocatedAddress::of(db, dv).ok_or_else(|| {
                use crate::check::errors::e14_config::UnlocatableAddress;
                errors.push(
                    ConfigError::DirectVariableUnsupported {
                        site: dv.as_call_site(db),
                        address: dv.to_address(db).into(),
                        why: if dv.partly(db) {
                            UnlocatableAddress::Incomplete
                        } else {
                            UnlocatableAddress::Malformed
                        },
                    }
                    .to_diagnostic(db, task.get_scope_id(db).file(db)),
                );
                UnschedulableReason::UnknownName
            })?;
            if address.width != 1 {
                return Err(UnschedulableReason::SingleNotBool);
            }
            Ok(Some(SingleRead {
                source: SingleSource::Address(address),
                shift: 0,
            }))
        }
        DataSource::Path(path, part) => {
            let reached = reached_path(db, config, path)?;
            if reached.ty.is_never() {
                return Err(UnschedulableReason::UnknownName); // already diagnosed
            }
            // A part of what the path reaches, read as it is in a body.
            let (ty, shift) = match part {
                None => (reached.ty, 0),
                Some(part) => {
                    let file = task.get_scope_id(db).file(db);
                    if let Some(fault) = crate::hir_ty::resolver::walk::multibits_fault(
                        db, path, reached.ty, None, part,
                    ) {
                        errors.push(fault.to_diagnostic(db, file));
                        return Err(UnschedulableReason::UnknownName);
                    }
                    let slice = crate::hir_ty::infer::normalize::multibits_slice(db, part)
                        .ok_or(UnschedulableReason::UnknownName)?;
                    (Type::Elementary(slice.spec), slice.shift as u32)
                }
            };
            if ty.normalize(db) != Type::Elementary(ElementarySpec::Bool) {
                return Err(UnschedulableReason::SingleNotBool);
            }
            let source = match &reached.source {
                SingleSource::Global { global, elements }
                    if global.qualifier(db).contains(crate::Qualifier::CONSTANT) =>
                {
                    // A CONSTANT is as fixed as a literal; one with no value
                    // starts at its type's, FALSE.
                    let value = match (elements.as_slice(), part, global.init(db)) {
                        ([], None, None) => Some(false),
                        ([], None, Some(_)) => {
                            crate::hir_ty::infer::const_eval::constant_init(db, *global)
                                .and_then(|init| bool_literal(db, init))
                        }
                        _ => None,
                    };
                    return constant_single(db, task, value);
                }
                SingleSource::Global { global, elements } if elements.is_empty() => {
                    located_at(db, *global)
                        .map_or_else(|| reached.source.clone(), SingleSource::Address)
                }
                source => source.clone(),
            };
            Ok(Some(SingleRead { source, shift }))
        }
    }
}

/// What a constant SINGLE comes to, given its value when it is known.
fn constant_single<'db>(
    db: &'db dyn WorkspaceDataBase,
    task: TaskConfig<'db>,
    value: Option<bool>,
) -> Result<Option<SingleRead<'db>>, UnschedulableReason> {
    match value {
        Some(false) => Ok(None),
        Some(true) if task.interval(db).is_some() => {
            Err(UnschedulableReason::SingleAlwaysTrueBesideInterval)
        }
        Some(true) => Err(UnschedulableReason::SingleAlwaysTrue),
        None => Err(UnschedulableReason::ConstantSingle),
    }
}

/// A task's INTERVAL in nanoseconds, `None` when it has none. Zero stays zero:
/// what it means depends on the task's SINGLE.
///
/// The period is baked into the emitted schedule, so it must be known at
/// compile time — but "known at compile time" is wider than "written as a
/// literal". A `VAR_GLOBAL CONSTANT period : TIME := T#10ms` is just as fixed
/// as `T#10ms`, and rejecting it would be stating a language rule to cover a
/// missing resolution. A directly represented variable (`%MW0`) is the only
/// source that genuinely cannot supply one.
fn task_interval<'db>(
    db: &'db dyn WorkspaceDataBase,
    config: ConfigDecl<'db>,
    task: TaskConfig<'db>,
) -> Result<Option<u64>, UnschedulableReason> {
    let not_fixed = UnschedulableReason::NonLiteralInterval;
    let Some(source) = task.interval(db) else {
        return Ok(None);
    };
    match source {
        DataSource::Constant(value) => time_literal_nanos(db, value).map(Some).ok_or(not_fixed),
        DataSource::Path(path, part) => {
            // Only a CONSTANT can be trusted: an ordinary VAR_GLOBAL may be
            // written at runtime, and the schedule cannot follow it. Which
            // declaration is fixed, and where its value lives, is the same
            // question a CASE label asks — so it is asked in one place.
            let reached = reached_path(db, config, path).map_err(|why| {
                if why == UnschedulableReason::UnknownName {
                    why
                } else {
                    not_fixed
                }
            })?;
            let SingleSource::Global { global, elements } = reached.source else {
                return Err(not_fixed);
            };
            if !elements.is_empty() || part.is_some() {
                return Err(not_fixed);
            }
            crate::hir_ty::infer::const_eval::constant_init(db, global)
                .and_then(|init| time_literal_nanos(db, init))
                .map(Some)
                .ok_or(not_fixed)
        }
        // `%MW0` and friends: a period read from process memory is not a
        // compile-time fact at all.
        DataSource::Direct(_) => Err(not_fixed),
    }
}

/// What a task parameter's path reaches, as [`task_paths`] resolved it.
fn reached_path<'db>(
    db: &'db dyn WorkspaceDataBase,
    config: ConfigDecl<'db>,
    path: PathExpr<'db>,
) -> Result<ReachedPath<'db>, UnschedulableReason> {
    task_paths(db, config)
        .reached
        .get(&path)
        .cloned()
        .unwrap_or(Err(UnschedulableReason::UnknownName))
}

/// The address a variable is declared at, when it is a complete one.
fn located_at<'db>(
    db: &'db dyn WorkspaceDataBase,
    var: VariableDecl<'db>,
) -> Option<LocatedAddress> {
    LocatedAddress::of(
        db,
        crate::hir_ty::index_graphs::effective_location(db, var)?,
    )
}

/// The value of a BOOL literal: `TRUE`, `FALSE`, `BOOL#1`, `BOOL#0`, or a bare
/// `1` or `0`, which is a BOOL's as much as an integer's.
fn bool_literal<'db>(db: &'db dyn WorkspaceDataBase, value: Expr<'db>) -> Option<bool> {
    let ExprKind::PrimaryExpr(PrimaryExpr::Literal(literal)) = value.expr(db) else {
        return None;
    };
    if let Some(value) = literal.as_bool(db) {
        return Some(value);
    }
    let Elementary::InferInteger(integer) = literal else {
        return None;
    };
    match integer.as_i128(db).ok()? {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    }
}

/// What each path in `config`'s task parameters names: a SINGLE's and an
/// INTERVAL's alike, which the standard gives one form.
///
/// A query of its own, apart from [`infer_config_result`], so the
/// configuration's body inference records these names for the IDE and the
/// linter without reading the whole configuration. Nothing here infers an
/// expression.
#[salsa::tracked(returns(ref))]
pub fn task_paths<'db>(db: &'db dyn WorkspaceDataBase, config: ConfigDecl<'db>) -> TaskPaths<'db> {
    let mut out = TaskPaths::default();
    for r in config.resources(db).iter() {
        for task in r.tasks(db).iter() {
            for source in [task.interval(db), task.single(db)].into_iter().flatten() {
                if let DataSource::Path(path, _) = source {
                    let reached = read_task_path(db, config, *r, path, &mut out);
                    out.reached.insert(path, reached);
                }
            }
        }
    }
    out
}

/// The path of a task parameter, walked from what it starts at: a VAR_GLOBAL of
/// the configuration or a program instance of the task's resource, after the
/// name of a resource if it is written.
///
/// A resource may share its name with either of them, and `flags.go` then reads
/// two ways. It is the variable or the instance when the next name is inside
/// it, and the resource otherwise, so a path written without its resource means
/// what it says, and one written with it still resolves.
fn read_task_path<'db>(
    db: &'db dyn WorkspaceDataBase,
    config: ConfigDecl<'db>,
    resource: ResourceDecl<'db>,
    path: PathExpr<'db>,
    out: &mut TaskPaths<'db>,
) -> Result<ReachedPath<'db>, UnschedulableReason> {
    use crate::check::errors::e02_resolve::ResolveError;
    let file = path.get_scope_id(db).file(db);
    let steps = path.flatten(db).as_slice();
    let fragments = crate::hir_ty::index_graphs::config_fragments(db, config.get_name_ident(db));
    let global_named = |name: Ident| {
        fragments
            .iter()
            .flat_map(|f| f.variables(db).iter())
            .find(|v| v.get_name_ident(db) == name)
    };
    let instance_named = |name: Ident| {
        resource
            .programs(db)
            .iter()
            .find(|p| p.name(db).ident(db) == name)
    };
    let holds = |name: Ident, next: Ident| match global_named(name) {
        Some(global) => match global.spec(db).infer(db).normalize(db) {
            Type::Struct(st) => st.struct_elements(db).contains_key(&next),
            base @ (Type::FunctionBlock(_) | Type::Class(_)) => {
                instance_member(db, base, next).is_some()
            }
            _ => false,
        },
        None => instance_named(name)
            .and_then(|instance| program_of_instance(db, instance))
            .is_some_and(|program| program.variables(db).iter().any(|v| v.name(db) == next)),
    };

    let mut at = 0;
    if let [
        PathExprWalkStep::Field { ident, expr },
        PathExprWalkStep::Field { ident: next, .. },
        ..,
    ] = steps
        && let Some(written) = fragments
            .iter()
            .flat_map(|f| f.resources(db).iter())
            .find(|r| r.name(db).ident(db) == ident.ident(db))
        && !holds(ident.ident(db), next.ident(db))
    {
        out.steps.insert(*expr, ConfigPathStep::Resource(*written));
        at = 1;
    }
    let Some(PathExprWalkStep::Field { ident, expr }) = steps.get(at) else {
        return Err(UnschedulableReason::SingleNotFixed);
    };
    let name = ident.ident(db);

    if let Some(global) = global_named(name) {
        out.names.insert(*expr, *global);
        let (elements, ty) = walk_source_steps(
            db,
            &fragments,
            path,
            global.spec(db).infer(db),
            &steps[at + 1..],
            out,
        )?;
        return Ok(ReachedPath {
            source: SingleSource::Global {
                global: *global,
                elements,
            },
            ty,
        });
    }

    let Some(instance) = instance_named(name) else {
        out.errors.push(
            ResolveError::NoItemInScope {
                expr: *expr,
                scope: path.get_scope_id(db),
            }
            .to_diagnostic(db, file),
        );
        return Err(UnschedulableReason::UnknownName);
    };
    out.steps.insert(*expr, ConfigPathStep::Instance(*instance));
    let Some(program) = program_of_instance(db, instance) else {
        return Err(UnschedulableReason::UnknownName); // already diagnosed
    };
    let (member, member_expr) = match steps.get(at + 1) {
        Some(PathExprWalkStep::Field { ident, expr }) => (ident, expr),
        // The instance itself.
        None => return Err(UnschedulableReason::SingleNotBool),
        // A bracket or a dereference of it, reported as a body reports one.
        Some(step) => {
            out.errors.push(not_indexed_or_dereferenced(
                db,
                step,
                Type::Program(program),
            ));
            return Err(UnschedulableReason::UnknownName);
        }
    };
    let Some(var) = program
        .variables(db)
        .iter()
        .find(|v| v.name(db) == member.ident(db))
    else {
        out.errors.push(
            ResolveError::NoSuchFieldPathExpr {
                expr: *member_expr,
                ident: member.with_case,
                ty: Type::Program(program),
            }
            .to_diagnostic(db, file),
        );
        return Err(UnschedulableReason::UnknownName);
    };
    out.names.insert(*member_expr, *var);
    if !var.is_output(db) {
        out.refused_at.insert(path, *member_expr);
        return Err(UnschedulableReason::SingleNotOutput);
    }
    let (elements, ty) = walk_source_steps(
        db,
        &fragments,
        path,
        var.spec(db).infer(db),
        &steps[at + 2..],
        out,
    )?;
    Ok(ReachedPath {
        source: SingleSource::Output {
            instance: instance.name(db).with_case,
            output: *var,
            elements,
        },
        ty,
    })
}

/// The value of a subscript in a task parameter's path. A bare name binds
/// through the scope chain, which is one fragment's: a CONSTANT of another is
/// looked up here, and a name no fragment declares is reported as a name of
/// the path is.
fn source_subscript<'db>(
    db: &'db dyn WorkspaceDataBase,
    fragments: &[ConfigDecl<'db>],
    subscript: Expr<'db>,
    out: &mut TaskPaths<'db>,
) -> Result<i64, UnschedulableReason> {
    use crate::hir_def::expressions::expression::VariableAccessKind;
    use crate::hir_ty::infer::const_eval::{bare_access_name, constant_init, spec_bound};
    let not_fixed = UnschedulableReason::SingleNotFixed;
    if let Some(value) = spec_bound(db, subscript) {
        return Ok(value);
    }
    let file = subscript.get_scope_id(db).file(db);
    let not_an_integer = |ty: Type<'db>| {
        crate::check::errors::e05_array::ArrayError::NonIntegerIndex {
            expr: subscript,
            ty,
        }
        .to_diagnostic(db, file)
    };
    let access = match subscript.expr(db) {
        ExprKind::PrimaryExpr(PrimaryExpr::VariableAccess(access)) => access,
        // An integer literal folds, unless it is too wide for any index.
        ExprKind::PrimaryExpr(PrimaryExpr::Literal(literal)) => {
            let ty = match Type::from(*literal) {
                Type::Infer(infer) => infer.to_ty(db),
                ty => ty,
            };
            if ty.is_signed_integer() || ty.is_unsigned_integer() {
                return Err(not_fixed);
            }
            out.errors.push(not_an_integer(ty));
            return Err(UnschedulableReason::UnknownName);
        }
        _ => return Err(not_fixed),
    };
    let Some(name) = bare_access_name(db, *access) else {
        return Err(not_fixed);
    };
    let global = fragments
        .iter()
        .flat_map(|f| f.variables(db).iter())
        .find(|v| v.get_name_ident(db) == name);
    if let Some(global) = global {
        let ty = global.spec(db).infer(db);
        let base = ty.normalize(db);
        if !base.is_never() && !base.is_signed_integer() && !base.is_unsigned_integer() {
            out.errors.push(not_an_integer(ty));
            return Err(UnschedulableReason::UnknownName);
        }
        return constant_init(db, *global)
            .and_then(|init| spec_bound(db, init))
            .ok_or(not_fixed);
    }
    if let VariableAccessKind::Symbolic(begin) = access.kind(db)
        && let Some(path) = begin.expr(db)
    {
        out.errors.push(
            crate::check::errors::e02_resolve::ResolveError::NoItemInScope {
                expr: path,
                scope: subscript.get_scope_id(db),
            }
            .to_diagnostic(db, subscript.get_scope_id(db).file(db)),
        );
    }
    Err(UnschedulableReason::UnknownName)
}

/// The program a program configuration instantiates, when its type resolves.
fn program_of_instance<'db>(
    db: &'db dyn WorkspaceDataBase,
    instance: &ProgConfig<'db>,
) -> Option<ProgramDecl<'db>> {
    let mut program = FxHashMap::default();
    resolve_prog_instance(db, instance, &mut program);
    program.into_values().next()
}

/// The steps from a variable of type `ty` to what a task parameter reads
/// inside it, and the type it ends at. A step is a STRUCT element, a function
/// block's output, or a bracket of subscripts that fold, which keeps the place
/// fixed.
fn walk_source_steps<'db>(
    db: &'db dyn WorkspaceDataBase,
    fragments: &[ConfigDecl<'db>],
    path: PathExpr<'db>,
    mut ty: Type<'db>,
    steps: &[PathExprWalkStep<'db>],
    out: &mut TaskPaths<'db>,
) -> Result<(Vec<SourceStep>, Type<'db>), UnschedulableReason> {
    use crate::check::errors::{e02_resolve::ResolveError, e05_array::ArrayError};
    let mut walked = Vec::new();
    // The array whose dimensions the brackets so far stop short of, `m[i]` of
    // `m[i][j]`, and how many they took.
    let mut row: Option<(crate::hir_def::expressions::spec::Array<'db>, usize)> = None;
    for step in steps {
        if ty.is_never() {
            return Err(UnschedulableReason::UnknownName); // already diagnosed
        }
        match step {
            PathExprWalkStep::Field { ident, expr } => {
                let file = expr.get_scope_id(db).file(db);
                let no_field = |ty| {
                    ResolveError::NoSuchFieldPathExpr {
                        expr: *expr,
                        ident: ident.with_case,
                        ty,
                    }
                    .to_diagnostic(db, file)
                };
                let base = ty.normalize(db);
                let next = match base {
                    Type::Struct(st) => {
                        st.struct_elements(db).get(&ident.ident(db)).map(|element| {
                            out.elements.insert(*expr, Type::StructElement(*element));
                            element.spec(db).infer(db)
                        })
                    }
                    Type::FunctionBlock(_) | Type::Class(_) => {
                        match instance_member(db, base, ident.ident(db)) {
                            Some(var) => {
                                out.names.insert(*expr, var);
                                if !var.is_output(db) {
                                    out.refused_at.insert(path, *expr);
                                    return Err(UnschedulableReason::SingleNotOutput);
                                }
                                Some(var.spec(db).infer(db))
                            }
                            None => None,
                        }
                    }
                    _ => None,
                };
                let Some(next) = next else {
                    out.errors.push(no_field(ty));
                    return Err(UnschedulableReason::UnknownName);
                };
                ty = next;
                walked.push(SourceStep::Element(ident.ident(db)));
            }
            PathExprWalkStep::Index { expr } => {
                let file = expr.get_scope_id(db).file(db);
                let not_an_array = || {
                    ArrayError::IndexNonArrayTypePathExpr { expr: *expr, ty }
                        .to_diagnostic(db, file)
                };
                let (array, first) = match (row.take(), ty.normalize(db)) {
                    (Some(row), _) => row,
                    (None, Type::Array(array)) => (array, 0),
                    _ => {
                        out.errors.push(not_an_array());
                        return Err(UnschedulableReason::UnknownName);
                    }
                };
                let PathExprKind::Index(bracket) = expr.expr(db) else {
                    return Err(UnschedulableReason::SingleNotFixed);
                };
                let dimensions = crate::hir_ty::infer::const_eval::array_dimensions(db, array);
                let through = first + bracket.index.len();
                if through > dimensions.len() {
                    out.errors.push(not_an_array());
                    return Err(UnschedulableReason::UnknownName);
                }
                let mut subscripts = Vec::with_capacity(bracket.index.len());
                for (k, subscript) in bracket.index.iter().enumerate() {
                    let value = source_subscript(db, fragments, *subscript, out)?;
                    let dimension = first + k;
                    if let (Some(min), Some(max)) = dimensions[dimension]
                        && !(min..=max).contains(&value)
                    {
                        out.errors.push(
                            ArrayError::IndexOutOfBounds {
                                expr: *subscript,
                                dimension,
                                index: i128::from(value),
                                min,
                                max,
                            }
                            .to_diagnostic(db, file),
                        );
                        return Err(UnschedulableReason::UnknownName);
                    }
                    subscripts.push(value);
                }
                if through == dimensions.len() {
                    ty = array.of_type(db).infer(db);
                    out.elements.insert(*expr, ty);
                } else {
                    row = Some((array, through));
                }
                walked.push(SourceStep::Index(subscripts));
            }
            // A reference has no fixed target. Anything else cannot be
            // dereferenced, wherever it is.
            PathExprWalkStep::Deref { .. } => {
                if matches!(ty.normalize(db), Type::RefTo(_)) {
                    return Err(UnschedulableReason::SingleNotFixed);
                }
                out.errors.push(not_indexed_or_dereferenced(db, step, ty));
                return Err(UnschedulableReason::UnknownName);
            }
        }
    }
    Ok((walked, ty))
}

/// What a body reports for a bracket or a dereference of what takes neither.
fn not_indexed_or_dereferenced<'db>(
    db: &'db dyn WorkspaceDataBase,
    step: &PathExprWalkStep<'db>,
    ty: Type<'db>,
) -> IdeDiagnostic {
    use crate::check::errors::{e05_array::ArrayError, e09_reference::ReferenceError};
    let expr = step.get_expr(db);
    let file = expr.get_scope_id(db).file(db);
    match step {
        PathExprWalkStep::Deref { .. } => {
            ReferenceError::DerefNonRefType { expr, ty }.to_diagnostic(db, file)
        }
        _ => ArrayError::IndexNonArrayTypePathExpr { expr, ty }.to_diagnostic(db, file),
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
pub enum EntryLocation<'db> {
    /// A complete address in the variable's area, as wide as its type.
    Given(LocatedAddress),
    Refused(crate::check::errors::e14_config::ConfigLocationRefusal<'db>),
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
        EntryLocation<'db>,
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
        let name = ident.ident(db);
        fragments
            .iter()
            .flat_map(|f| f.resources(db).iter())
            .find(|r| r.name(db).ident(db) == name)
            .copied()
    };
    let instance_named = |ident: &SpanIdent<'db>| {
        let name = ident.ident(db);
        fragments
            .iter()
            .flat_map(|f| f.resources(db).iter())
            .flat_map(|r| r.programs(db).iter())
            .find(|p| p.name(db).ident(db) == name)
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
                let next = match &steps[1] {
                    PathExprWalkStep::Field { ident, .. } => Some(ident.ident(db)),
                    _ => None,
                };
                let resource = resource_named(ident).filter(|_| {
                    config_path_starts_at_resource(db, &fragments, ident.ident(db), next)
                });
                match resource {
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
        let Some(prog) = instances.get(&first_ident.ident(db)).copied() else {
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
            match instance_member(db, current_type, ident.ident(db)) {
                Some(var) => {
                    current_type = var.spec(db).infer(db).normalize(db);
                    members.push(var);
                }
                None => {
                    let error = match declared_but_not_held(db, current_type, ident.ident(db)) {
                        Some(held) => ConfigError::ConfigEntryRefused {
                            expr: decl.path,
                            why: ConfigEntryRefusal::NotHeld {
                                field: *ident,
                                parent_type: current_type,
                                decl: held,
                            },
                        },
                        None => ConfigError::ConfigInstInitFieldNotFound {
                            field: *ident,
                            parent_type: current_type,
                        },
                    };
                    out.errors.push(error.to_diagnostic(db, file));
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
                            var: var.name_with_case(db),
                            written,
                            declared,
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
            instance: first_ident.ident(db),
            members,
            location,
            init: decl.init,
        });
    }
    out
}

/// Whether a VAR_CONFIG path starting `first.next` starts at a resource. A
/// resource may share its name with a program instance, as `RESOURCE Main` and
/// `PROGRAM Main` do: `first` is then the instance, unless `next` is none of its
/// variables and names an instance itself, as in `Main.Main.x`.
fn config_path_starts_at_resource<'db>(
    db: &'db dyn WorkspaceDataBase,
    fragments: &[ConfigDecl<'db>],
    first: Ident,
    next: Option<Ident>,
) -> bool {
    let resources = || fragments.iter().flat_map(|f| f.resources(db).iter());
    if !resources().any(|r| r.name(db).ident(db) == first) {
        return false;
    }
    let instances = || resources().flat_map(|r| r.programs(db).iter());
    let Some(instance) = instances().find(|p| p.name(db).ident(db) == first) else {
        return true;
    };
    let Some(next) = next else {
        return false;
    };
    let holds = program_of_instance(db, instance)
        .is_some_and(|program| program.variables(db).iter().any(|v| v.name(db) == next));
    !holds && instances().any(|p| p.name(db).ident(db) == next)
}

/// The member `name` among [`members_of`] `ty`.
fn instance_member<'db>(
    db: &'db dyn WorkspaceDataBase,
    ty: Type<'db>,
    name: Ident,
) -> Option<VariableDecl<'db>> {
    let fold = name;
    members_of(db, ty).into_iter().find(|v| v.name(db) == fold)
}

/// The VAR_TEMP or VAR_EXTERNAL named `name` that `ty` declares: what a
/// VAR_CONFIG path can name and no instance holds, said as such (E1426).
fn declared_but_not_held<'db>(
    db: &'db dyn WorkspaceDataBase,
    ty: Type<'db>,
    name: Ident,
) -> Option<VariableDecl<'db>> {
    use crate::hir_def::pous::pou::Pou;
    let declared: &[VariableDecl<'db>] = match ty {
        Type::Program(p) => p.variables(db),
        _ => match crate::hir_ty::head::instances::pou_of_type(db, ty) {
            Some(Pou::FunctionBlock(fb)) => fb.variables(db),
            Some(Pou::Class(cl)) => cl.variables(db),
            _ => &[],
        },
    };
    declared
        .iter()
        .find(|v| v.name(db) == name && (v.is_temp(db) || v.is_external(db)))
        .copied()
}

/// Every member an instance of `ty` holds: a PROGRAM's instance state, or
/// an FB's or CLASS's [`instance_members`], inherited ones included.
///
/// [`instance_members`]: crate::hir_ty::oop::instance_members
pub fn members_of<'db>(db: &'db dyn WorkspaceDataBase, ty: Type<'db>) -> Vec<VariableDecl<'db>> {
    match ty {
        Type::Program(p) => p
            .variables(db)
            .iter()
            .copied()
            .filter(|v| !v.is_temp(db) && !v.is_external(db))
            .collect(),
        _ => match crate::hir_ty::head::instances::pou_of_type(db, ty) {
            Some(pou) => crate::hir_ty::oop::instance_members(db, pou)
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
    /// A resource: one of its program instances comes next, or a variable of
    /// the instance it shares its name with.
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
        .map(|step| Ident::new(db, compact_str::CompactString::from(*step)).folded(db))
        .collect();
    let fragments = crate::hir_ty::index_graphs::config_fragments(db, config.get_name_ident(db));
    let resources: Vec<ResourceDecl<'db>> = fragments
        .iter()
        .flat_map(|f| f.resources(db).iter().copied())
        .collect();
    let mut steps = steps.iter().peekable();
    let Some(first) = steps.next() else {
        return Some(ConfigPathPrefix::Start);
    };
    let resource = resources
        .iter()
        .find(|r| r.name(db).ident(db) == *first)
        .copied();
    let next = steps.peek().map(|name| **name);
    if let (Some(r), None) = (resource, next) {
        return Some(ConfigPathPrefix::Resource(r));
    }
    let instance = match resource {
        Some(r) if config_path_starts_at_resource(db, &fragments, *first, next) => {
            let name = steps.next()?;
            r.programs(db)
                .iter()
                .find(|p| p.name(db).ident(db) == *name)
                .copied()
        }
        _ => resources
            .iter()
            .flat_map(|r| r.programs(db).iter())
            .find(|p| p.name(db).ident(db) == *first)
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
                let name = var.name_with_case(db);
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
                            .find(|t| t.name(db).ident(db) == fb.task.ident(db));
                        if let Some(task) = task {
                            out.tasks.insert(path, *task);
                        }
                        let block = match var.spec(db).infer(db).normalize(db) {
                            Type::FunctionBlock(block) => Some(block),
                            _ => None,
                        };
                        // A task's run binds no VAR_IN_OUT, and an `ARRAY[*]`
                        // one has no body to run without.
                        let conformand = block.and_then(|block| {
                            crate::hir_ty::oop::instance_members(
                                db,
                                crate::hir_def::pous::pou::Pou::FunctionBlock(block),
                            )
                            .iter()
                            .find(|m| m.var.is_in_out(db) && m.var.conformand(db).is_some())
                            .map(|m| m.var.name_with_case(db))
                        });
                        if block.is_none() {
                            Some(ProgElementRefusal::NotAFunctionBlock { var: name })
                        } else if let Some(param) = conformand {
                            Some(ProgElementRefusal::UnboundConformand { var: name, param })
                        } else if let Some(task) = task {
                            named.push((var, path, true));
                            resolved.function_blocks.push((var, *task, path));
                            None
                        } else {
                            // The one code for a WITH naming no TASK, after a
                            // program or after a block.
                            out.errors.push(
                                ConfigError::UnknownTaskRef { task: fb.task }
                                    .to_diagnostic(db, file),
                            );
                            None
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
            // One report per input or block, from its first element,
            // pointing at the others.
            for (var, path, fb) in &named {
                if !twice(var, *fb) {
                    continue;
                }
                let start = |p: &PathExpr<'db>| p.get_span(db).start_byte;
                let mut others: Vec<_> = named
                    .iter()
                    .filter(|(v, p, f)| v == var && f == fb && p != path)
                    .map(|(_, p, _)| *p)
                    .collect();
                if others.iter().any(|o| start(o) < start(path)) {
                    continue;
                }
                others.sort_by_key(start);
                let var = var.name_with_case(db);
                let why = if *fb {
                    ProgElementRefusal::AssociatedTwice { var, others }
                } else {
                    ProgElementRefusal::ConnectedTwice { var, others }
                };
                out.errors.push(
                    ConfigError::ProgElementRefused { expr: *path, why }.to_diagnostic(db, file),
                );
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
    // A task's INTERVAL or SINGLE may name a VAR_GLOBAL or a program's output,
    // and go on inside it.
    let paths = task_paths(db, config);
    named.extend(paths.names.iter().map(|(path, var)| (*path, *var)));
    for (path, ty) in &paths.elements {
        result.type_of_path_expr.insert(*path, *ty);
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
    let var = instance_member(db, Type::Program(program), ident.ident(db));
    if var.is_none() {
        errors.push(
            ConfigError::ProgElementRefused {
                expr: path,
                why: ProgElementRefusal::NoSuchVariable {
                    var: ident.with_case,
                    program: program.name_with_case(db),
                },
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
        crate::hir_ty::index_graphs::external_var_lookup(db, ident.ident(db)).ok_or_else(|| {
            ProgElementRefusal::NoSuchGlobal {
                name: ident.with_case,
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
    let name = var.name_with_case(db);
    match end {
        ConnectionEnd::Constant(_) => Ok(()),
        ConnectionEnd::Global(global) => {
            let ty = global.spec(db).infer(db);
            if ty.is_never() || global_connects(db, var, *global) {
                return Ok(());
            }
            Err(ProgElementRefusal::TypeMismatch {
                var: name,
                declared,
                global: global.name_with_case(db),
                ty,
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
                declared,
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
        DataSource::Path(global, _) => {
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
                    constant: Some(global),
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
                    // Located twice: one report per variable, from the first
                    // entry in file order, pointing at the others; none of
                    // them locates it.
                    let position = |p: &PathExpr<'db>| {
                        (
                            p.get_scope_id(db).file(db).url(db).to_string(),
                            p.get_span(db).start_byte,
                        )
                    };
                    let mut others: Vec<_> = all
                        .iter()
                        .filter(|o| {
                            o.path != entry.path
                                && o.location.is_some()
                                && o.instance == entry.instance
                                && o.members == entry.members
                        })
                        .collect();
                    if others
                        .iter()
                        .any(|o| position(&o.path) < position(&entry.path))
                    {
                        continue;
                    }
                    others.sort_by_key(|o| position(&o.path));
                    if !others.is_empty() {
                        Some(Refusal::LocatedTwice {
                            others: others
                                .iter()
                                .map(|o| crate::check::errors::e14_config::OtherLocation {
                                    path: o.path,
                                    address: o
                                        .location
                                        .as_ref()
                                        .map(|(dv, _)| {
                                            compact_str::CompactString::from(dv.to_address(db))
                                        })
                                        .unwrap_or_default(),
                                })
                                .collect(),
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
                        var: var.name_with_case(db),
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
    let var_name = var.name_with_case(db);
    let same_instance = |o: &ConfigEntry<'db>| o.instance == entry.instance;
    // One report per conflict, from the first entry in file order, pointing
    // at the others; the later ones give no value either.
    let position = |p: &PathExpr<'db>| {
        (
            p.get_scope_id(db).file(db).url(db).to_string(),
            p.get_span(db).start_byte,
        )
    };
    let mut others: Vec<_> = all
        .iter()
        .filter(|o| {
            o.path != entry.path
                && o.init.is_some()
                && same_instance(o)
                && (o.members.starts_with(&entry.members) || entry.members.starts_with(&o.members))
        })
        .collect();
    if !others.is_empty() {
        if others
            .iter()
            .all(|o| position(&o.path) > position(&entry.path))
        {
            others.sort_by_key(|o| position(&o.path));
            // Each other entry is labelled by what it names: `pump.k`.
            let others = others
                .into_iter()
                .map(|o| crate::check::errors::e14_config::OtherValue {
                    path: o.path,
                    name: o
                        .members
                        .iter()
                        .map(|m| m.name_with_case(db).text(db).as_str())
                        .collect::<Vec<_>>()
                        .join(".")
                        .into(),
                })
                .collect();
            errors.push(refuse(ConfigEntryRefusal::ValueTwice {
                var: var_name,
                others,
            }));
        }
        return None;
    }
    // A PROGRAM's VAR located in full is its channel, one cell for every
    // instance of the program.
    if var.is_program_located(db) {
        let program = match crate::hir_def::semantic_index::get_scope(db, var.get_scope_id(db)).kind
        {
            crate::hir_def::scope::ScopeKind::Program(p) => p.name_with_case(db),
            _ => Ident::new(db, compact_str::CompactString::default()),
        };
        errors.push(refuse(ConfigEntryRefusal::ChannelShared {
            var: var_name,
            address: var
                .location(db)
                .map(|dv| compact_str::CompactString::from(dv.to_address(db)))
                .unwrap_or_default(),
            program,
            decl: *var,
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
                owner_decl: crate::hir_ty::index_graphs::located_declaration(db, &view.owner),
                owner_mention: crate::hir_ty::index_graphs::located_mention(db, &view.owner),
                owner: view.owner.text,
                usage: WiderAddressUse::Initializer(owner),
            }
            .to_diagnostic(db, file),
        );
        return None;
    }
    if let Some(decl) = located_declaration(db, &channel) {
        errors.push(refuse(ConfigEntryRefusal::ChannelDeclared {
            var: var_name,
            address: channel.text.clone(),
            decl: Some(decl),
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
) -> Result<LocatedAddress, crate::check::errors::e14_config::ConfigLocationRefusal<'db>> {
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
            declared,
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
            let Some(program) = result.prog_instance.get(&instance.ident(db)) else {
                continue;
            };
            let mut paths = Vec::new();
            // Instance state only, as `instance_members` has it for a
            // function block: a VAR_TEMP is made per call (E1425 says so where
            // it is declared) and a VAR_EXTERNAL is a global's.
            crate::hir_ty::head::instances::collect_partly_located(
                db,
                program
                    .variables(db)
                    .iter()
                    .copied()
                    .filter(|v| !v.is_temp(db) && !v.is_external(db)),
                &mut paths,
            );
            for path in paths {
                let located = all.iter().any(|e| {
                    e.location.is_some() && e.instance == instance.ident(db) && e.members == path
                });
                if located {
                    continue;
                }
                let location = path.last().and_then(|v| v.location(db));
                let address = location.map(|dv| dv.to_address(db)).unwrap_or_default();
                // The entry to write, from the variable itself: its path, a
                // complete address in its area as wide as its type, its type.
                let example = path.last().and_then(|v| {
                    let ty = Type::resolve_spec(db, v.spec(db));
                    let bits =
                        crate::hir_ty::head::checks::variables::located_width(ty.normalize(db))?;
                    let (size, offset) = match bits {
                        1 => ('X', "0.0"),
                        8 => ('B', "0"),
                        16 => ('W', "0"),
                        32 => ('D', "0"),
                        64 => ('L', "0"),
                        _ => return None,
                    };
                    let area = location?.area(db)?.prefix();
                    let members = path
                        .iter()
                        .map(|v| v.name_with_case(db).text(db).as_str())
                        .collect::<Vec<_>>()
                        .join(".");
                    Some(compact_str::CompactString::from(format!(
                        "{}.{}.{members} AT {area}{size}{offset} : {};",
                        resource.name(db).with_case.text(db),
                        instance.with_case.text(db),
                        ty.type_name(db)
                    )))
                });
                result.errors.push(
                    ConfigError::PartlyLocatedUnlocated(PartlyUnlocated::Missing {
                        instance,
                        path: path.iter().map(|v| v.name_with_case(db)).collect(),
                        address: compact_str::CompactString::from(address),
                        example,
                    })
                    .to_diagnostic(db, config.get_scope_id(db).file(db)),
                );
            }
        }
    }
}
