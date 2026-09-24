//! The resolved scheduling model of a module's CONFIGURATION: which TASKs
//! exist, how often each fires, which PROGRAM instances each runs; built
//! from HIR's `ResolvedSchedule` and carried as the `rk.schedule` manifest.
//! Each program configuration allocates its own instance and runs
//! `Type$__body__(&inst)`. Only cyclic (INTERVAL) tasks are scheduled.

use db::WorkspaceDataBase;
use hir::{
    Qualifier,
    hir_def::{
        config::ConfigDecl,
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
    /// The RESOURCE that declares this task: the group a deployment binds to
    /// an execution unit.
    pub resource: Ident,

    pub name: Ident,
    /// How often the task fires, as a count of base ticks
    /// (`interval / common_ticktime`). Always >= 1.
    pub period_ticks: u64,
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

impl MirSchedule {
    /// The manifest the module carries: a serialization of what was decided,
    /// built after any retain relocation.
    pub fn to_manifest(&self, db: &dyn WorkspaceDataBase) -> debug_format::ScheduleManifest {
        debug_format::ScheduleManifest {
            version: debug_format::SCHEDULE_VERSION,
            common_ticktime_ns: self.common_ticktime_ns,
            tasks: self
                .tasks
                .iter()
                .map(|t| debug_format::TaskEntry {
                    name: t.name.text(db).to_string(),
                    resource: t.resource.text(db).to_string(),
                    period_ticks: t.period_ticks,
                    priority: t.priority,
                    programs: t
                        .programs
                        .iter()
                        .map(|p| debug_format::ProgramEntry {
                            instance: p.inst_name.text(db).to_string(),
                            export: p.body_fn.text(db).to_string(),
                            instance_addr: p.instance_addr,
                        })
                        .collect(),
                })
                .collect(),
        }
    }
}

/// Lower the CONFIGURATION (every fragment of it) into a schedule,
/// allocating each program instance and registering its RETAIN fields.
/// `None` without a cyclic task.
pub fn lower_schedule<'db>(
    db: &'db dyn WorkspaceDataBase,
    config: &[ConfigDecl<'db>],
    memory_layout: &mut MirMemoryLayout,
    program_infos: &FxHashMap<Ident, ProgramInfo<'db>>,
) -> Result<Option<MirSchedule>, crate::lower::lower_type::LowerTypeError> {
    // HIR resolved what runs; lowering gives each instance memory and
    // expresses periods against one tick counter.
    let mut pending: Vec<(Ident, &hir::hir_ty::config::ResolvedTask<'db>, Vec<MirProgInstance>)> =
        Vec::new();
    // Fragments contribute in file order; a RESOURCE cannot span two (E0115).
    for resource in config
        .iter()
        .flat_map(|c| infer_config_result(db, *c).schedule.resources.iter())
    {
        for task in &resource.tasks {
            let mut instances = Vec::new();
            for p in &task.programs {
                // HIR bound this instance to a program lowering registered; a miss is
                // the two disagreeing.
                let Some(info) = program_infos.get(&p.program.name(db)) else {
                    return Err(crate::lower::lower_type::LowerTypeError::UnsupportedType(
                        format!(
                            "configured program '{}' has no lowered body",
                            p.program.name(db).text(db)
                        ),
                    ));
                };

                // Allocate this instance's state, and register its RETAIN
                // fields for the host-snapshottable band. At least a byte, so
                // an instance holding nothing (every VAR located) still has
                // an address of its own for a debugger to tell it by.
                let base = memory_layout.allocate(
                    p.instance_name,
                    info.struct_type.size.max(1),
                    info.struct_type.align,
                    MirAllocKind::InstanceData,
                );
                // A program with any RETAIN state persists its whole instance: the body
                // addresses fields via `this + offset`, so the instance is relocated
                // whole. A config-level qualifier (`PROGRAM RETAIN p WITH t : Type`)
                // overrides the declaration-driven decision; otherwise a field is
                // retained when it is `RETAIN` itself or its FB type declares
                // `VAR RETAIN` state.
                let has_retain = match p.retain {
                    Some(config_qualifier) => config_qualifier,
                    None => info
                        .struct_type
                        .fields
                        .iter()
                        .any(|f| is_retain_field(db, &info.decl, f.name)),
                };
                if has_retain {
                    memory_layout.record_retain(
                        p.instance_name,
                        base,
                        info.struct_type.size,
                        info.struct_type.align,
                    );
                }

                instances.push(MirProgInstance {
                    inst_name: p.instance_name,
                    prog_name: p.program.name(db),
                    body_fn: info.body_fn,
                    instance_addr: base,
                    config_retain: p.retain,
                });
            }
            if !instances.is_empty() {
                pending.push((resource.name, task, instances));
            }
        }
    }

    if pending.is_empty() {
        return Ok(None);
    }

    // One tick counter drives every task, so the base period is the GCD of the
    // intervals and each task's period is its multiple of that.
    let Some(common) = pending.iter().map(|(_, t, _)| t.interval_ns).reduce(gcd) else {
        return Ok(None);
    };
    if common == 0 {
        return Ok(None);
    }

    let tasks: Vec<MirTask> = pending
        .into_iter()
        .map(|(resource, task, programs)| MirTask {
            resource,
            name: task.name,
            // As wide as the interval itself: a period never needs narrowing.
            period_ticks: task.interval_ns / common,
            priority: task.priority,
            programs,
        })
        .collect();

    Ok(Some(MirSchedule {
        common_ticktime_ns: common,
        tasks,
    }))
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

/// Does this type declare RETAIN state anywhere in its nesting? Members
/// come from [`instance_members`], the flattened `EXTENDS` view, so
/// inherited retain state counts; NON_RETAIN members prune their subtree.
fn type_has_retain<'db>(
    db: &'db dyn WorkspaceDataBase,
    ty: Type<'db>,
    visited: &mut FxHashSet<Ident>,
) -> bool {
    let pou = match ty.normalize(db) {
        Type::FunctionBlock(fb) => {
            if !visited.insert(fb.name(db)) {
                return false;
            }
            hir::hir_def::pous::pou::Pou::FunctionBlock(fb)
        }
        Type::Class(class) => {
            if !visited.insert(class.name(db)) {
                return false;
            }
            hir::hir_def::pous::pou::Pou::Class(class)
        }
        Type::Array(arr) => {
            return type_has_retain(db, arr.of_type(db).infer(db), visited);
        }
        _ => return false,
    };
    // `instance_members` already excludes VAR_TEMP and VAR_EXTERNAL — the
    // sections that are not instance state.
    hir::hir_ty::head::inheritance::instance_members(db, pou)
        .iter()
        .any(|m| {
            let v = m.var;
            !v.qualifier(db).contains(Qualifier::NON_RETAIN)
                && (v.qualifier(db).contains(Qualifier::RETAIN)
                    || type_has_retain(db, v.spec(db).infer(db), visited))
        })
}


fn gcd(a: u64, b: u64) -> u64 {
    if b == 0 { a } else { gcd(b, a % b) }
}
