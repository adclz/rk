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
//! A tool registers before it opens a workspace, then is rk:
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

use std::sync::RwLock;

use auto_lsp::core::parsers::Parser;
use auto_lsp::default::db::file::File;
use auto_lsp::default::db::tracked::ParsedAst;
use db::WorkspaceDataBase;
use db::source::SourceKind;

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
    /// any case. One extension names one kind.
    pub extensions: &'static [&'static str],
    /// The parser that reads one.
    pub parser: &'static Parser,
    /// What builds its index.
    pub index: IndexFn,
}

/// The index function of each front-end added, by the address of its
/// parser: a file keeps the parser that read it.
static INDEXES: RwLock<Vec<(usize, IndexFn)>> = RwLock::new(Vec::new());

fn address(parser: &Parser) -> usize {
    std::ptr::from_ref(parser) as usize
}

/// Adds a front-end to the process, beside rk's own.
///
/// Call it before a workspace is opened or a file read. Adding the same
/// front-end again changes nothing, at any time.
///
/// # Panics
///
/// When files were read already and this front-end is a new one: what was
/// answered without it would stay as it is. And when another front-end
/// names one of its extensions.
pub fn register(front_end: FrontEnd) {
    // Held across both halves: a file of this kind is not indexed between
    // the moment it can be found and the moment its index can be built.
    let mut indexes = INDEXES.write().unwrap_or_else(|e| e.into_inner());
    db::source::register(SourceKind {
        extensions: front_end.extensions,
        parser: front_end.parser,
    });
    let parser = address(front_end.parser);
    if !db::source::is_rk(front_end.parser) && indexes.iter().all(|(known, _)| *known != parser) {
        indexes.push((parser, front_end.index));
    }
}

/// What builds the index of a file `parser` read, when a front-end was
/// added for it. rk's own is not in this table.
pub(crate) fn index_of(parser: &Parser) -> Option<IndexFn> {
    // The answer is kept with the file's index, so the table is fixed
    // before it is given: a front-end added after is refused, where it
    // would leave this file with the index of a file nothing reads.
    db::source::kinds();
    let parser = address(parser);
    INDEXES
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .find(|(known, _)| *known == parser)
        .map(|(_, index)| *index)
}
