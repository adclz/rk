// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

pub mod identifier;
pub mod namespace;

/// How long an interned value is kept once nothing uses it: the revisions
/// that interned anything, counted from the last one that used it. Then its
/// slot goes to a new value. Salsa's default is one: a name written in an
/// edited file took the slot of a name another file held, and that file was
/// indexed and inferred again, then the one whose name it took in turn. A
/// check uses every name of the workspace, so a name the last check used is
/// kept; the margin is for the checks an edit cancelled before they reached
/// every file. A name nothing uses is still let go, so what was typed does
/// not stay in memory.
pub(crate) const REVISIONS: usize = 1024;
