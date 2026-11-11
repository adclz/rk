// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! What the module `rk test` builds imports to say where it raises:
//! `rk:test/raise`'s `raised`, called just before each raise. The exception
//! then unwinds the calls in progress, so the host takes the call stack in
//! that call, and a test that fails on a raise failed at the last one.
//!
//! No other build imports it: a debug and a release build of a program
//! differ in their custom sections only, so that what a monitor or a
//! retain file holds of one holds of the other.

use compact_str::CompactString;
use db::WorkspaceDataBase;
use hir::hir_def::interned::identifier::Ident;

use crate::MirModule;
use crate::function::{MirExternFunction, MirLinkage};

/// The function's name in its interface.
pub const FUNCTION: &str = "raised";

/// The symbol the import is known by in the module, one no declaration can
/// spell.
const SYMBOL: &str = "$raised";

/// Adds the import to `module`, numbered after everything it has.
pub fn add(db: &dyn WorkspaceDataBase, module: &mut MirModule) {
    if hook(module).is_some() {
        return;
    }
    let index = module.functions.len() as u32 + module.extern_functions.len() as u32;
    let name = Ident::new(db, CompactString::from(SYMBOL));
    module.extern_functions.push(MirExternFunction {
        name,
        index,
        module: CompactString::from(debug_format::wit::TEST_INTERFACE),
        import_name: CompactString::from(FUNCTION),
        params: Vec::new(),
        out_results: Vec::new(),
        return_type: None,
        linkage: MirLinkage::Internal,
    });
    module.function_indices.insert(name, index);
}

/// The import, when `module` was built for `rk test`.
pub fn hook(module: &MirModule) -> Option<&MirExternFunction> {
    module
        .extern_functions
        .iter()
        .find(|ext| ext.module == debug_format::wit::TEST_INTERFACE && ext.import_name == FUNCTION)
}
