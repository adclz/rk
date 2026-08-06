//! The resolved scheduling model for a module's CONFIGURATION: which TASKs
//! exist, how often each fires, and which PROGRAM *instances* each runs. Built
//! from HIR (reusing `infer_config_result`'s resolved instance/task maps) and
//! consumed by codegen (per-task entry points + scheduler globals) and by the
//! runtime to drive the scan cycle.
//!
//! A PROGRAM is compiled like a FUNCTION_BLOCK (struct + `this`-body), so each
//! program configuration `PROGRAM inst WITH task : Type` allocates its own
//! instance of the program's struct and runs `Type$__body__(&inst)`. Only
//! cyclic (INTERVAL) tasks are scheduled; SINGLE/event tasks are deferred.

use db::WorkspaceDataBase;
use hir::{
    Qualifier,
    hir_def::{
        config::{ConfigDecl, TaskConfig},
        interned::identifier::Ident,
        pous::variable::VariableKind,
        program::ProgramDecl,
    },
    hir_ty::{config::infer_config_result, infer::Infer, ty::Type},
};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{memory::MirAllocKind, memory::MirMemoryLayout, types::MirStructType};

/// What `lower_module` hands the scheduler about each lowered PROGRAM type.
#[derive(Debug, Clone)]
pub struct ProgramInfo<'db> {
    /// The `Type$__body__` function to call on an instance.
    pub body_fn: Ident,
    /// The program's instance struct layout.
    pub struct_type: MirStructType,
    /// The HIR declaration (for per-field RETAIN/qualifier lookup).
    pub decl: ProgramDecl<'db>,
}

/// A configured program instance a task runs.
#[derive(Debug, Clone)]
pub struct MirProgInstance {
    /// The configuration instance name (e.g. `Main` in `PROGRAM Main WITH …`).
    /// The root segment of this instance's variables' debug-symbol paths.
    pub inst_name: Ident,
    /// The program TYPE's name (key into the module's program info — used to
    /// find the instance's field layout + initializers).
    pub prog_name: Ident,
    /// The `Type$__body__` function to call.
    pub body_fn: Ident,
    /// Base address of this instance's state in linear memory.
    pub instance_addr: u32,
    /// Config-level retain qualifier (`PROGRAM RETAIN p` = `Some(true)`,
    /// `NON_RETAIN` = `Some(false)`).
    pub config_retain: Option<bool>,
}

/// One cyclic TASK and the program instances it runs each time it fires.
#[derive(Debug, Clone)]
pub struct MirTask {
    pub name: Ident,
    /// How often the task fires, as a count of base ticks
    /// (`interval / common_ticktime`). Always >= 1.
    pub period_ticks: u32,
    /// IEC priority (lower number = more urgent). `None` when PRIORITY is omitted.
    pub priority: Option<u32>,
    /// Program instances this task runs, in declaration order.
    pub programs: Vec<MirProgInstance>,
}

/// The resolved schedule of the module's single CONFIGURATION.
#[derive(Debug, Clone)]
pub struct MirSchedule {
    /// Base tick period in nanoseconds — the GCD of all task intervals.
    pub common_ticktime_ns: u64,
    /// Tasks, sorted most-urgent-first (lowest priority number).
    pub tasks: Vec<MirTask>,
}

