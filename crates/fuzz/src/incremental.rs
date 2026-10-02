// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! The editor's path against `rk check`'s.
//!
//! The language server never builds a file from scratch: each keystroke
//! arrives as a `didChange`, auto-lsp applies it to the text, edits the
//! syntax tree and reparses incrementally, and Salsa recomputes what the
//! change invalidated. `rk check` parses and analyses the final text once.
//! Both must end in the same place, down to the positions: a stale span
//! (a `#[no_eq]` field that should have moved) is a squiggle on the wrong
//! word, and a stale memo a diagnostic that outlives its cause.
//!
//! The edit is derived from the input, so any text can serve: a slice of it
//! is taken out (or a slice of junk put in), the database is primed on that
//! text, and the edit that restores the input is sent the LSP way, typed in
//! pieces with the diagnostics pulled after each one, as an editor does.

use auto_lsp::default::db::file::File;
use auto_lsp::lsp_types::{
    DidChangeTextDocumentParams, Position, Range, TextDocumentContentChangeEvent,
    VersionedTextDocumentIdentifier,
};
use auto_lsp::tree_sitter::Node;
use db::RootDatabase;
use hir::check::diagnostics_for_file;
use hir::hir_def::semantic_index::semantic_index;

use crate::{Finding, session};

pub fn check(target: &str) -> Result<(), Finding> {
    // LSP counts a lone `\r` as a line break and this harness does not
    // model it; neither does any editor produce one.
    if target.is_empty() || target.replace("\r\n", "").contains('\r') {
        return Ok(());
    }
    let plan = Plan::new(target);
    let Some((mut db, file)) = session::load(&plan.start) else {
        return Ok(());
    };
    // Prime every stage, so the edit has memos to invalidate.
    prime(&db, file);

    let mut text = plan.start.clone();
    for (version, change) in plan.changes.iter().enumerate() {
        let range = Range::new(
            position(&text, change.at),
            position(&text, change.at + change.remove),
        );
        text.replace_range(change.at..change.at + change.remove, &change.insert);
        let event = DidChangeTextDocumentParams {
            text_document: VersionedTextDocumentIdentifier {
                uri: session::url(),
                version: version as i32 + 1,
            },
            content_changes: vec![TextDocumentContentChangeEvent {
                range: Some(range),
                range_length: None,
                text: change.insert.clone(),
            }],
        };
        if let Err(e) = file.update_edit(&mut db, &event) {
            return Err(Finding::new(
                "edit-apply",
                format!("didChange #{}: {e:?}", version + 1),
            ));
        }
        let _ = diagnostics_for_file(&db, file);
    }
    debug_assert_eq!(text, target);

    let edited = file.document(&db).texter.text.clone();
    if edited != target {
        return Err(Finding::new(
            "edit-text",
            format!(
                "after the edits the document reads {edited:?}, not {target:?} (edits: {:?})",
                plan.changes
            ),
        ));
    }

    let Some((fresh_db, fresh_file)) = session::load(target) else {
        return Ok(());
    };
    let incremental_tree = shape(file.document(&db).tree.root_node());
    let fresh_root = fresh_file.document(&fresh_db).tree.root_node();
    if incremental_tree != shape(fresh_root) {
        // Error recovery may legitimately depend on what the parser saw
        // before; a tree without errors may not.
        if !fresh_root.has_error() {
            return Err(Finding::new(
                "incremental-parse",
                format!(
                    "the reparsed tree differs from a fresh parse (edits: {:?})",
                    plan.changes
                ),
            ));
        }
        return Ok(());
    }

    let (live, fresh) = (
        session::positioned_diagnostics(&db, file),
        session::positioned_diagnostics(&fresh_db, fresh_file),
    );
    if live != fresh {
        return Err(Finding::new(
            "incremental-diagnostics",
            format!(
                "after the edits the editor reports\n  {live:?}\nwhere a fresh check reports\n  {fresh:?}\n(edits: {:?})",
                plan.changes
            ),
        ));
    }

    if session::compiles(&fresh_db, fresh_file)
        && let (Some(live), Some(fresh)) = (emit(&db, file), emit(&fresh_db, fresh_file))
        && live != fresh
    {
        return Err(Finding::new(
            "incremental-codegen",
            format!(
                "the module built after the edits differs from a fresh build (edits: {:?})",
                plan.changes
            ),
        ));
    }
    Ok(())
}

/// Run what the server and `rk watch` run, so every query has a memo.
fn prime(db: &RootDatabase, file: File) {
    let _ = diagnostics_for_file(db, file);
    if session::compiles(db, file) {
        let _ = emit(db, file);
    }
}

/// The debug module: its line table carries every span, so a span left
/// stale by an edit shows here even when the code is right.
fn emit(db: &RootDatabase, file: File) -> Option<Vec<u8>> {
    let mir = mir::lower::lower_module::lower_module(db, semantic_index(db, file)).ok()?;
    Some(wasm_codegen::generate_wasm(db, &mir).finish())
}

