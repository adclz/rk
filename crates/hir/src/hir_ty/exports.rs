// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! The FUNCTIONs a component exports for a host to call: each `{export}`
//! FUNCTION, in one interface named after the project, under a WIT name.

use compact_str::CompactString;
use db::WorkspaceDataBase;

use crate::hir_def::pous::function::Function;
use crate::hir_def::pous::pragma::{is_wit_name, wit_name, wit_parameter_name};
use crate::{HasName, HasPragmas, HirNodeInfo};

/// The interface a component exports its `{export}` FUNCTIONs in: the
/// project's name, as a WIT name.
pub fn export_interface(db: &dyn WorkspaceDataBase) -> String {
    wit_name(&db::config_file::get_config(db).project.name)
}

/// Why an `{export}` FUNCTION has no WIT name.
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum ExportNameError {
    /// The name its pragma gives is not a WIT name.
    Given(CompactString, auto_lsp::tree_sitter::Range),
    /// One of its own names gives none: this namespace's or its own, as
    /// written.
    Derived(CompactString),
}

/// The WIT name a host calls `func` by: the one its pragma gives,
/// `{export 'read-coils'}`, or else its namespaces' names and its own, each
/// in lowercase with `_` written `-`, joined by `-`. `Motion.Scale_Axis` is
/// `motion-scale-axis`, and a library's FUNCTION starts with the library's
/// name.
pub fn export_wit_name<'db>(
    db: &'db dyn WorkspaceDataBase,
    func: Function<'db>,
) -> Result<String, ExportNameError> {
    if let Some((name, span)) = func.export_given_name(db) {
        return match is_wit_name(name) {
            true => Ok(name.to_string()),
            false => Err(ExportNameError::Given(name.clone(), *span)),
        };
    }
    let namespace =
        crate::hir_ty::resolver::name::enclosing_namespace_path(db, func.get_scope_id(db));
    let mut words = Vec::new();
    for fragment in namespace
        .iter()
        .flat_map(|path| path.fragments(db).iter().copied())
        .chain(std::iter::once(func.get_name_with_case(db)))
    {
        let text = fragment.text(db);
        match wit_parameter_name(text) {
            Some(word) => words.push(word),
            None => return Err(ExportNameError::Derived(text.clone())),
        }
    }
    Ok(words.join("-"))
}

/// What the component model tells two names apart by: `name` without its
/// `-`. Two exports of one key are one to a host.
pub fn wit_key(name: &str) -> String {
    name.replace('-', "").to_ascii_lowercase()
}
