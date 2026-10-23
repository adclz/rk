// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

pub mod debug_symbols;
pub mod expr;
pub mod function;
pub mod located;
pub mod memory;
pub mod retain_map;
pub mod schedule;
pub mod stmt;
/// The `{test}` manifest lives in `debug_format`, the schema crate hosts read.
pub use debug_format::test_manifest;
pub mod types;

pub mod lower;

use hir::hir_def::interned::identifier::Ident;
use rustc_hash::FxHashMap;

use crate::debug_symbols::DebugSymbols;
use crate::function::{MirExternFunction, MirFunction};
use crate::memory::MirMemoryLayout;
use crate::test_manifest::TestManifest;
use crate::types::MirType;

/// A fully lowered MIR module, ready for backend consumption.
#[derive(Debug, Clone)]
pub struct MirModule {
    /// All functions (including monomorphized copies and lowered methods).
    pub functions: Vec<MirFunction>,

    /// Extern function declarations (imports).
    pub extern_functions: Vec<MirExternFunction>,

    /// Instance type layouts (FB and Class types used in the module).
    pub instance_types: Vec<MirInstanceType>,

    /// Function name → function index (imports first, then locals).
    pub function_indices: FxHashMap<Ident, u32>,

    /// Type name → type index.
    pub type_indices: FxHashMap<Ident, u32>,

    /// Memory layout — fully resolved.
    pub memory_layout: MirMemoryLayout,

    /// String data section entries: (offset, bytes), a literal's `id` its
    /// index.
    pub string_data: Vec<(u32, Vec<u8>)>,

    /// Test manifest: metadata about test functions.
    pub test_manifest: TestManifest,

    /// Start of the contiguous RETAIN band the host snapshots and restores;
    /// `retain_size == 0` without `RETAIN` variables.
    pub retain_base: u32,
    pub retain_size: u32,

    /// Start and byte length of the contiguous GLOBALS band: every configuration
    /// `VAR_GLOBAL` as one host-visible region. RETAIN globals sit in its
    /// overlap with the retain band.
    pub globals_base: u32,
    pub globals_size: u32,

    /// The three located (`AT %…`) bands: `%I` written by the host before a
    /// scan, `%Q` read back after it, `%M` the program's own marker area.
    /// Each is contiguous, so a host copies one range per direction; all
    /// three are zero-sized when the workspace declares no located variable.
    pub input_base: u32,
    pub input_size: u32,
    pub output_base: u32,
    pub output_size: u32,
    pub marker_base: u32,
    pub marker_size: u32,

    /// Resolved task schedule of the module's CONFIGURATION; `None` without one.
    pub schedule: Option<schedule::MirSchedule>,

    /// The schedule as the module carries it (the `rk.schedule` section),
    /// built once instance addresses are final.
    pub schedule_manifest: Option<debug_format::ScheduleManifest>,

    /// Debug-symbol table: every debuggable variable at its absolute address,
    /// emitted as the `debug-symbols` section.
    pub debug_symbols: DebugSymbols,

    /// Per-field RETAIN map: which byte ranges of the retain band persist
    /// (see `retain_map`), emitted as the `retain-map` section.
    pub retain_map: debug_format::RetainMap,

    /// Which located (`AT %…`) variable sits where inside the three bands,
    /// emitted as the `located-map` section. Empty when the workspace
    /// declares none, and the section is then not emitted at all.
    pub located_map: debug_format::LocatedMap,

    /// Source file URLs, indexed by `MirSourceLocation::file_id` and emitted
    /// as `DebugLines::files`.
    pub source_files: Vec<String>,

    /// The size of the stack recursive calls push their frames on, when
    /// `config.toml` sets one (`stack_size`). `None` gives the stack the
    /// largest frame and 64 KiB more. Lowering leaves it unset: the build
    /// reads the setting, as it does the optimization level.
    pub stack_size: Option<u64>,
}

/// Instance type layout for a FUNCTION_BLOCK or CLASS.
/// Forms a recursive tree via `nested_instance` on fields.
#[derive(Debug, Clone)]
pub struct MirInstanceType {
    pub name: Ident,
    pub fields: Vec<MirInstanceField>,
    pub size: u32,
    pub align: u32,
}

/// A field within an instance type (FB/Class).
#[derive(Debug, Clone)]
pub struct MirInstanceField {
    pub name: Ident,
    pub ty: MirType,
    /// Byte offset within the instance.
    pub offset: u32,
    /// The instance type name when this field is itself an FB or class.
    pub nested_instance: Option<Ident>,
    /// Pre-computed initialization for this field.
    pub init: Option<MirFieldInit>,
}

/// Flat initialization data for an instance field.
#[derive(Debug, Clone)]
pub struct MirFieldInit {
    /// `(relative offset, value)` stores to run at initialization, nested FBs
    /// flattened.
    pub stores: Vec<MirInitStore>,
}

/// A single initialization store operation.
#[derive(Debug, Clone)]
pub struct MirInitStore {
    /// Relative offset from the field's base address.
    pub offset: u32,
    pub value: expr::MirConstant,
}

