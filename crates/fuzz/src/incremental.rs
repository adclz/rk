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

use crate::{Finding, session};

pub fn check(target: &str) -> Result<(), Finding> {
    if target.is_empty() {
        return Ok(());
    }
    let plan = Plan::new(target);
    if !plan.positions_can_say_it() {
        return Ok(());
    }
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
    let mir = session::lower(db, file).ok()?;
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
    /// LSP counts a lone `\r` as a line break, and has no position between the two
    /// bytes of a `\r\n`. This harness models neither, and no editor produces
    /// either, but a slice of the text can cut a `\r\n` in two: junk ending on its
    /// `\r` left the plan's first text with a lone one, and the edit that restores
    /// the input removed the wrong bytes.
    fn positions_can_say_it(&self) -> bool {
        let mut text = self.start.clone();
        for change in &self.changes {
            let end = change.at + change.remove;
            if has_lone_cr(&text) || inside_crlf(&text, change.at) || inside_crlf(&text, end) {
                return false;
            }
            text.replace_range(change.at..end, &change.insert);
        }
        !has_lone_cr(&text)
    }

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

fn has_lone_cr(text: &str) -> bool {
    text.replace("\r\n", "").contains('\r')
}

fn inside_crlf(text: &str, offset: usize) -> bool {
    let bytes = text.as_bytes();
    offset > 0 && bytes[offset - 1] == b'\r' && bytes.get(offset) == Some(&b'\n')
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

    /// What no position can say: a text with a lone `\r`, at the start or after
    /// a piece typed, and an edit between the two bytes of a `\r\n`.
    #[test]
    fn a_plan_no_position_can_say_is_left_out() {
        let plan = |start: &str, changes: &[(usize, usize, &str)]| Plan {
            start: start.to_string(),
            changes: changes
                .iter()
                .map(|(at, remove, insert)| Change {
                    at: *at,
                    remove: *remove,
                    insert: insert.to_string(),
                })
                .collect(),
        };
        // The night's crash: junk ending on the `\r` of a `\r\n`, pasted and deleted.
        assert!(!plan("ab: INT\rcd", &[(2, 6, "")]).positions_can_say_it());
        // Typed in two pieces, with the `\r` standing alone between them.
        assert!(!plan("abcd", &[(2, 0, "\r"), (3, 0, "\n")]).positions_can_say_it());
        // A `\r` left alone at the end.
        assert!(!plan("abcd", &[(2, 0, "\r")]).positions_can_say_it());
        // Between the two bytes, at either end of the edit.
        assert!(!plan("ab\r\ncd", &[(3, 1, "")]).positions_can_say_it());
        assert!(!plan("ab\r\ncd", &[(1, 2, "")]).positions_can_say_it());

        // A `\r\n` kept whole, typed whole or deleted whole is what editors send.
        assert!(plan("ab\r\ncd", &[(4, 2, "xy")]).positions_can_say_it());
        assert!(plan("abcd", &[(2, 0, "\r\n")]).positions_can_say_it());
        assert!(plan("ab\r\ncd", &[(2, 2, "")]).positions_can_say_it());
        assert!(plan("abcd", &[(1, 1, "x")]).positions_can_say_it());
    }

    /// Whatever the plan cuts, a text with `\r\n` line ends reads back as itself.
    /// The harness reported the wrong bytes removed when a cut left a lone `\r`.
    #[test]
    fn a_text_with_crlf_line_ends_is_no_finding() {
        for salt in 0..24 {
            let target = format!(
                "FUNCTION f : INT\r\nVAR x : INT; END_VAR\r\n    f := x;\r\nEND_FUNCTION\r\n{}",
                " ".repeat(salt)
            );
            let plan = Plan::new(&target);
            if let Err(finding) = check(&target) {
                panic!("{finding} (plan {:?})", plan.changes);
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