/// A tree as a comparable value: every node's kind, extent and error state.
fn shape(root: Node<'_>) -> Vec<(u16, usize, usize, bool, bool)> {
    let mut out = Vec::new();
    let mut cursor = root.walk();
    loop {
        let n = cursor.node();
        out.push((
            n.kind_id(),
            n.start_byte(),
            n.end_byte(),
            n.is_missing(),
            n.is_error(),
        ));
        if cursor.goto_first_child() || cursor.goto_next_sibling() {
            continue;
        }
        loop {
            if !cursor.goto_parent() {
                return out;
            }
            if cursor.goto_next_sibling() {
                break;
            }
        }
    }
}

/// The LSP position of a byte offset: UTF-16 code units, auto-lsp's default.
fn position(text: &str, offset: usize) -> Position {
    let before = &text[..offset];
    let line = before.matches('\n').count();
    let line_start = before.rfind('\n').map_or(0, |i| i + 1);
    let character: usize = before[line_start..].chars().map(char::len_utf16).sum();
    Position::new(line as u32, character as u32)
}

#[derive(Debug)]
struct Change {
    at: usize,
    remove: usize,
    insert: String,
}

/// Where the database starts, and the edits that take it to the target.
struct Plan {
    start: String,
    changes: Vec<Change>,
}

impl Plan {
    fn new(target: &str) -> Self {
        let mut bits = fnv1a(target.as_bytes());
        let mut take = |n: u64| {
            let v = bits % n;
            bits = bits.rotate_right(13) ^ 0x9e37_79b9_7f4a_7c15;
            v as usize
        };

        let n = target.len();
        let at = floor(target, take(n as u64 + 1));
        let end = ceil(target, (at + 1 + take(48)).min(n));
        let chunk = &target[at..end];
        // Junk is another slice of the same text: tokens the grammar knows.
        let from = floor(target, take(n as u64));
        let junk = &target[from..ceil(target, (from + 1 + take(24)).min(n))];

        match take(3) {
            // Typed back in, in up to three pieces.
            0 => {
                let start = format!("{}{}", &target[..at], &target[end..]);
                let mut changes = Vec::new();
                let mut typed = 0;
                while typed < chunk.len() {
                    let left = &chunk[typed..];
                    let piece = ceil(left, (1 + take(left.len() as u64)).min(left.len()));
                    changes.push(Change {
                        at: at + typed,
                        remove: 0,
                        insert: left[..piece].to_string(),
                    });
                    typed += piece;
                    if changes.len() == 2 {
                        changes.push(Change {
                            at: at + typed,
                            remove: 0,
                            insert: chunk[typed..].to_string(),
                        });
                        break;
                    }
                }
                changes.retain(|c| !c.insert.is_empty());
                Self { start, changes }
            }
            // Pasted junk, deleted.
            1 => Self {
                start: format!("{}{junk}{}", &target[..at], &target[at..]),
                changes: vec![Change {
                    at,
                    remove: junk.len(),
                    insert: String::new(),
                }],
            },
            // Junk replaced by what belongs there, in one change.
            _ => Self {
                start: format!("{}{junk}{}", &target[..at], &target[end..]),
                changes: vec![Change {
                    at,
                    remove: junk.len(),
                    insert: chunk.to_string(),
                }],
            },
        }
    }
}

fn floor(s: &str, mut i: usize) -> usize {
    while !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

fn ceil(s: &str, mut i: usize) -> usize {
    while !s.is_char_boundary(i) {
        i += 1;
    }
    i
}

fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ *b as u64).wrapping_mul(0x0100_0000_01b3)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Whatever the plan, replaying it on its start yields the target.
    #[test]
    fn every_plan_ends_at_its_target() {
        let texts = [
            "FUNCTION f : INT\n    f := 1;\nEND_FUNCTION\n",
            "PROGRAM p\nVAR s : STRING := 'héllo ✓'; END_VAR\nEND_PROGRAM",
            "x",
            "ÿ",
        ];
        for text in texts {
            for salt in 0..64 {
                let target = format!("{text}{}", " ".repeat(salt));
                let plan = Plan::new(&target);
                let mut t = plan.start.clone();
                for c in &plan.changes {
                    t.replace_range(c.at..c.at + c.remove, &c.insert);
                }
                assert_eq!(t, target, "plan {:?}", plan.changes);
            }
        }
    }

    #[test]
    fn positions_count_utf16_units() {
        let text = "ab\n✓😀x";
        assert_eq!(position(text, 0), Position::new(0, 0));
        assert_eq!(position(text, 3), Position::new(1, 0));
        assert_eq!(position(text, 3 + '✓'.len_utf8()), Position::new(1, 1));
        assert_eq!(position(text, text.len()), Position::new(1, 4));
    }
}
