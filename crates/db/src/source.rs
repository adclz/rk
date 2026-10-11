// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! The kinds of source file a workspace holds.
//!
//! rk reads `.st` files with its own parser. A tool built on rk can read
//! other files beside them: it names their extensions and the parser that
//! reads one, before it opens a workspace. The call such a tool makes is
//! `hir::front_end::register`, which also says who builds a file's index;
//! this module is the half of it that finds and parses the files.
//!
//! The table belongs to the process, as the parsers do, and is fixed the
//! first time it is read. A kind added later would be missing from what
//! was already answered: the files a workspace was found to hold, the
//! index of a file read before its front-end was known. So a late one is
//! refused.

use std::path::Path;
use std::sync::{Mutex, OnceLock};

use auto_lsp::core::parsers::Parser;

/// The extensions of rk's own files.
pub const RK_EXTENSIONS: &[&str] = &["st"];

/// A kind of source file.
#[derive(Debug, Clone, Copy)]
pub struct SourceKind {
    /// The extensions that name a file of this kind, without the dot.
    /// Compared in any case: `MAIN.ST` is a `.st` file.
    pub extensions: &'static [&'static str],
    /// The parser that reads one. A file keeps the parser it was read
    /// with, and that is how its kind is known afterwards.
    pub parser: &'static Parser,
}

impl SourceKind {
    fn names(&self, extension: &std::ffi::OsStr) -> bool {
        self.extensions
            .iter()
            .any(|own| extension.eq_ignore_ascii_case(own))
    }

    fn is(&self, parser: &Parser) -> bool {
        std::ptr::eq(self.parser, parser)
    }
}

/// Why a kind was not added.
#[derive(Debug, PartialEq, Eq)]
pub enum Refused {
    /// The table was read already.
    Late,
    /// Another kind names this extension.
    ExtensionTaken(&'static str),
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refused::Late => f.write_str(
                "the kinds of source file are fixed once a workspace was opened or a file read",
            ),
            Refused::ExtensionTaken(extension) => {
                write!(f, "another kind of source file is named '.{extension}'")
            }
        }
    }
}

enum Table {
    /// The kinds added so far, rk's left out.
    Open(Vec<SourceKind>),
    /// rk's kind, then the others in the order they were added.
    Fixed(&'static [SourceKind]),
}

/// The table of kinds: filled, then read. One per process ([`kinds`]);
/// a type of its own so that its rules are tested on a table of the
/// test's.
pub struct Kinds {
    table: Mutex<Table>,
    /// The fixed table again, to read it without the lock.
    fixed: OnceLock<&'static [SourceKind]>,
}

impl Default for Kinds {
    fn default() -> Self {
        Self::new()
    }
}

impl Kinds {
    pub const fn new() -> Self {
        Kinds {
            table: Mutex::new(Table::Open(Vec::new())),
            fixed: OnceLock::new(),
        }
    }

    /// Adds `kind`. One whose parser is there already is there already,
    /// at any time: a tool may register on each of its entries.
    pub fn register(&self, kind: SourceKind) -> Result<(), Refused> {
        let mut table = self.table.lock().unwrap_or_else(|e| e.into_inner());
        let rk = rk_kind();
        let known: &[SourceKind] = match &*table {
            Table::Open(added) => added,
            Table::Fixed(all) => all,
        };
        if rk.is(kind.parser) || known.iter().any(|other| other.is(kind.parser)) {
            return Ok(());
        }
        // Before the table's state is looked at: the fault of naming a
        // taken extension is the same whenever it is made.
        let taken = kind.extensions.iter().find(|extension| {
            let extension = std::ffi::OsStr::new(extension);
            rk.names(extension) || known.iter().any(|other| other.names(extension))
        });
        if let Some(extension) = taken {
            return Err(Refused::ExtensionTaken(extension));
        }
        match &mut *table {
            Table::Open(added) => {
                added.push(kind);
                Ok(())
            }
            Table::Fixed(_) => Err(Refused::Late),
        }
    }

    /// Every kind, rk's first. The table is fixed from this call on.
    pub fn all(&self) -> &'static [SourceKind] {
        if let Some(all) = self.fixed.get() {
            return all;
        }
        let mut table = self.table.lock().unwrap_or_else(|e| e.into_inner());
        let all = match &mut *table {
            Table::Fixed(all) => *all,
            Table::Open(added) => {
                let mut all = vec![rk_kind()];
                all.append(added);
                // Once per table, for the life of the process.
                let all: &'static [SourceKind] = Box::leak(all.into_boxed_slice());
                *table = Table::Fixed(all);
                all
            }
        };
        // Set under the lock: `register` never sees a table that is fixed
        // for one reader and open for another.
        let _ = self.fixed.set(all);
        all
    }
}

fn rk_kind() -> SourceKind {
    SourceKind {
        extensions: RK_EXTENSIONS,
        parser: &ast::RK_PARSER,
    }
}

static KINDS: Kinds = Kinds::new();

/// Adds a kind of source file to the process's table.
///
/// A tool calls `hir::front_end::register`, which calls this.
///
/// # Panics
///
/// When the table was read already and does not hold this kind, or when
/// another kind names one of its extensions.
pub fn register(kind: SourceKind) {
    if let Err(refused) = KINDS.register(kind) {
        panic!("{refused}");
    }
}

