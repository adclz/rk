//! The resolved scheduling model for a module's CONFIGURATION: which TASKs
//! exist, how often each fires, and which PROGRAMs each runs. Built from HIR
//! (reusing `infer_config_result`'s resolved instance/task maps) and consumed
//! by codegen (per-task entry points + a task table) and by the runtime to
//! drive the scan cycle.
//!
//! Phase 1 is cooperative and treats each PROGRAM type as a singleton; only
//! cyclic (INTERVAL) tasks are scheduled. SINGLE/event tasks and per-instance
//! program state are later phases.

use db::WorkspaceDataBase;
use hir::{
    hir_def::{
        config::{ConfigDecl, ConfigResource, DataSource, TaskConfig},
        expressions::expression::{Elementary, ExprKind, PrimaryExpr},
        interned::identifier::Ident,
    },
    hir_ty::{config::infer_config_result, ty::Type},
};

/// One cyclic TASK and the program bodies it runs each time it fires.
#[derive(Debug, Clone)]
pub struct MirTask {
    pub name: Ident,
    /// How often the task fires, as a count of base ticks
    /// (`interval / common_ticktime`). Always >= 1.
    pub period_ticks: u32,
    /// IEC priority (lower number = more urgent). Used for deterministic
    /// cooperative ordering within a tick. `None` when PRIORITY is omitted.
    pub priority: Option<u32>,
    /// MIR function names of the program bodies this task runs, in declaration
    /// order. Phase 1: each program type is a singleton.
    pub programs: Vec<Ident>,
}

/// The resolved schedule of the module's single CONFIGURATION.
#[derive(Debug, Clone)]
pub struct MirSchedule {
    /// Base tick period in nanoseconds — the GCD of all task intervals. The
    /// host advances one `tick` every `common_ticktime_ns`.
    pub common_ticktime_ns: u64,
    /// Tasks, sorted most-urgent-first (lowest priority number) for
    /// deterministic dispatch.
    pub tasks: Vec<MirTask>,
}

/// Lower a module's CONFIGURATION (if any) into a schedule. IEC allows exactly
/// one configuration; we take the first. Returns `None` when there is no
/// configuration or it schedules no cyclic tasks.
pub fn lower_schedule<'db>(
    db: &'db dyn WorkspaceDataBase,
    configs: &[ConfigDecl<'db>],
) -> Option<MirSchedule> {
    let config = *configs.first()?;
    let inferred = infer_config_result(db, config);

    // Every program instance — bare config-level + inside resources.
    let mut prog_configs = Vec::new();
    for res in config.resources(db) {
        match res {
            ConfigResource::Program(p) => prog_configs.push(*p),
            ConfigResource::Resource(r) => prog_configs.extend(r.programs(db).iter().copied()),
            ConfigResource::Task(_) => {}
        }
    }

    // Group resolved program bodies under their WITH-task, preserving the
    // declaration order of both tasks and their programs.
    let mut grouped: Vec<(TaskConfig<'db>, Vec<Ident>)> = Vec::new();
    for p in &prog_configs {
        let Some(task) = inferred.task_of_prog.get(p) else {
            continue; // PROGRAM with no resolved WITH <task> — unscheduled.
        };
        let Some(prog) = inferred.prog_instance.get(&p.name(db).ident) else {
            continue; // program type didn't resolve (a diagnostic was emitted).
        };
        let body = crate::lower::monomorphize::qualified_pou_ident(db, Type::Program(*prog));
        match grouped.iter_mut().find(|(t, _)| *t == *task) {
            Some((_, progs)) => progs.push(body),
            None => grouped.push((*task, vec![body])),
        }
    }

    // Resolve intervals (cyclic tasks only) and keep them for the GCD pass.
    struct Pending {
        name: Ident,
        interval_ns: u64,
        priority: Option<u32>,
        programs: Vec<Ident>,
    }
    let mut pending = Vec::new();
    for (task, programs) in grouped {
        let Some(ds) = task.interval(db) else {
            continue; // SINGLE/event tasks deferred.
        };
        let Some(interval_ns) = interval_nanos(db, &ds) else {
            continue;
        };
        if interval_ns == 0 {
            continue;
        }
        pending.push(Pending {
            name: task.name(db).ident,
            interval_ns,
            priority: task_priority(db, &task),
            programs,
        });
    }

    if pending.is_empty() {
        return None;
    }

    // Base period = GCD of all task intervals.
    let common = pending.iter().map(|p| p.interval_ns).reduce(gcd)?;
    if common == 0 {
        return None;
    }

    let mut tasks: Vec<MirTask> = pending
        .into_iter()
        .map(|p| MirTask {
            name: p.name,
            period_ticks: (p.interval_ns / common) as u32,
            priority: p.priority,
            programs: p.programs,
        })
        .collect();

    // Deterministic dispatch order: most urgent first; no-priority tasks last;
    // ties keep declaration order (stable sort).
    tasks.sort_by_key(|t| t.priority.unwrap_or(u32::MAX));

    Some(MirSchedule {
        common_ticktime_ns: common,
        tasks,
    })
}

/// Extract a duration in nanoseconds from a task's INTERVAL data source.
/// Handles the constant TIME/LTIME literal case (the only form used in
/// practice for intervals).
fn interval_nanos<'db>(db: &'db dyn WorkspaceDataBase, ds: &DataSource<'db>) -> Option<u64> {
    let DataSource::Constant(expr) = ds else {
        return None;
    };
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

fn task_priority<'db>(db: &'db dyn WorkspaceDataBase, task: &TaskConfig<'db>) -> Option<u32> {
    task.priority(db)?.text(db).parse().ok()
}

fn gcd(a: u64, b: u64) -> u64 {
    if b == 0 { a } else { gcd(b, a % b) }
}
