// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! What a build says of a statement rk reads and checks and has no code
//! for yet: the statement, where it is, and that it is not compiled. Not
//! an internal compiler error, which is what any other failure to lower a
//! checked program is.
//!
//! When the statement is compiled, this test fails: it is then to go, with
//! `LowerTypeError::NotCompiledYet` if nothing else is left to refuse.

use rk::workspace::init_db;

use super::{disable_stdlib, temp_workspace};

/// A `__FINALLY` passes `rk check`, and a build refuses it as not
/// compiled yet, under its `__TRY`.
#[test]
fn a_finally_is_refused_by_the_build_as_not_compiled_yet() {
    let source = "FUNCTION Guarded : INT
VAR e : STRING; END_VAR
    __TRY
        Guarded := 1;
    __CATCH(e)
        Guarded := 2;
    __FINALLY
        Guarded := 3;
    __END_TRY;
END_FUNCTION
";
    let (_ws, root) = temp_workspace(&[("main.st", source)]);
    disable_stdlib();
    let db = init_db(&root, false, true).expect("init db");

    let reported: Vec<_> = rk::file_order::ordered_files_with_libraries(&db)
        .into_iter()
        .flat_map(|file| hir::check::diagnostics_for_file(&db, file).to_vec())
        .map(|d| d.diagnostic.message)
        .collect();
    assert!(reported.is_empty(), "`rk check` accepts it: {reported:?}");

    let error = rk::compiler::build_core_quiet(&db, &root, false).expect_err("no code for it");
    assert!(
        error.ends_with("compilation failed: a __FINALLY is not compiled yet\n"),
        "{error}"
    );
    assert!(
        error.contains("main.st:3:5"),
        "under the statement: {error}"
    );
    assert!(
        !error.contains("internal compiler error"),
        "the program's to rewrite, not a bug to report: {error}"
    );
}
