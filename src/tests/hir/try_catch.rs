// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! `__TRY … __CATCH … __FINALLY … __END_TRY`: read, and its shape checked.

use crate::tests::utils::{add_source, render_snapshot, test_diagnostics, with_db};
use db::RootDatabase;
use hir::check::diagnostics_for_file;
use insta::assert_snapshot;
use rstest::*;

/// The diagnostics of `source`, without the build `test_diagnostics` makes
/// of an accepted program: a `__TRY` is not compiled yet.
fn checked(db: &mut RootDatabase, source: &str) -> String {
    let file = add_source(db, source);
    let diags = diagnostics_for_file(db, file).to_vec();
    render_snapshot(db, file, diags)
}

/// Every shape: a `__CATCH` with its variable or bare, a `__FINALLY` with
/// or without it, one clause alone, a `__TRY` inside a `__CATCH`, and the
/// keywords in any case.
#[rstest]
fn a_try_with_its_clauses_is_read(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION f1 : INT
        VAR
            e : STRING;
        END_VAR
            __TRY
                __RAISE('oops');
            __CATCH(e)
                __TRY
                    f1 := 1;
                __CATCH
                    f1 := 2;
                __END_TRY;
            __FINALLY
                f1 := 3;
            __END_TRY;
            __TRY
                f1 := 4;
            __FINALLY
                f1 := 5;
            __END_TRY;
            __try
                f1 := 6;
            __catch (e)
                f1 := 7;
            __end_try;
        END_FUNCTION
    "#;
    assert_snapshot!(checked(&mut with_db, source), @"");
}

/// With neither clause, nothing is left of the `__TRY` but its body.
#[rstest]
fn invalid_try_without_a_clause(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION f1 : INT
            __TRY
                f1 := 1;
            __END_TRY;
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1209] Error: __TRY without a __CATCH or a __FINALLY
       ,-[ file:///test0.st:3:13 ]
       |
     3 |             __TRY
       |             ^^|^^
       |               `---- this __TRY has neither a __CATCH nor a __FINALLY
       |
       | Help: add a __CATCH, or remove the __TRY
       |
       | Note: a __TRY handles the exception its body raises in its __CATCH, or runs its __FINALLY on the way out
    ---'
    ");
}

/// A jump out of a `__FINALLY` would drop the exception it runs with: a
/// `RETURN` anywhere in it, and an `EXIT` or a `CONTINUE` for a loop
/// outside it.
#[rstest]
fn invalid_jump_out_of_a_finally(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION f1 : INT
        VAR
            i : INT;
        END_VAR
            WHILE i < 3 DO
                i := i + 1;
                __TRY
                    f1 := 1;
                __FINALLY
                    IF i = 1 THEN
                        EXIT;
                    END_IF;
                    IF i = 2 THEN
                        CONTINUE;
                    END_IF;
                    RETURN;
                __END_TRY;
            END_WHILE;
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1210] Error: jump out of a __FINALLY
        ,-[ file:///test0.st:12:25 ]
        |
     12 |                         EXIT;
        |                         ^^|^
        |                           `--- 'EXIT' leaves the __FINALLY
        |
        | Help: move the 'EXIT' after __END_TRY
        |
        | Note: a __FINALLY runs on the way out of its __TRY, with or without an exception pending, which a jump out of it would drop
    ----'
    [E1210] Error: jump out of a __FINALLY
        ,-[ file:///test0.st:15:25 ]
        |
     15 |                         CONTINUE;
        |                         ^^^^|^^^
        |                             `----- 'CONTINUE' leaves the __FINALLY
        |
        | Help: move the 'CONTINUE' after __END_TRY
        |
        | Note: a __FINALLY runs on the way out of its __TRY, with or without an exception pending, which a jump out of it would drop
    ----'
    [E1210] Error: jump out of a __FINALLY
        ,-[ file:///test0.st:17:21 ]
        |
     17 |                     RETURN;
        |                     ^^^|^^
        |                        `---- 'RETURN' leaves the __FINALLY
        |
        | Help: move the 'RETURN' after __END_TRY
        |
        | Note: a __FINALLY runs on the way out of its __TRY, with or without an exception pending, which a jump out of it would drop
    ----'
    ");
}

/// A loop inside the `__FINALLY` is left from inside it, and a `__TRY`
/// inside a loop is at the loop's level: its `EXIT` leaves the loop.
#[rstest]
fn a_jump_inside_a_finally_or_a_try_stays_in_its_loop(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION f1 : INT
        VAR
            i : INT;
        END_VAR
            __TRY
                f1 := 1;
            __FINALLY
                WHILE i < 3 DO
                    i := i + 1;
                    IF i = 1 THEN
                        CONTINUE;
                    END_IF;
                    EXIT;
                END_WHILE;
            __END_TRY;
            WHILE i < 6 DO
                i := i + 1;
                __TRY
                    EXIT;
                __CATCH
                    CONTINUE;
                __END_TRY;
            END_WHILE;
        END_FUNCTION
    "#;
    assert_snapshot!(checked(&mut with_db, source), @"");
}

/// An `EXIT` in a `__TRY` outside any loop has nothing to leave, as
/// before.
#[rstest]
fn invalid_exit_in_a_try_outside_a_loop(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION f1 : INT
            __TRY
                EXIT;
            __CATCH
                f1 := 1;
            __END_TRY;
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1201] Error: EXIT outside loop
       ,-[ file:///test0.st:4:17 ]
       |
     4 |                 EXIT;
       |                 ^^|^
       |                   `--- 'EXIT' is outside a loop
    ---'
    ");
}

/// The `__CATCH` comes before the `__FINALLY`. Written after it, it is
/// reported, and still read: a name it uses is resolved.
#[rstest]
fn invalid_catch_after_finally(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION f1 : INT
        VAR
            e : STRING;
        END_VAR
            __TRY
                f1 := 1;
            __FINALLY
                f1 := 2;
            __CATCH(e)
                f1 := unknown;
            __END_TRY;
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1211] Error: __CATCH after __FINALLY
        ,-[ file:///test0.st:10:13 ]
        |
     10 |             __CATCH(e)
        |             ^^^|^^^
        |                `----- this __CATCH comes after the __FINALLY
        |
        | Help: move the __CATCH before the __FINALLY
        |
        | Note: the clauses of a __TRY are its __CATCH, then its __FINALLY
    ----'
    [E0201] Error: unknown name
        ,-[ file:///test0.st:11:23 ]
        |
     11 |                 f1 := unknown;
        |                       ^^^|^^^
        |                          `----- no item 'unknown' found in scope
    ----'
    ");
}
