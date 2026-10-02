// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use auto_lsp::default::db::file::File;
use auto_lsp::tree_sitter;
use db::WorkspaceDataBase;
use ide_diagnostic::IdeDiagnostic;

pub mod e00_syntax;
pub mod e01_duplicates;
pub mod e02_resolve;
pub mod e03_type;
pub mod e04_init;
pub mod e05_array;
pub mod e06_enum;
pub mod e07_subrange;
pub mod e08_call;
pub mod e09_reference;
pub mod e10_visibility;
pub mod e11_oop;
pub mod e12_control_flow;
pub mod e13_recursion;
pub mod e14_config;
pub mod e15_pragma;

pub trait ToIdeDiagnostic<'db> {
    /// Builds the IDE diagnostic. `file` is the file the diagnostic's primary range belongs to;
    /// it is carried (a `Copy` salsa struct) so ranges can be denormalized to the client encoding
    /// via [`crate::denormalize`] without re-deriving the document per node.
    fn to_diagnostic(&self, db: &'db dyn WorkspaceDataBase, file: File) -> IdeDiagnostic;
}

/*
E00xx = Syntax
E01xx = Duplicates
E02xx = Resolution
E03xx = Type System
E04xx = Initializers
E05xx = Arrays
E06xx = Enums
E07xx = Subranges
E08xx = Calls
E09xx = References
E10xx = Visibility
E11xx = OOP
E12xx = Control Flow
E13xx = Recursion
E14xx = Configuration
E15xx = Pragmas
*/

/// The first word of `span`: the keyword a misplaced section or declaration
/// opens with, so that a report underlines `VAR_TEMP` and not the block down
/// to its `END_VAR`. The span itself when it opens with anything else.
pub(crate) fn first_word(
    db: &dyn WorkspaceDataBase,
    file: File,
    span: &tree_sitter::Range,
) -> tree_sitter::Range {
    let document = file.document(db);
    let text = document.texter.text.as_bytes();
    let len = text
        .get(span.start_byte..span.end_byte)
        .unwrap_or_default()
        .iter()
        .take_while(|byte| byte.is_ascii_alphanumeric() || **byte == b'_')
        .count();
    if len == 0 {
        return *span;
    }
    tree_sitter::Range {
        start_byte: span.start_byte,
        end_byte: span.start_byte + len,
        start_point: span.start_point,
        end_point: tree_sitter::Point {
            row: span.start_point.row,
            column: span.start_point.column + len,
        },
    }
}
