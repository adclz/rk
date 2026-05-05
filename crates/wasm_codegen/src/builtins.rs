//! Math intrinsics pulled from `crates/wasm_builtins`, embedded at build
//! time. The codegen looks up a function by its dotted IEC name (e.g.
//! `f32.sin`), then grafts the function's body — plus every helper it
//! transitively calls — into the output module on demand.
//!
//! Prototype status: the lookup + transitive-closure helpers are wired up,
//! but the call-relocation / dedup graft into the output module is not yet
//! implemented — that's the next PR.

#![allow(dead_code)]

pub(crate) use wasm_builtins_generated::{
    BUILTIN_DATA, BUILTIN_FUNCS, BUILTIN_GLOBALS, BUILTIN_NAMES, BUILTIN_SIGS, BUNDLE_MEMORY_TOP,
    BuiltinCallSite, BuiltinFunc, BuiltinGlobal, BuiltinSig,
};

/// Look up a builtin by its dotted name; real WASM instructions take
/// priority in `emit_stmt`.
pub(crate) fn lookup(name: &str) -> Option<u32> {
    BUILTIN_NAMES.get(name).copied()
}

/// Walk the call graph from `root_idx`, collecting every function index
/// the root transitively depends on, including itself. Order is
/// post-order so callees appear before callers — useful when grafting
/// into an output module that needs forward-declared indices.
pub(crate) fn transitive_closure(root_idx: u32) -> Vec<u32> {
    use rustc_hash::FxHashSet;
    let mut seen = FxHashSet::default();
    let mut order = Vec::new();
    fn walk(
        idx: u32,
        seen: &mut rustc_hash::FxHashSet<u32>,
        order: &mut Vec<u32>,
    ) {
        if !seen.insert(idx) {
            return;
        }
        if let Some(f) = BUILTIN_FUNCS.get(idx as usize) {
            for cs in f.call_sites {
                walk(cs.target, seen, order);
            }
        }
        order.push(idx);
    }
    walk(root_idx, &mut seen, &mut order);
    order
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_iec_math_name_resolves() {
        // build.rs compiled the bundle and emitted a phf entry for every export.
        for name in [
            "f32.sin", "f32.cos", "f32.tan", "f32.asin", "f32.acos", "f32.atan",
            "f32.atan2", "f32.exp", "f32.ln", "f32.log",
            "f64.sin", "f64.cos", "f64.tan", "f64.asin", "f64.acos", "f64.atan",
            "f64.atan2", "f64.exp", "f64.ln", "f64.log",
        ] {
            assert!(
                lookup(name).is_some(),
                "builtin {name} missing from BUILTIN_NAMES"
            );
        }
    }

    #[test]
    fn transitive_closure_post_orders_dependencies() {
        // sin pulls in libm helpers (k_sin, rem_pio2, …). The closure must
        // contain at least the root, and the root must be the last entry
        // (post-order — callees before callers).
        let root = lookup("f64.sin").expect("f64.sin");
        let order = transitive_closure(root);
        assert!(!order.is_empty());
        assert_eq!(*order.last().unwrap(), root);
    }

    #[test]
    #[ignore = "informational — run with --nocapture to see footprint"]
    fn print_closure_sizes() {
        let names = [
            "f32.sin", "f32.cos", "f32.tan", "f32.exp", "f32.ln",
            "f64.sin", "f64.cos", "f64.tan", "f64.exp", "f64.ln",
        ];
        for name in names {
            let root = lookup(name).unwrap();
            let order = transitive_closure(root);
            let bytes: usize = order
                .iter()
                .map(|&i| BUILTIN_FUNCS[i as usize].body.len())
                .sum();
            eprintln!("{name}: {} fns, {} body bytes", order.len(), bytes);
        }
        let combined: rustc_hash::FxHashSet<u32> = names
            .iter()
            .flat_map(|n| transitive_closure(lookup(n).unwrap()))
            .collect();
        let combined_bytes: usize = combined
            .iter()
            .map(|&i| BUILTIN_FUNCS[i as usize].body.len())
            .sum();
        eprintln!(
            "all 10 combined: {} unique fns, {} body bytes",
            combined.len(),
            combined_bytes
        );
    }
}
