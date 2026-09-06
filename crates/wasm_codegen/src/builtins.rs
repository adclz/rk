//! Math intrinsics from `crates/wasm_builtins`, embedded at build time: a
//! function is looked up by its dotted IEC name (`f32.sin`) and grafted,
//! with every helper it transitively calls, into the output module.

#![allow(dead_code)]

pub(crate) use wasm_builtins_generated::{
    BUILTIN_DATA, BUILTIN_FUNCS, BUILTIN_GLOBALS, BUILTIN_IMPORTS, BUILTIN_NAMES, BUILTIN_SIGS,
    BUNDLE_MEMORY_TOP, BuiltinCallSite, BuiltinFunc, BuiltinSig, N_IMPORTS,
};

/// Look up a builtin by its dotted name; real WASM instructions take
/// priority in `emit_stmt`.
pub(crate) fn lookup(name: &str) -> Option<u32> {
    BUILTIN_NAMES.get(name).copied()
}

/// Every function index `root_idx` transitively depends on, itself
/// included, in post-order (callees before callers). Indices are positions
/// into `BUILTIN_FUNCS`; reachable imports are reported by
/// [`closure_uses_import`].
pub(crate) fn transitive_closure(root_idx: u32) -> Vec<u32> {
    use rustc_hash::FxHashSet;
    let mut seen = FxHashSet::default();
    let mut order = Vec::new();
    fn walk(idx: u32, seen: &mut rustc_hash::FxHashSet<u32>, order: &mut Vec<u32>) {
        if !seen.insert(idx) {
            return;
        }
        if let Some(f) = BUILTIN_FUNCS.get(idx as usize) {
            for cs in f.call_sites {
                // Imports have no body in the bundle; the graft synthesizes them.
                if cs.target < N_IMPORTS {
                    continue;
                }
                walk(cs.target - N_IMPORTS, seen, order);
            }
        }
        order.push(idx);
    }
    walk(root_idx, &mut seen, &mut order);
    order
}

/// Whether any function in `closure` calls the import `import_name` (the
/// graft then synthesizes `__iec_raise`).
pub(crate) fn closure_uses_import(closure: &[u32], import_name: &str) -> bool {
    let Some(imp_idx) = BUILTIN_IMPORTS
        .iter()
        .position(|imp| imp.name == import_name)
    else {
        return false;
    };
    let wasm_idx = imp_idx as u32; // imports occupy 0..N_IMPORTS
    closure.iter().any(|&fn_idx| {
        BUILTIN_FUNCS
            .get(fn_idx as usize)
            .map(|f| f.call_sites.iter().any(|cs| cs.target == wasm_idx))
            .unwrap_or(false)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_iec_math_name_resolves() {
        // build.rs compiled the bundle and emitted a phf entry for every export.
        for name in [
            "f32.sin",
            "f32.cos",
            "f32.tan",
            "f32.asin",
            "f32.acos",
            "f32.atan",
            "f32.atan2",
            "f32.exp",
            "f32.ln",
            "f32.log",
            "f64.sin",
            "f64.cos",
            "f64.tan",
            "f64.asin",
            "f64.acos",
            "f64.atan",
            "f64.atan2",
            "f64.exp",
            "f64.ln",
            "f64.log",
        ] {
            assert!(
                lookup(name).is_some(),
                "builtin {name} missing from BUILTIN_NAMES"
            );
        }
    }

    #[test]
    fn transitive_closure_post_orders_dependencies() {
        // sin pulls in libm helpers; the root is the last entry (post-order).
        let root = lookup("f64.sin").expect("f64.sin");
        let order = transitive_closure(root);
        assert!(!order.is_empty());
        assert_eq!(*order.last().unwrap(), root);
    }

    #[test]
    #[ignore = "informational - run with --nocapture to see footprint"]
    fn print_closure_sizes() {
        let names = [
            "f32.sin", "f32.cos", "f32.tan", "f32.exp", "f32.ln", "f64.sin", "f64.cos", "f64.tan",
            "f64.exp", "f64.ln",
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


#[cfg(test)]
mod parity {
    /// Every grafted builtin must be a name `rk check` accepts — otherwise a
    /// working pragma draws E0248.
    #[test]
    fn every_builtin_name_is_known_to_the_check() {
        for name in wasm_builtins_generated::BUILTIN_NAMES.keys() {
            assert!(
                hir::check::wasm_instructions::known(name),
                "builtin `{name}` is emittable but the check refuses it"
            );
        }
    }

    /// And the check's builtin list must not invent names — a name the check
    /// accepts that nothing emits would fall to the emitter's unknown arm.
    #[test]
    fn every_checked_builtin_really_exists() {
        for (name, _, _) in hir::check::wasm_instructions::BUILTINS {
            assert!(
                wasm_builtins_generated::BUILTIN_NAMES.contains_key(name),
                "the check lists `{name}` but no builtin exports it"
            );
        }
    }

    /// The signature the check holds for each builtin is the one the bundle
    /// was compiled with: a pragma the check passes is one the module
    /// validator passes.
    #[test]
    fn every_checked_builtin_signature_is_the_bundles() {
        use hir::check::wasm_instructions::Lane;
        use wasm_encoder::ValType;
        let lane = |v: &ValType| match v {
            ValType::I32 => Lane::I32,
            ValType::I64 => Lane::I64,
            ValType::F32 => Lane::F32,
            ValType::F64 => Lane::F64,
            other => panic!("a builtin signature carries {other:?}"),
        };
        for (name, params, results) in hir::check::wasm_instructions::BUILTINS {
            let idx = wasm_builtins_generated::BUILTIN_NAMES[name] as usize;
            let sig = &wasm_builtins_generated::BUILTIN_SIGS
                [wasm_builtins_generated::BUILTIN_FUNCS[idx].sig_idx as usize];
            let bundle_params: Vec<Lane> = sig.params.iter().map(lane).collect();
            let bundle_results: Vec<Lane> = sig.results.iter().map(lane).collect();
            assert_eq!(&bundle_params[..], *params, "`{name}`: parameters");
            assert_eq!(&bundle_results[..], *results, "`{name}`: results");
        }
    }
}