/// Every kind of source file, rk's first.
pub fn kinds() -> &'static [SourceKind] {
    KINDS.all()
}

/// The kind `path` is named as, by its extension.
pub fn kind_of(path: &Path) -> Option<&'static SourceKind> {
    let extension = path.extension()?;
    kinds().iter().find(|kind| kind.names(extension))
}

/// Whether `parser` is rk's own: the file it read is one of rk's.
pub fn is_rk(parser: &Parser) -> bool {
    std::ptr::eq(parser, &*ast::RK_PARSER)
}

/// The extensions a source file can have, as a message names them:
/// `.st`, or `.st, .a or .b`.
pub fn extensions() -> String {
    let all: Vec<String> = kinds()
        .iter()
        .flat_map(|kind| kind.extensions)
        .map(|extension| format!(".{extension}"))
        .collect();
    match all.split_last() {
        Some((last, rest)) if !rest.is_empty() => format!("{} or {last}", rest.join(", ")),
        _ => all.concat(),
    }
}

// `configure_parser!` (auto-lsp) expands to functions returning
// `Result<_, ParseError>` whose Err variant is ~144 bytes.
#[cfg(test)]
#[allow(clippy::result_large_err)]
mod tests {
    use super::*;

    auto_lsp::configure_parser!(
        SECOND,
        language: tree_sitter_rk::LANGUAGE,
        ast_root: SourceFile,
    );
    auto_lsp::configure_parser!(
        THIRD,
        language: tree_sitter_rk::LANGUAGE,
        ast_root: SourceFile,
    );
    use ast::generated::SourceFile;

    fn second() -> SourceKind {
        SourceKind {
            extensions: &["two", "deux"],
            parser: &SECOND,
        }
    }

    #[test]
    fn rk_comes_first_and_the_others_in_the_order_they_were_added() {
        let kinds = Kinds::new();
        kinds.register(second()).unwrap();
        kinds
            .register(SourceKind {
                extensions: &["three"],
                parser: &THIRD,
            })
            .unwrap();
        let all = kinds.all();
        assert_eq!(all.len(), 3);
        assert!(is_rk(all[0].parser));
        assert_eq!(all[0].extensions, RK_EXTENSIONS);
        assert_eq!(all[1].extensions, ["two", "deux"]);
        assert_eq!(all[2].extensions, ["three"]);
    }

    #[test]
    fn a_table_nothing_was_added_to_holds_rk_alone() {
        let all = Kinds::new().all();
        assert_eq!(all.len(), 1);
        assert!(is_rk(all[0].parser));
    }

    /// A tool with several entries registers on each: the second time is
    /// no fault, before the table is read or after.
    #[test]
    fn a_kind_added_twice_is_there_once() {
        let kinds = Kinds::new();
        kinds.register(second()).unwrap();
        kinds.register(second()).unwrap();
        assert_eq!(kinds.all().len(), 2);
        assert_eq!(kinds.register(second()), Ok(()));
        assert_eq!(kinds.all().len(), 2);
    }

    /// What was read stays what it was: a kind unknown to a table that
    /// was read is refused, where it would have changed the answer.
    #[test]
    fn a_kind_is_refused_once_the_table_was_read() {
        let kinds = Kinds::new();
        assert_eq!(kinds.all().len(), 1);
        assert_eq!(kinds.register(second()), Err(Refused::Late));
        assert_eq!(kinds.all().len(), 1);
    }

    #[test]
    fn an_extension_names_one_kind() {
        let kinds = Kinds::new();
        kinds.register(second()).unwrap();
        let again = SourceKind {
            extensions: &["three", "DEUX"],
            parser: &THIRD,
        };
        assert_eq!(kinds.register(again), Err(Refused::ExtensionTaken("DEUX")));
        let rk_s = SourceKind {
            extensions: &["St"],
            parser: &THIRD,
        };
        assert_eq!(kinds.register(rk_s), Err(Refused::ExtensionTaken("St")));
        assert_eq!(kinds.all().len(), 2);
    }

    /// The same fault is named the same way before the table is read
    /// and after.
    #[test]
    fn a_taken_extension_is_named_once_the_table_was_read_too() {
        let kinds = Kinds::new();
        kinds.register(second()).unwrap();
        kinds.all();
        let again = SourceKind {
            extensions: &["two"],
            parser: &THIRD,
        };
        assert_eq!(kinds.register(again), Err(Refused::ExtensionTaken("two")));
    }

    /// rk's own kind is there without being added, so adding it changes
    /// nothing, whatever extensions it is given.
    #[test]
    fn rk_s_parser_is_never_added() {
        let kinds = Kinds::new();
        let rk = SourceKind {
            extensions: &["other"],
            parser: &ast::RK_PARSER,
        };
        assert_eq!(kinds.register(rk), Ok(()));
        let all = kinds.all();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].extensions, RK_EXTENSIONS);
    }

    #[test]
    fn a_refusal_says_why() {
        assert_eq!(
            Refused::ExtensionTaken("st").to_string(),
            "another kind of source file is named '.st'"
        );
        assert_eq!(
            Refused::Late.to_string(),
            "the kinds of source file are fixed once a workspace was opened or a file read"
        );
    }
}
