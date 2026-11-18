// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! What a module keeps: the functions its roots reach, and the imports
//! those call.
//!
//! The roots are the functions lowering marks for export: `__init`, the
//! PROGRAM bodies and their scans, the bodies of the function blocks a task
//! runs, the `{export}` FUNCTIONs and the tests, which are roots in both
//! profiles though codegen exports them in a debug build only (a release
//! build leaves them to Binaryen). A call names its callee, since lowering
//! has resolved every dispatch: a method is copied for each POU that owns
//! it, and an interface, only ever a parameter, is specialized at each call
//! for the implementer bound to it. So what the roots reach is a walk over
//! the calls, and a debug build imports what its code calls, as a release
//! build does. A dispatch
//! at run time, should one come, would reach every function its slot may
//! hold, the overrides or the implementers, as HIR's call graph counts
//! them: its call would list them for [`crate::stmt::callees`].

use hir::hir_def::interned::identifier::Ident;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::MirModule;
use crate::function::MirLinkage;

impl MirModule {
    /// Keep the functions the roots reach and the imports those call;
    /// every other function and import is dropped, and what is kept is
    /// numbered again from zero, imports first, so that a function added
    /// after, numbered by the count of both, takes a number of its own. The
    /// memory layout stays as lowering laid it out, a dropped FUNCTION's
    /// variables included, so both profiles keep the same addresses.
    pub fn keep_reached(&mut self) {
        let reached = self.reached();
        self.functions.retain(|func| reached.contains(&func.name));
        let kept: Vec<bool> = self
            .extern_functions
            .iter()
            .map(|ext| reached.contains(&ext.name))
            .collect();
        // The longest list each import keeps, by its place among them: a
        // dropped caller may have been the longest, so what is kept is a
        // bound, as the largest string is.
        self.longest_lists = std::mem::take(&mut self.longest_lists)
            .into_iter()
            .zip(&kept)
            .filter_map(|(length, keep)| keep.then_some(length))
            .collect();
        let mut keep = kept.iter();
        self.extern_functions
            .retain(|_| keep.next().copied().unwrap_or(false));

        let mut renumbered: FxHashMap<u32, u32> = FxHashMap::default();
        let imports = self.extern_functions.iter_mut().map(|ext| &mut ext.index);
        let locals = self.functions.iter_mut().map(|func| &mut func.index);
        for (index, old) in imports.chain(locals).enumerate() {
            let index = index as u32;
            renumbered.insert(*old, index);
            *old = index;
        }
        self.function_indices = std::mem::take(&mut self.function_indices)
            .into_iter()
            .filter_map(|(name, old)| Some((name, *renumbered.get(&old)?)))
            .collect();
    }

    /// The names of the functions and imports the roots reach.
    fn reached(&self) -> FxHashSet<Ident> {
        let bodies: FxHashMap<Ident, usize> = self
            .functions
            .iter()
            .enumerate()
            .map(|(at, func)| (func.name, at))
            .collect();
        let mut next: Vec<Ident> = self
            .functions
            .iter()
            .filter(|func| func.linkage == MirLinkage::Export)
            .map(|func| func.name)
            .collect();
        let mut reached = FxHashSet::default();
        let mut checks_a_call = false;
        while let Some(name) = next.pop() {
            if !reached.insert(name) {
                continue;
            }
            // An import calls nothing.
            let Some(&at) = bodies.get(&name) else {
                continue;
            };
            crate::stmt::callees(&self.functions[at].body, &mut next);
            // An async import's block calls its import in the body codegen
            // emits for it, and checks the call with the component model's
            // built-ins.
            if let Some(call) = self.async_calls.get(&name) {
                next.push(call.import);
                checks_a_call = true;
            }
        }
        if checks_a_call {
            reached.extend(
                self.extern_functions
                    .iter()
                    .filter(|ext| ext.module.as_str() == crate::async_call::ROOT)
                    .map(|ext| ext.name),
            );
        }
        reached
    }
}
