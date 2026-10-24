// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! A library's root: the namespace its files are read in.
//!
//! A library's name, the `name` of its `config.toml`, is a namespace that
//! holds everything the library declares: `NAMESPACE Motion` in the library
//! `drives` is `Drives.Motion`, and a FUNCTION at the top of one of its files
//! is `Drives.F`. Two libraries then never clash, since two libraries never
//! share a name. The workspace's files are read at the top level, as
//! written.
//!
//! The root is nowhere in the files, so it is nowhere in the HIR: a
//! `NamespaceDecl` keeps the path its author wrote, and the lookups add the
//! root, through [`crate::hir_def::namespace::NamespaceDecl::absolute_path`]
//! and [`crate::hir_ty::resolver::name::enclosing_namespace_path`].

use auto_lsp::default::db::file::File;
use db::WorkspaceDataBase;
use db::workspace::Workspace;

use crate::hir_def::interned::identifier::Ident;
use crate::hir_def::interned::namespace::NamespacePath;

/// A library's root, computed once per library: its name as matched, as
/// written, and as a path of one fragment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Root {
    /// Case folded, as names are matched.
    pub name: Ident,
    /// As the library's `config.toml` writes it.
    pub spelling: Ident,
    /// `[name]`: the path the root's members are looked up under.
    pub path: NamespacePath,
}

/// Every library's root, in the order the libraries are read, which is
/// their [`db::dependencies::LibraryId`]: what an unqualified name is
/// looked up in after the imports. Memoized on the workspace input.
#[salsa::tracked(returns(ref))]
pub fn library_roots(db: &dyn WorkspaceDataBase) -> Vec<Root> {
    let Some(workspace) = Workspace::try_get(db) else {
        // Whether the singleton exists is not a read salsa records: say so,
        // or an answer given before the workspace was opened would stand.
        db.report_untracked_read();
        return Vec::new();
    };
    workspace
        .libraries(db)
        .iter()
        .map(|library| {
            let spelling = Ident::from_slice(db, &library.name);
            let name = root_name(db, spelling);
            Root {
                name,
                spelling,
                path: NamespacePath::new(db, vec![name]),
            }
        })
        .collect()
}

/// A library's name as a root is matched: the one place it is folded, since
/// it arrives as text from the library's `config.toml`, not as a declared
/// identifier with an accessor of its own.
fn root_name(db: &dyn WorkspaceDataBase, name: Ident) -> Ident {
    name.folded(db)
}

/// The root of `file`'s library, or `None` for a file of the workspace: an
/// index into [`library_roots`], which holds the memo.
pub fn library_root(db: &dyn WorkspaceDataBase, file: File) -> Option<Root> {
    let id = db.library_of(file)?;
    library_roots(db).get(id as usize).copied()
}

/// A namespace path of `file`, `written` as paths are matched and
/// `written_with_case` as the author spelled it, as the indexes key it and
/// as it is shown: with the file's root before it, unless it is written
/// from the root already. `NAMESPACE Std.Timers` in the library `Std` is
/// `Std.Timers`, as `NAMESPACE Timers` there is. Computed once, when the
/// declaration is built: the result is a field of it.
pub fn absolute_paths(
    db: &dyn WorkspaceDataBase,
    file: File,
    written: NamespacePath,
    written_with_case: NamespacePath,
) -> (NamespacePath, NamespacePath) {
    match library_root(db, file) {
        Some(root) if written.fragments(db).first() != Some(&root.name) => {
            let prefixed = |first: Ident, rest: NamespacePath| {
                let mut fragments = vec![first];
                fragments.extend(rest.fragments(db).iter().copied());
                NamespacePath::new(db, fragments)
            };
            (
                prefixed(root.name, written),
                prefixed(root.spelling, written_with_case),
            )
        }
        _ => (written, written_with_case),
    }
}

/// The root named `name`, when a library has it.
pub fn root_named(db: &dyn WorkspaceDataBase, name: Ident) -> Option<Root> {
    library_roots(db)
        .iter()
        .find(|root| root.name == name)
        .copied()
}

/// The files of the library whose root is `root`, in the order of their
/// URLs: the top level of each is a block of that namespace.
pub fn library_files_of_root(db: &dyn WorkspaceDataBase, root: Root) -> Vec<File> {
    let Some(id) = library_roots(db).iter().position(|other| *other == root) else {
        return Vec::new();
    };
    db::FileSet::get(db)
        .libraries(db)
        .iter()
        .copied()
        .filter(|file| db.library_of(*file) == Some(id as db::dependencies::LibraryId))
        .collect()
}