/// The strongly connected component of each node of `edges`, Kosaraju's
/// way: the order in which a search of the calls leaves each node, then a
/// search of the calls reversed, from the last node left: each search of
/// the second pass stays in one component. Both searches keep their own
/// stack, for a chain of thousands of calls.
fn components(edges: &[Vec<usize>]) -> Vec<usize> {
    let mut left = Vec::with_capacity(edges.len());
    let mut seen = vec![false; edges.len()];
    for start in 0..edges.len() {
        if std::mem::replace(&mut seen[start], true) {
            continue;
        }
        let mut path = vec![(start, 0)];
        while let Some((at, next)) = path.last_mut() {
            let at = *at;
            if let Some(&callee) = edges[at].get(*next) {
                *next += 1;
                if !std::mem::replace(&mut seen[callee], true) {
                    path.push((callee, 0));
                }
            } else {
                left.push(at);
                path.pop();
            }
        }
    }
    let mut callers = vec![Vec::new(); edges.len()];
    for (caller, callees) in edges.iter().enumerate() {
        for &callee in callees {
            callers[callee].push(caller);
        }
    }
    let mut component = vec![usize::MAX; edges.len()];
    let mut next_component = 0;
    for &start in left.iter().rev() {
        if component[start] != usize::MAX {
            continue;
        }
        component[start] = next_component;
        let mut pending = vec![start];
        while let Some(at) = pending.pop() {
            for &caller in &callers[at] {
                if component[caller] == usize::MAX {
                    component[caller] = next_component;
                    pending.push(caller);
                }
            }
        }
        next_component += 1;
    }
    component
}

impl MirModule {
    /// Look up an instance type by name.
    pub fn find_instance_type(&self, name: Ident) -> Option<&MirInstanceType> {
        self.instance_types.iter().find(|it| it.name == name)
    }

    /// A function that can call itself again, following the calls lowering
    /// emitted, but has no frame. HIR's call graph decides which functions
    /// get one, and a call it missed would leave a recursive function's
    /// storage shared between its calls: lowering refuses such a module
    /// (`LowerTypeError::UnframedRecursion`). Kosaraju's algorithm finds the
    /// cycles, where HIR's graph uses Tarjan's: a mistake in one is not made
    /// again by the other.
    pub(crate) fn unframed_recursion(&self) -> Option<&MirFunction> {
        let index: FxHashMap<Ident, usize> = self
            .functions
            .iter()
            .enumerate()
            .map(|(i, func)| (func.name, i))
            .collect();
        let edges: Vec<Vec<usize>> = self
            .functions
            .iter()
            .map(|func| {
                let mut names = Vec::new();
                stmt::callees(&func.body, &mut names);
                names
                    .iter()
                    .filter_map(|name| index.get(name).copied())
                    .collect()
            })
            .collect();
        let component = components(&edges);
        let mut size = vec![0usize; edges.len()];
        for &c in &component {
            size[c] += 1;
        }
        self.functions
            .iter()
            .enumerate()
            .find(|(at, func)| {
                let recursive = size[component[*at]] > 1 || edges[*at].contains(at);
                recursive && func.frame.is_none()
            })
            .map(|(_, func)| func)
    }
}

#[cfg(test)]
mod tests {
    use super::components;

    /// Whether each node reaches each other, through one edge or more.
    fn reaches(edges: &[Vec<usize>]) -> Vec<Vec<bool>> {
        (0..edges.len())
            .map(|start| {
                let mut seen = vec![false; edges.len()];
                let mut pending = edges[start].clone();
                while let Some(at) = pending.pop() {
                    if !std::mem::replace(&mut seen[at], true) {
                        pending.extend(&edges[at]);
                    }
                }
                seen
            })
            .collect()
    }

    /// Random graphs, self edges included: two nodes share a component
    /// exactly when each reaches the other.
    #[test]
    fn a_component_is_the_nodes_that_reach_each_other() {
        let mut seed: u64 = 0x9e37_79b9_7f4a_7c15;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        for size in [1, 2, 5, 12, 30] {
            for per_mille in [50, 150, 400] {
                for _ in 0..20 {
                    let edges: Vec<Vec<usize>> = (0..size)
                        .map(|_| (0..size).filter(|_| next() % 1000 < per_mille).collect())
                        .collect();
                    let component = components(&edges);
                    let reach = reaches(&edges);
                    for a in 0..size {
                        for b in 0..size {
                            assert_eq!(
                                component[a] == component[b],
                                a == b || (reach[a][b] && reach[b][a]),
                                "{a} and {b} in {edges:?}"
                            );
                        }
                    }
                }
            }
        }
    }

    /// The search keeps its own stack: a cycle through a million nodes,
    /// searched on a thread of 64 KiB, is one component.
    #[test]
    fn a_cycle_of_a_million_nodes_is_one_component() {
        let edges: Vec<Vec<usize>> = (0..1_000_000)
            .map(|at| vec![(at + 1) % 1_000_000])
            .collect();
        let component = std::thread::Builder::new()
            .stack_size(64 * 1024)
            .spawn(move || components(&edges))
            .unwrap()
            .join()
            .unwrap();
        assert!(component.iter().all(|c| *c == component[0]));
    }
}
