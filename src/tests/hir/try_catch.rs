// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! `__TRY … __CATCH … __FINALLY … __END_TRY`: read, its shape checked and
//! its `__CATCH` variable typed. What it runs is `codegen/try_catch.rs`.

use crate::tests::utils::{test_diagnostics, with_db};
use db::RootDatabase;
use insta::assert_snapshot;
use rstest::*;

/// A valid `__TRY` is accepted. A `RETURN` in the body and one in the
/// `__CATCH` are the ways out a `__FINALLY` has to run on. The keywords
/// are read in any case.
#[rstest]
fn a_try_with_its_clauses_is_accepted(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION f1 : INT
        VAR
            e : STRING;
        END_VAR
            __Try
                f1 := 1;
                RETURN;
            __Catch(e)
                f1 := 2;
                RETURN;
            __Finally
                f1 := 3;
            __End_Try;
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// Each clause is read, a nested `__TRY` included: a name that does not
/// exist is reported in the body, as the `__CATCH`'s variable, in a body
/// nested in the `__CATCH` and in that one's `__FINALLY`.
#[rstest]
fn invalid_names_in_every_clause_are_reported(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION f1 : INT
            __TRY
                f1 := in_try;
            __CATCH(nope)
                __TRY
                    f1 := in_nested;
                __FINALLY
                    f1 := in_finally;
                __END_TRY;
            __END_TRY;
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0201] Error: unknown name
       ,-[ file:///test0.st:4:23 ]
       |
     4 |                 f1 := in_try;
       |                       ^^^|^^
       |                          `---- no item 'in_try' found in scope
    ---'
    [E0201] Error: unknown name
       ,-[ file:///test0.st:5:21 ]
       |
     5 |             __CATCH(nope)
       |                     ^^|^
       |                       `--- no item 'nope' found in scope
    ---'
    [E0201] Error: unknown name
       ,-[ file:///test0.st:7:27 ]
       |
     7 |                     f1 := in_nested;
       |                           ^^^^|^^^^
       |                               `------ no item 'in_nested' found in scope
    ---'
    [E0201] Error: unknown name
       ,-[ file:///test0.st:9:27 ]
       |
     9 |                     f1 := in_finally;
       |                           ^^^^^|^^^^
       |                                `------ no item 'in_finally' found in scope
    ---'
    ");
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
       | Help: add a __CATCH or remove the __TRY
       |
       | Note: a __TRY needs a __CATCH, a __FINALLY or both
    ---'
    ");
}

/// A jump out of a `__FINALLY` would end the exception it runs with: a
/// `RETURN` in it, and an `EXIT` or a `CONTINUE` for a loop outside it.
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
        | Note: a jump out of a __FINALLY would cancel an exception that was not caught
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
        | Note: a jump out of a __FINALLY would cancel an exception that was not caught
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
        | Note: a jump out of a __FINALLY would cancel an exception that was not caught
    ----'
    ");
}

/// A loop inside the `__FINALLY` does not make a `RETURN` its own, and a
/// `__TRY` inside the `__FINALLY` does not hide it: an `EXIT` in that
/// one's `__CATCH` is for the loop outside.
#[rstest]
fn invalid_jump_out_of_a_finally_from_a_loop_or_a_try_inside_it(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION f1 : INT
        VAR
            i : INT;
        END_VAR
            WHILE i < 3 DO
                __TRY
                    f1 := 1;
                __FINALLY
                    WHILE i < 6 DO
                        RETURN;
                    END_WHILE;
                    __TRY
                        f1 := 2;
                    __CATCH
                        EXIT;
                    __END_TRY;
                __END_TRY;
            END_WHILE;
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1210] Error: jump out of a __FINALLY
        ,-[ file:///test0.st:11:25 ]
        |
     11 |                         RETURN;
        |                         ^^^|^^
        |                            `---- 'RETURN' leaves the __FINALLY
        |
        | Help: move the 'RETURN' after __END_TRY
        |
        | Note: a jump out of a __FINALLY would cancel an exception that was not caught
    ----'
    [E1210] Error: jump out of a __FINALLY
        ,-[ file:///test0.st:16:25 ]
        |
     16 |                         EXIT;
        |                         ^^|^
        |                           `--- 'EXIT' leaves the __FINALLY
        |
        | Help: move the 'EXIT' after __END_TRY
        |
        | Note: a jump out of a __FINALLY would cancel an exception that was not caught
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
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// A `__RAISE` in a `__FINALLY` is accepted. A jump would end the
/// exception that is leaving, and the program would go on as if nothing
/// had been raised. A raise puts another in its place, and one still
/// leaves.
#[rstest]
fn a_raise_in_a_finally_is_accepted(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION f1 : INT
            __TRY
                f1 := 1;
            __FINALLY
                __RAISE('while cleaning up');
            __END_TRY;
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// An `EXIT` in a `__TRY` outside any loop has nothing to leave, as
/// anywhere else.
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
        | Note: in a __TRY, the __CATCH is written before the __FINALLY
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

/// A `__TRY` has one `__CATCH` and one `__FINALLY`. A second of either is
/// reported against the first, and what it holds is checked all the same:
/// its variable and its statements are read as the first's. Two `__CATCH`
/// were reported as one written after a `__FINALLY` that was not there.
#[rstest]
fn invalid_clause_written_twice(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION f1 : INT
            __TRY
                f1 := 1;
            __CATCH
                f1 := 2;
            __CATCH(nowhere)
                f1 := unknown;
            __END_TRY;
            __TRY
                f1 := 3;
            __FINALLY
                f1 := 4;
            __FINALLY
                f1 := 5;
            __END_TRY;
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1212] Error: clause of a __TRY written twice
       ,-[ file:///test0.st:7:13 ]
       |
     5 |             __CATCH
       |             ^^^|^^^
       |                `----- the first __CATCH is here
       |
     7 |             __CATCH(nowhere)
       |             ^^^|^^^
       |                `----- this is the second __CATCH of the __TRY
       |
       | Help: move its statements into the first __CATCH
       |
       | Note: a __TRY can only have one __CATCH
    ---'
    [E0201] Error: unknown name
       ,-[ file:///test0.st:7:21 ]
       |
     7 |             __CATCH(nowhere)
       |                     ^^^|^^^
       |                        `----- no item 'nowhere' found in scope
    ---'
    [E0201] Error: unknown name
       ,-[ file:///test0.st:8:23 ]
       |
     8 |                 f1 := unknown;
       |                       ^^^|^^^
       |                          `----- no item 'unknown' found in scope
    ---'
    [E1212] Error: clause of a __TRY written twice
        ,-[ file:///test0.st:14:13 ]
        |
     12 |             __FINALLY
        |             ^^^^|^^^^
        |                 `------ the first __FINALLY is here
        |
     14 |             __FINALLY
        |             ^^^^|^^^^
        |                 `------ this is the second __FINALLY of the __TRY
        |
        | Help: move its statements into the first __FINALLY
        |
        | Note: a __TRY can only have one __FINALLY
    ----'
    ");
}

/// A `__CATCH`, a `__FINALLY` or an `__END_TRY` with no `__TRY` is
/// reported at the keyword: the statements before and after it are read as
/// what they are.
#[rstest]
fn invalid_clause_without_a_try(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION f1 : INT
            f1 := before;
            __CATCH
                f1 := 2;
            f1 := after;
            __FINALLY
                f1 := 3;
            __END_TRY;
        END_FUNCTION

        FUNCTION f2 : INT
            __TRY
                f2 := 1;
            __FINALLY
                f2 := 2;
            __END_TRY;
            __END_TRY;
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0201] Error: unknown name
       ,-[ file:///test0.st:3:19 ]
       |
     3 |             f1 := before;
       |                   ^^^|^^
       |                      `---- no item 'before' found in scope
    ---'
    [E1213] Error: __CATCH, __FINALLY or __END_TRY without a __TRY
       ,-[ file:///test0.st:4:13 ]
       |
     4 |             __CATCH
       |             ^^^|^^^
       |                `----- this __CATCH has no __TRY
       |
       | Help: add a __TRY before it or remove it
       |
       | Note: __CATCH, __FINALLY and __END_TRY are only accepted inside a __TRY
    ---'
    [E0201] Error: unknown name
       ,-[ file:///test0.st:6:19 ]
       |
     6 |             f1 := after;
       |                   ^^|^^
       |                     `---- no item 'after' found in scope
    ---'
    [E1213] Error: __CATCH, __FINALLY or __END_TRY without a __TRY
       ,-[ file:///test0.st:7:13 ]
       |
     7 |             __FINALLY
       |             ^^^^|^^^^
       |                 `------ this __FINALLY has no __TRY
       |
       | Help: add a __TRY before it or remove it
       |
       | Note: __CATCH, __FINALLY and __END_TRY are only accepted inside a __TRY
    ---'
    [E1213] Error: __CATCH, __FINALLY or __END_TRY without a __TRY
       ,-[ file:///test0.st:9:13 ]
       |
     9 |             __END_TRY;
       |             ^^^^|^^^^
       |                 `------ this __END_TRY has no __TRY
       |
       | Help: add a __TRY before it or remove it
       |
       | Note: __CATCH, __FINALLY and __END_TRY are only accepted inside a __TRY
    ---'
    [E1213] Error: __CATCH, __FINALLY or __END_TRY without a __TRY
        ,-[ file:///test0.st:18:13 ]
        |
     18 |             __END_TRY;
        |             ^^^^|^^^^
        |                 `------ this __END_TRY has no __TRY
        |
        | Help: add a __TRY before it or remove it
        |
        | Note: __CATCH, __FINALLY and __END_TRY are only accepted inside a __TRY
    ----'
    ");
}

/// A `__TRY` that is not closed is asked for its `__END_TRY`, and takes no
/// more than its own function: the next one is read.
#[rstest]
fn invalid_try_without_its_end(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION f1 : INT
            __TRY
                f1 := 1;
            __CATCH
                f1 := 2;
        END_FUNCTION

        FUNCTION f2 : INT
            f2 := next;
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0002] Error: missing element
       ,-[ file:///test0.st:6:25 ]
       |
     6 |                 f1 := 2;
       |                         |
       |                         `- missing '__END_TRY'
    ---'
    [E0201] Error: unknown name
        ,-[ file:///test0.st:10:19 ]
        |
     10 |             f2 := next;
        |                   ^^|^
        |                     `--- no item 'next' found in scope
    ----'
    ");
}

/// `__ENDTRY` is reported as the spelling it is, where it read as an
/// unknown variable followed by a missing closer. The statement is still
/// read: the name its `__CATCH` does not know is reported.
#[rstest]
fn invalid_end_try_spelling(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION f1 : INT
            __TRY
                f1 := 1;
            __CATCH
                f1 := unknown;
            __ENDTRY;
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0201] Error: unknown name
       ,-[ file:///test0.st:6:23 ]
       |
     6 |                 f1 := unknown;
       |                       ^^^|^^^
       |                          `----- no item 'unknown' found in scope
    ---'
    [E0030] Error: __ENDTRY for __END_TRY
       ,-[ file:///test0.st:7:13 ]
       |
     7 |             __ENDTRY;
       |             ^^^^|^^^
       |                 `----- '__ENDTRY' does not close a __TRY
       |
       | Help: replace '__ENDTRY' with '__END_TRY'
    ---'
    ");
}

/// The variable a `__CATCH` names is written the message, as the left
/// side of an assignment is: a STRING of any capacity, a member or an
/// element that is one.
#[rstest]
fn a_catch_takes_the_message_in_a_string(mut with_db: RootDatabase) {
    let source = r#"
        TYPE Log : STRUCT last : STRING[20]; END_STRUCT; END_TYPE

        FUNCTION f1 : INT
        VAR
            short : STRING[4];
            log : Log;
            all : ARRAY[0..1] OF STRING;
        END_VAR
            __TRY
                f1 := 1;
            __CATCH(short)
                f1 := 2;
            __END_TRY;
            __TRY
                f1 := 3;
            __CATCH(log.last)
                f1 := 4;
            __END_TRY;
            __TRY
                f1 := 5;
            __CATCH(all[1])
                f1 := 6;
            __END_TRY;
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// What cannot take the message: a variable of another type, a constant,
/// and a member of a constant.
#[rstest]
fn invalid_catch_variable(mut with_db: RootDatabase) {
    let source = r#"
        TYPE Log : STRUCT last : STRING; END_STRUCT; END_TYPE

        FUNCTION f1 : INT
        VAR
            n : INT;
        END_VAR
        VAR CONSTANT
            fixed : STRING := 'x';
            kept : Log := (last := 'y');
        END_VAR
            __TRY
                f1 := 1;
            __CATCH(n)
                f1 := 2;
            __END_TRY;
            __TRY
                f1 := 3;
            __CATCH(fixed)
                f1 := 4;
            __END_TRY;
            __TRY
                f1 := 5;
            __CATCH(kept.last)
                f1 := 6;
            __END_TRY;
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
        ,-[ file:///test0.st:14:21 ]
        |
      6 |             n : INT;
        |             |
        |             `-- 'n' is declared here
        |
     14 |             __CATCH(n)
        |                     |
        |                     `-- expected 'INT', got 'STRING'
        |
        | Note: the variable of a __CATCH receives the message as a STRING
    ----'
    [E0404] Error: write to a constant
        ,-[ file:///test0.st:19:21 ]
        |
      9 |             fixed : STRING := 'x';
        |             ^^|^^
        |               `---- 'fixed' is declared CONSTANT here
        |
     19 |             __CATCH(fixed)
        |                     ^^|^^
        |                       `---- cannot write to constant 'fixed'
        |
        | Help: copy it into a variable to change the copy
        |
        | Note: a CONSTANT keeps the value it is declared with
    ----'
    [E0404] Error: write to a constant
        ,-[ file:///test0.st:24:21 ]
        |
     10 |             kept : Log := (last := 'y');
        |             ^^|^
        |               `--- 'kept' is declared CONSTANT here
        |
     24 |             __CATCH(kept.last)
        |                     ^^^^|^^^^
        |                         `------ cannot write to 'kept.last' in constant 'kept'
        |
        | Help: copy it into a variable to change the copy
        |
        | Note: a CONSTANT keeps the value it is declared with
    ----'
    ");
}

/// The variable is written as the left side of an assignment is, and is
/// refused no more than one: an input of the block, an output of another
/// instance and an input of it are accepted, and compiled. The linter warns
/// of the first two, as it does of the assignment
/// (`linter::input_assignment`, `linter::external_mutation`).
#[rstest]
fn a_catch_variable_of_an_instance_is_accepted(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Counter
        VAR_INPUT
            label : STRING;
        END_VAR
        VAR_OUTPUT
            last : STRING;
        END_VAR
        VAR
            n : INT;
        END_VAR
            __TRY
                n := n + 1;
            __CATCH(label)
                n := 0;
            __END_TRY;
            __TRY
                n := n + 1;
            __CATCH(last)
                n := 0;
            __END_TRY;
        END_FUNCTION_BLOCK

        FUNCTION f1 : INT
        VAR
            c : Counter;
        END_VAR
            __TRY
                f1 := 1;
            __CATCH(c.last)
                f1 := 2;
            __END_TRY;
            __TRY
                f1 := 3;
            __CATCH(c.label)
                f1 := 4;
            __END_TRY;
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}
