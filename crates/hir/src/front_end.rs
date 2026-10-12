// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! A front-end: what reads one kind of source file into an index.
//!
//! rk's own reads `.st` files. A tool built on rk can add another, for a
//! language rk does not know: the extensions of its files, the parser that
//! reads one, and the function that builds a file's [`SemanticIndex`].
//!
//! From the index on there is one compiler. A name resolves across files
//! of the two kinds, and the checks, the lints, the lowering and the
//! emitted module are the same for both.
//!
//! A tool adds its front-end, then is rk:
//!
//! ```ignore
//! fn main() -> std::process::ExitCode {
//!     hir::front_end::register(hir::front_end::FrontEnd {
//!         extensions: &["other"],
//!         parser: &other::PARSER,
//!         index: other::index,
//!     });
//!     rk::main()
//! }
//! ```
//!
//! Its index function has rk's builder to make the index with
//! ([`SemanticIndexBuilder`](crate::builder::semantic_index::SemanticIndexBuilder)):
//! it walks its own tree, and the builder's methods make the nodes.
//!
//! What stays rk's: the formatter and the editor read rk's own tree, so
//! `rk fmt` and the language server take `.st` files only.

use std::path::Path;
use std::sync::RwLock;

use auto_lsp::core::parsers::Parser;
use auto_lsp::default::db::file::File;
use auto_lsp::default::db::tracked::ParsedAst;
use db::WorkspaceDataBase;

use crate::hir_def::semantic_index::SemanticIndex;

/// Builds the index of a file its front-end's parser read.
///
/// It runs inside [`semantic_index`](crate::hir_def::semantic_index::semantic_index),
/// once for each text the file takes: what it reads through `db` is
/// tracked as that query's. `ast` has a root, and each of its nodes found
/// its place in the typed tree. What the file holds that the front-end
/// does not read is its to report, in the index's `errors`.
pub type IndexFn =
    for<'db> fn(&'db dyn WorkspaceDataBase, File, &'db ParsedAst) -> SemanticIndex<'db>;

/// What reads one kind of source file.
#[derive(Clone, Copy)]
pub struct FrontEnd {
    /// The extensions that name a file of this kind, without the dot, in
    /// any case. One extension names one kind, and `st` is rk's.
    pub extensions: &'static [&'static str],
    /// The parser that reads one. A file keeps the parser it was read
    /// with, and that is how its front-end is found again.
    pub parser: &'static Parser,
    /// What builds its index.
    pub index: IndexFn,
}

impl FrontEnd {
    fn names(&self, extension: &std::ffi::OsStr) -> bool {
        self.extensions
            .iter()
            .any(|own| extension.eq_ignore_ascii_case(own))
    }
}

/// The front-ends added, in the order they were. rk's own is not among
/// them: it needs no adding.
static ADDED: RwLock<Vec<FrontEnd>> = RwLock::new(Vec::new());

/// Adds a front-end to the process, beside rk's own.
///
/// Call it before a workspace is opened: a file is found, and its index
/// built, with the front-ends there are at that moment. Adding the same
/// front-end again changes nothing.
///
/// # Panics
///
/// When rk or another front-end names one of its extensions.
pub fn register(front_end: FrontEnd) {
    let taken = {
        let mut added = ADDED.write().unwrap_or_else(|e| e.into_inner());
        let known = db::loader::is_rk_parser(front_end.parser)
            || added
                .iter()
                .any(|other| std::ptr::eq(other.parser, front_end.parser));
        if known {
            return;
        }
        let taken = front_end.extensions.iter().find(|extension| {
            let extension = std::ffi::OsStr::new(extension);
            db::loader::is_rk_extension(extension)
                || added.iter().any(|other| other.names(extension))
        });
        if taken.is_none() {
            added.push(front_end);
        }
        taken
    };
    if let Some(extension) = taken {
        panic!("another front-end reads '.{extension}' files");
    }
}

/// The parser that reads the file at `path`, by its extension: rk's for a
/// `.st` file, a front-end's for one of its own, and none for a file that
/// is no source. What the workspace's loader is given to find its files
/// with.
pub fn parser_of(path: &Path) -> Option<&'static Parser> {
    if let Some(rk) = db::loader::rk_parser(path) {
        return Some(rk);
    }
    let extension = path.extension()?;
    ADDED
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .find(|front_end| front_end.names(extension))
        .map(|front_end| front_end.parser)
}

/// What builds the index of a file `parser` read, when a front-end was
/// added for it. rk's own is not asked here.
pub(crate) fn index_of(parser: &Parser) -> Option<IndexFn> {
    ADDED
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .find(|front_end| std::ptr::eq(front_end.parser, parser))
        .map(|front_end| front_end.index)
}