/// Lower a module's CONFIGURATION (if any) into a schedule, allocating each
/// program instance's state in `memory_layout` and registering its RETAIN
/// fields. IEC allows exactly one configuration; we take the first. Returns
/// `None` when there is no configuration or it schedules no cyclic tasks.
pub fn lower_schedule<'db>(
    db: &'db dyn WorkspaceDataBase,
    configs: &[ConfigDecl<'db>],
    memory_layout: &mut MirMemoryLayout,
    program_infos: &FxHashMap<Ident, ProgramInfo<'db>>,
) -> Option<MirSchedule> {
    let config = *configs.first()?;
    let inferred = infer_config_result(db, config);

    // Every program instance, across all resources.
    let mut prog_configs = Vec::new();
    for r in config.resources(db).iter() {
        prog_configs.extend(r.programs(db).iter().copied());
    }

    // Allocate one instance per ProgConfig and group under its WITH-task,
    // preserving declaration order.
    let mut grouped: Vec<(TaskConfig<'db>, Vec<MirProgInstance>)> = Vec::new();
    for p in &prog_configs {
        let Some(task) = inferred.task_of_prog.get(p) else {
            continue; // PROGRAM with no resolved WITH <task> — unscheduled.
        };
        let Some(prog_decl) = inferred.prog_instance.get(&p.name(db).ident) else {
            continue; // program type didn't resolve (a diagnostic was emitted).
        };
        let Some(info) = program_infos.get(&prog_decl.name(db)) else {
            continue;
        };

        // Allocate this instance's state, and register its RETAIN fields for
        // the host-snapshottable band.
        let base = memory_layout.allocate(
            p.name(db).ident,
            info.struct_type.size,
            info.struct_type.align,
            MirAllocKind::InstanceData,
        );
        // If the program has ANY RETAIN state, persist its whole instance. The
        // body addresses fields via `this + offset`, so individual fields can't
        // be relocated into the band out from under it — we relocate the whole
        // instance instead (its base becomes a band address). The cost: a retain
        // program's non-RETAIN fields are persisted too — a simplification vs.
        // strict per-field RETAIN (a C-emitting compiler copies each retained field in/out).
        //
        // A config-level qualifier (`PROGRAM RETAIN p WITH t : Type` /
        // `PROGRAM NON_RETAIN ...`, IEC program configuration) overrides the
        // declaration-driven decision entirely: RETAIN persists the instance
        // even without retained fields, NON_RETAIN suppresses persistence even
        // with them. Otherwise a field is retained if it is RETAIN-qualified
        // itself OR its type (an FB/class instance, possibly nested) declares
        // `VAR RETAIN` state internally — other toolchains semantics: FB-internal
        // RETAIN persists for every instance.
        let has_retain = match p.retain(db) {
            Some(config_qualifier) => config_qualifier,
            None => info
                .struct_type
                .fields
                .iter()
                .any(|f| is_retain_field(db, &info.decl, f.name)),
        };
        if has_retain {
            memory_layout.record_retain(
                p.name(db).ident,
                base,
                info.struct_type.size,
                info.struct_type.align,
            );
        }

        let instance = MirProgInstance {
            inst_name: p.name(db).ident,
            prog_name: prog_decl.name(db),
            body_fn: info.body_fn,
            instance_addr: base,
            config_retain: p.retain(db),
        };
        match grouped.iter_mut().find(|(t, _)| *t == *task) {
            Some((_, v)) => v.push(instance),
            None => grouped.push((*task, vec![instance])),
        }
    }

    // Resolve intervals (cyclic tasks only) and keep them for the GCD pass.
    struct Pending {
        name: Ident,
        interval_ns: u64,
        priority: Option<u32>,
        programs: Vec<MirProgInstance>,
    }
    let mut pending = Vec::new();
    for (task, programs) in grouped {
        // HIR decided which tasks are schedulable and reported E0239 for the
        // rest, so a task missing from this map has already been diagnosed —
        // MIR neither re-parses the interval nor re-derives the reason.
        let Some(&interval_ns) = inferred.task_interval_ns.get(&task) else {
            continue;
        };
        if interval_ns == 0 {
            continue;
        }
        pending.push(Pending {
            name: task.name(db).ident,
            interval_ns,
            priority: inferred.task_priority.get(&task).copied(),
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

/// Whether a program field is persistent: `RETAIN` itself, or an FB/class
/// instance (arrays included) whose type declares `VAR RETAIN` anywhere
/// in its nesting. An explicit `NON_RETAIN` prunes the subtree, internal
/// `RETAIN` included.
fn is_retain_field<'db>(
    db: &'db dyn WorkspaceDataBase,
    prog: &ProgramDecl<'db>,
    name: Ident,
) -> bool {
    prog.variables(db).iter().any(|v| {
        v.name(db) == name
            && v.kind(db) != VariableKind::Temp
            && !v.qualifier(db).contains(Qualifier::NON_RETAIN)
            && (v.qualifier(db).contains(Qualifier::RETAIN)
                || type_has_retain(db, v.spec(db).infer(db), &mut FxHashSet::default()))
    })
}

/// Does this type (an FB/class instance, or an array of them) declare RETAIN
/// state anywhere in its nesting? `visited` guards against type cycles;
/// NON_RETAIN members prune their subtree (see `is_retain_field`).
fn type_has_retain<'db>(
    db: &'db dyn WorkspaceDataBase,
    ty: Type<'db>,
    visited: &mut FxHashSet<Ident>,
) -> bool {
    let vars = match ty.normalize(db) {
        Type::FunctionBlock(fb) => {
            if !visited.insert(fb.name(db)) {
                return false;
            }
            fb.variables(db)
        }
        Type::Class(class) => {
            if !visited.insert(class.name(db)) {
                return false;
            }
            class.variables(db)
        }
        Type::Array(arr) => {
            return type_has_retain(db, arr.of_type(db).infer(db), visited);
        }
        _ => return false,
    };
    vars.iter().any(|v| {
        v.kind(db) != VariableKind::Temp
            && !v.qualifier(db).contains(Qualifier::NON_RETAIN)
            && (v.qualifier(db).contains(Qualifier::RETAIN)
                || type_has_retain(db, v.spec(db).infer(db), visited))
    })
}


fn gcd(a: u64, b: u64) -> u64 {
    if b == 0 { a } else { gcd(b, a % b) }
}
