// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! `RETAIN` and `NON_RETAIN` on a struct's elements.

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

/// The qualifier goes before the element's name, in a TYPE or a struct
/// written in place, nested or not, and a variable of the type takes any
/// qualifier of its own as before. Where nothing persists, a FUNCTION's
/// local, it says nothing. A `NON_RETAIN` reference is never restored, so
/// it is no E0904 in a `VAR RETAIN` of the struct, and neither is one
/// under a `NON_RETAIN` field or member holding it.
#[rstest]
fn valid_a_qualifier_on_a_struct_element(mut with_db: RootDatabase) {
    let source = r#"
        TYPE
            Counter : STRUCT
                RETAIN count : DINT;
                NON_RETAIN scratch : INT;
                NON_RETAIN link : REF_TO INT;
                name : STRING[8];
            END_STRUCT;
            Pair : STRUCT
                a : Counter;
                RETAIN b : Counter;
                NON_RETAIN c : Counter;
                RETAIN rows : ARRAY[0..1] OF Counter;
            END_STRUCT;
            Holder : STRUCT
                r : REF_TO INT;
            END_STRUCT;
        END_TYPE

        FUNCTION_BLOCK Fb
        VAR
            c : Counter;
        END_VAR
        VAR RETAIN
            r : Pair;
            v : STRUCT
                NON_RETAIN h : Holder;
            END_STRUCT;
        END_VAR
        VAR NON_RETAIN
            n : Pair;
        END_VAR
        END_FUNCTION_BLOCK

        FUNCTION f : DINT
        VAR
            c : Counter;
        END_VAR
            f := c.count;
        END_FUNCTION

        FUNCTION_BLOCK Use
        VAR_IN_OUT
            c : Counter;
        END_VAR
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK Keeper
        VAR NON_RETAIN
            h : Holder;
        END_VAR
        END_FUNCTION_BLOCK

        PROGRAM P
        VAR
            fb : Fb;
            c : Counter;
            p : REF_TO Counter;
            u : Use;
        END_VAR
        VAR RETAIN
            k : Keeper;
        END_VAR
            c.count := c.count + 1;
            p := REF(c);
            u(c := c);
        END_PROGRAM

        CONFIGURATION Cfg
        VAR_GLOBAL
            g : Counter;
            h : STRUCT
                RETAIN kept : DINT;
                lost : DINT;
            END_STRUCT;
        END_VAR
        VAR_GLOBAL RETAIN
            all : Counter;
        END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// A `RETAIN` element persists on its own, so a reference in it, itself or
/// down a nested struct, is E0904 as it is in a RETAIN variable. An
/// unqualified reference beside it is not, until a `VAR RETAIN` of the
/// struct keeps it: that reports the unqualified one and not the two
/// already reported at the type.
#[rstest]
fn invalid_a_retained_element_holding_a_reference(mut with_db: RootDatabase) {
    let source = r#"
        TYPE
            Holder : STRUCT
                r : REF_TO INT;
            END_STRUCT;
            Links : STRUCT
                RETAIN p : REF_TO INT;
                RETAIN inner : Holder;
                q : REF_TO INT;
            END_STRUCT;
        END_TYPE

        PROGRAM P
        VAR RETAIN
            l : Links;
        END_VAR
        VAR
            x : INT;
        END_VAR
            x := 1;
        END_PROGRAM
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0904] Error: reference in RETAIN storage
       ,-[ file:///test0.st:7:24 ]
       |
     7 |                 RETAIN p : REF_TO INT;
       |                        |
       |                        `-- 'p' is a reference marked RETAIN: its address does not survive a new build
       |
       | Help: leave the qualifier off 'p' and set the reference in the first scan
       |
       | Note: a warm start restores the address even where a new build moved its target
    ---'
    [E0904] Error: reference in RETAIN storage
       ,-[ file:///test0.st:8:24 ]
       |
     8 |                 RETAIN inner : Holder;
       |                        ^^|^^
       |                          `---- 'inner' is marked RETAIN and holds the reference 'inner.r': its address does not survive a new build
       |
       | Help: mark 'r' NON_RETAIN in 'Holder' and set it in the first scan
       |
       | Note: a warm start restores the address even where a new build moved its target
    ---'
    [E0904] Error: reference in RETAIN storage
        ,-[ file:///test0.st:15:13 ]
        |
     15 |             l : Links;
        |             |
        |             `-- 'l.q' is a reference in RETAIN storage: its address does not survive a new build
        |
        | Help: mark 'q' NON_RETAIN in 'Links' and set it in the first scan
        |
        | Note: a warm start restores the address even where a new build moved its target
    ----'
    ");
}

/// E0904 reaches a marked element by every route a reference can take: in
/// a struct written in place, through an alias of a reference, and in an
/// array of them.
#[rstest]
fn invalid_a_retained_element_by_other_routes(mut with_db: RootDatabase) {
    let source = r#"
        TYPE
            PInt : REF_TO INT;
            Aliased : STRUCT
                RETAIN p : PInt;
            END_STRUCT;
            Several : STRUCT
                RETAIN a : ARRAY[0..1] OF REF_TO INT;
            END_STRUCT;
        END_TYPE

        PROGRAM P
        VAR
            x : INT;
        END_VAR
            x := 1;
        END_PROGRAM

        CONFIGURATION Cfg
        VAR_GLOBAL
            h : STRUCT
                RETAIN p : REF_TO INT;
            END_STRUCT;
        END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0904] Error: reference in RETAIN storage
       ,-[ file:///test0.st:5:24 ]
       |
     5 |                 RETAIN p : PInt;
       |                        |
       |                        `-- 'p' is a reference marked RETAIN: its address does not survive a new build
       |
       | Help: leave the qualifier off 'p' and set the reference in the first scan
       |
       | Note: a warm start restores the address even where a new build moved its target
    ---'
    [E0904] Error: reference in RETAIN storage
       ,-[ file:///test0.st:8:24 ]
       |
     8 |                 RETAIN a : ARRAY[0..1] OF REF_TO INT;
       |                        |
       |                        `-- 'a' is marked RETAIN and holds references: their addresses do not survive a new build
       |
       | Help: leave the qualifier off 'a' and set them in the first scan
       |
       | Note: a warm start restores the address even where a new build moved its target
    ---'
    [E0904] Error: reference in RETAIN storage
        ,-[ file:///test0.st:22:24 ]
        |
     22 |                 RETAIN p : REF_TO INT;
        |                        |
        |                        `-- 'p' is a reference marked RETAIN: its address does not survive a new build
        |
        | Help: leave the qualifier off 'p' and set the reference in the first scan
        |
        | Note: a warm start restores the address even where a new build moved its target
    ----'
    ");
}

/// A `RETAIN` reference to a marked struct is refused as any RETAIN
/// reference is: the struct's marks say nothing about the pointer, and a
/// plain one is accepted.
#[rstest]
fn invalid_a_retained_reference_to_a_marked_struct(mut with_db: RootDatabase) {
    let source = r#"
        TYPE Counter : STRUCT RETAIN count : DINT; END_STRUCT; END_TYPE

        PROGRAM P
        VAR RETAIN
            p : REF_TO Counter;
        END_VAR
        VAR
            c : Counter;
        END_VAR
            p := REF(c);
        END_PROGRAM
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0904] Error: reference in RETAIN storage
       ,-[ file:///test0.st:6:13 ]
       |
     6 |             p : REF_TO Counter;
       |             |
       |             `-- 'p' is a reference in RETAIN storage: its address does not survive a new build
       |
       | Help: move 'p' out of RETAIN and set it in the first scan
       |
       | Note: a warm start restores the address even where a new build moved its target
    ---'
    ");
}

/// A second qualifier is E0031, both labeled, and the first is kept: on a
/// struct element, on a section of any kind, and on a program instance.
#[rstest]
fn invalid_a_doubled_qualifier(mut with_db: RootDatabase) {
    let source = r#"
        TYPE
            S : STRUCT
                RETAIN NON_RETAIN x : DINT;
                RETAIN RETAIN y : DINT;
            END_STRUCT;
        END_TYPE

        FUNCTION_BLOCK Fb
        VAR_INPUT RETAIN NON_RETAIN
            a : INT;
        END_VAR
        VAR RETAIN NON_RETAIN
            b : INT;
        END_VAR
        VAR NON_RETAIN RETAIN
            c : INT;
        END_VAR
            c := a + b;
        END_FUNCTION_BLOCK

        PROGRAM P
        VAR_OUTPUT NON_RETAIN RETAIN
            o : INT;
        END_VAR
            o := 1;
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM RETAIN NON_RETAIN P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0031] Error: two qualifiers on a declaration
       ,-[ file:///test0.st:4:24 ]
       |
     4 |                 RETAIN NON_RETAIN x : DINT;
       |                 ^^^|^^ ^^^^^|^^^^
       |                    `--------------- 'RETAIN' is the first
       |                             |
       |                             `------ 'NON_RETAIN' is a second qualifier on 'x'
       |
       | Help: keep the one you mean
       |
       | Note: a declaration takes one qualifier, CONSTANT, RETAIN or NON_RETAIN
    ---'
    [E0031] Error: two qualifiers on a declaration
       ,-[ file:///test0.st:5:24 ]
       |
     5 |                 RETAIN RETAIN y : DINT;
       |                 ^^^|^^ ^^^|^^
       |                    `----------- 'RETAIN' is the first
       |                           |
       |                           `---- 'RETAIN' is a second qualifier on 'y'
       |
       | Help: keep the one you mean
       |
       | Note: a declaration takes one qualifier, CONSTANT, RETAIN or NON_RETAIN
    ---'
    [E0031] Error: two qualifiers on a declaration
        ,-[ file:///test0.st:10:26 ]
        |
     10 |         VAR_INPUT RETAIN NON_RETAIN
        |                   ^^^|^^ ^^^^^|^^^^
        |                      `--------------- 'RETAIN' is the first
        |                               |
        |                               `------ 'NON_RETAIN' is a second qualifier on 'VAR_INPUT'
        |
        | Help: keep the one you mean
        |
        | Note: a declaration takes one qualifier, CONSTANT, RETAIN or NON_RETAIN
    ----'
    [E0031] Error: two qualifiers on a declaration
        ,-[ file:///test0.st:13:20 ]
        |
     13 |         VAR RETAIN NON_RETAIN
        |             ^^^|^^ ^^^^^|^^^^
        |                `--------------- 'RETAIN' is the first
        |                         |
        |                         `------ 'NON_RETAIN' is a second qualifier on 'VAR'
        |
        | Help: keep the one you mean
        |
        | Note: a declaration takes one qualifier, CONSTANT, RETAIN or NON_RETAIN
    ----'
    [E0031] Error: two qualifiers on a declaration
        ,-[ file:///test0.st:16:24 ]
        |
     16 |         VAR NON_RETAIN RETAIN
        |             ^^^^^|^^^^ ^^^|^^
        |                  `------------- 'NON_RETAIN' is the first
        |                           |
        |                           `---- 'RETAIN' is a second qualifier on 'VAR'
        |
        | Help: keep the one you mean
        |
        | Note: a declaration takes one qualifier, CONSTANT, RETAIN or NON_RETAIN
    ----'
    [E0031] Error: two qualifiers on a declaration
        ,-[ file:///test0.st:23:31 ]
        |
     23 |         VAR_OUTPUT NON_RETAIN RETAIN
        |                    ^^^^^|^^^^ ^^^|^^
        |                         `------------- 'NON_RETAIN' is the first
        |                                  |
        |                                  `---- 'RETAIN' is a second qualifier on 'VAR_OUTPUT'
        |
        | Help: keep the one you mean
        |
        | Note: a declaration takes one qualifier, CONSTANT, RETAIN or NON_RETAIN
    ----'
    [E0031] Error: two qualifiers on a declaration
        ,-[ file:///test0.st:32:32 ]
        |
     32 |                 PROGRAM RETAIN NON_RETAIN P1 WITH T : P;
        |                         ^^^|^^ ^^^^^|^^^^
        |                            `--------------- 'RETAIN' is the first
        |                                     |
        |                                     `------ 'NON_RETAIN' is a second qualifier on 'P1'
        |
        | Help: keep the one you mean
        |
        | Note: a declaration takes one qualifier, CONSTANT, RETAIN or NON_RETAIN
    ----'
    ");
}

/// E0904 reaches a nested instance exactly where the retain map keeps it:
/// a plain reference in the block a `VAR RETAIN` holder holds is kept and
/// reported, and nothing is under a `NON_RETAIN` holder, a `NON_RETAIN`
/// member or a plain holder.
#[rstest]
fn invalid_a_reference_in_a_block_a_retained_instance_holds(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Inner
        VAR RETAIN kept : DINT; END_VAR
        VAR q : REF_TO INT; plain : DINT; END_VAR
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK Wrapper
        VAR inner : Inner; END_VAR
        VAR NON_RETAIN ni : Inner; END_VAR
        END_FUNCTION_BLOCK

        PROGRAM P
        VAR RETAIN w : Wrapper; END_VAR
        VAR NON_RETAIN nw : Wrapper; END_VAR
        VAR w2 : Wrapper; END_VAR
        END_PROGRAM
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0904] Error: reference in RETAIN storage
        ,-[ file:///test0.st:13:20 ]
        |
     13 |         VAR RETAIN w : Wrapper; END_VAR
        |                    |
        |                    `-- 'w.inner.q' is a reference in RETAIN storage: its address does not survive a new build
        |
        | Help: mark 'q' NON_RETAIN in 'Inner' and set it in the first scan
        |
        | Note: a warm start restores the address even where a new build moved its target
    ----'
    ");
}

/// The advice names where to mark the reference `NON_RETAIN`: the TYPE or
/// the block declaring it, the program itself under `PROGRAM RETAIN`, or
/// the struct written in place. An array of references is said to hold
/// them, wherever it is, and an array crossed on the way is written with
/// an index letter per dimension.
#[rstest]
fn invalid_a_retained_reference_is_told_where_to_mark_it(mut with_db: RootDatabase) {
    let source = r#"
        TYPE
            PInt : REF_TO INT;
            Several : STRUCT
                a : ARRAY[0..1] OF PInt;
            END_STRUCT;
            Row : ARRAY[0..1] OF Several;
            Marked : STRUCT
                RETAIN many : Several;
                RETAIN inner : STRUCT
                    r : PInt;
                END_STRUCT;
            END_STRUCT;
        END_TYPE

        PROGRAM P
        VAR RETAIN
            arr : ARRAY[0..1] OF PInt;
        END_VAR
        VAR
            x : INT;
        END_VAR
            x := 1;
        END_PROGRAM

        PROGRAM Q
        VAR
            r : PInt;
            held : ARRAY[0..1] OF PInt;
            grid : ARRAY[0..1] OF Row;
            plane : ARRAY[0..1, 0..1] OF Several;
            y : INT;
        END_VAR
            y := 1;
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
                PROGRAM RETAIN Q1 WITH T : Q;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0904] Error: reference in RETAIN storage
       ,-[ file:///test0.st:9:24 ]
       |
     9 |                 RETAIN many : Several;
       |                        ^^|^
       |                          `--- 'many' is marked RETAIN and holds references in 'many.a': their addresses do not survive a new build
       |
       | Help: mark 'a' NON_RETAIN in 'Several' and set them in the first scan
       |
       | Note: a warm start restores the address even where a new build moved its target
    ---'
    [E0904] Error: reference in RETAIN storage
        ,-[ file:///test0.st:10:24 ]
        |
     10 |                 RETAIN inner : STRUCT
        |                        ^^|^^
        |                          `---- 'inner' is marked RETAIN and holds the reference 'inner.r': its address does not survive a new build
        |
        | Help: mark 'r' NON_RETAIN in the struct that declares it and set it in the first scan
        |
        | Note: a warm start restores the address even where a new build moved its target
    ----'
    [E0904] Error: reference in RETAIN storage
        ,-[ file:///test0.st:18:13 ]
        |
     18 |             arr : ARRAY[0..1] OF PInt;
        |             ^|^
        |              `--- 'arr' holds references in RETAIN storage: their addresses do not survive a new build
        |
        | Help: move 'arr' out of RETAIN and set them in the first scan
        |
        | Note: a warm start restores the address even where a new build moved its target
    ----'
    [E0904] Error: reference in RETAIN storage
        ,-[ file:///test0.st:41:32 ]
        |
     28 |             r : PInt;
        |             |
        |             `-- 'r' is declared here
        |
     41 |                 PROGRAM RETAIN Q1 WITH T : Q;
        |                                ^|
        |                                 `-- PROGRAM RETAIN 'Q1' keeps 'r', a reference: its address does not survive a new build
        |
        | Help: mark 'r' NON_RETAIN in 'Q' and set it in the first scan
        |
        | Note: a warm start restores the address even where a new build moved its target
    ----'
    [E0904] Error: reference in RETAIN storage
        ,-[ file:///test0.st:41:32 ]
        |
     29 |             held : ARRAY[0..1] OF PInt;
        |             ^^|^
        |               `--- 'held' is declared here
        |
     41 |                 PROGRAM RETAIN Q1 WITH T : Q;
        |                                ^|
        |                                 `-- PROGRAM RETAIN 'Q1' keeps 'held', which holds references: their addresses do not survive a new build
        |
        | Help: mark 'held' NON_RETAIN in 'Q' and set them in the first scan
        |
        | Note: a warm start restores the address even where a new build moved its target
    ----'
    [E0904] Error: reference in RETAIN storage
        ,-[ file:///test0.st:41:32 ]
        |
     30 |             grid : ARRAY[0..1] OF Row;
        |             ^^|^
        |               `--- 'grid' is declared here
        |
     41 |                 PROGRAM RETAIN Q1 WITH T : Q;
        |                                ^|
        |                                 `-- PROGRAM RETAIN 'Q1' keeps 'grid[i][j].a', which holds references: their addresses do not survive a new build
        |
        | Help: mark 'a' NON_RETAIN in 'Several' and set them in the first scan
        |
        | Note: a warm start restores the address even where a new build moved its target
    ----'
    [E0904] Error: reference in RETAIN storage
        ,-[ file:///test0.st:41:32 ]
        |
     31 |             plane : ARRAY[0..1, 0..1] OF Several;
        |             ^^|^^
        |               `---- 'plane' is declared here
        |
     41 |                 PROGRAM RETAIN Q1 WITH T : Q;
        |                                ^|
        |                                 `-- PROGRAM RETAIN 'Q1' keeps 'plane[i, j].a', which holds references: their addresses do not survive a new build
        |
        | Help: mark 'a' NON_RETAIN in 'Several' and set them in the first scan
        |
        | Note: a warm start restores the address even where a new build moved its target
    ----'
    ");
}

/// The same holds for a block's members: a `VAR RETAIN p` in the block is
/// reported there, and a `VAR RETAIN` instance of it reports the plain `q`
/// and not `p` again.
#[rstest]
fn invalid_a_retained_instance_reports_the_plain_reference_only(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Fb
        VAR RETAIN
            p : REF_TO INT;
        END_VAR
        VAR
            q : REF_TO INT;
            x : INT;
        END_VAR
            p := REF(x); q := REF(x);
        END_FUNCTION_BLOCK

        PROGRAM P
        VAR RETAIN
            fb : Fb;
        END_VAR
            fb();
        END_PROGRAM
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0904] Error: reference in RETAIN storage
       ,-[ file:///test0.st:4:13 ]
       |
     4 |             p : REF_TO INT;
       |             |
       |             `-- 'p' is a reference in RETAIN storage: its address does not survive a new build
       |
       | Help: move 'p' out of RETAIN and set it in the first scan
       |
       | Note: a warm start restores the address even where a new build moved its target
    ---'
    [E0904] Error: reference in RETAIN storage
        ,-[ file:///test0.st:15:13 ]
        |
     15 |             fb : Fb;
        |             ^|
        |              `-- 'fb.q' is a reference in RETAIN storage: its address does not survive a new build
        |
        | Help: mark 'q' NON_RETAIN in 'Fb' and set it in the first scan
        |
        | Note: a warm start restores the address even where a new build moved its target
    ----'
    ");
}

/// `CONSTANT` is a qualifier too: with a second one on a `VAR` or a
/// `VAR_GLOBAL`, E0031 keeps the first, where a `RETAIN` after `CONSTANT`
/// would otherwise keep nothing in silence.
#[rstest]
fn invalid_a_constant_with_a_second_qualifier(mut with_db: RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR CONSTANT RETAIN
            a : INT := 1;
        END_VAR
        VAR RETAIN CONSTANT
            b : INT := 1;
        END_VAR
        VAR CONSTANT CONSTANT
            c : INT := 1;
        END_VAR
        VAR_EXTERNAL CONSTANT
            g : INT;
        END_VAR
        VAR_EXTERNAL
            h : INT;
            i : INT;
        END_VAR
        VAR x : INT; END_VAR
            x := a + b + c + g + h + i;
        END_PROGRAM

        CONFIGURATION Cfg
        VAR_GLOBAL CONSTANT RETAIN
            g : INT := 1;
        END_VAR
        VAR_GLOBAL RETAIN CONSTANT
            h : INT := 1;
        END_VAR
        VAR_GLOBAL RETAIN NON_RETAIN
            i : INT := 1;
        END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0031] Error: two qualifiers on a declaration
       ,-[ file:///test0.st:3:22 ]
       |
     3 |         VAR CONSTANT RETAIN
       |             ^^^^|^^^ ^^^|^^
       |                 `------------ 'CONSTANT' is the first
       |                         |
       |                         `---- 'RETAIN' is a second qualifier on 'VAR'
       |
       | Help: keep the one you mean
       |
       | Note: a declaration takes one qualifier, CONSTANT, RETAIN or NON_RETAIN
    ---'
    [E0031] Error: two qualifiers on a declaration
       ,-[ file:///test0.st:6:20 ]
       |
     6 |         VAR RETAIN CONSTANT
       |             ^^^|^^ ^^^^|^^^
       |                `------------- 'RETAIN' is the first
       |                        |
       |                        `----- 'CONSTANT' is a second qualifier on 'VAR'
       |
       | Help: keep the one you mean
       |
       | Note: a declaration takes one qualifier, CONSTANT, RETAIN or NON_RETAIN
    ---'
    [E0031] Error: two qualifiers on a declaration
       ,-[ file:///test0.st:9:22 ]
       |
     9 |         VAR CONSTANT CONSTANT
       |             ^^^^|^^^ ^^^^|^^^
       |                 `-------------- 'CONSTANT' is the first
       |                          |
       |                          `----- 'CONSTANT' is a second qualifier on 'VAR'
       |
       | Help: keep the one you mean
       |
       | Note: a declaration takes one qualifier, CONSTANT, RETAIN or NON_RETAIN
    ---'
    [E0031] Error: two qualifiers on a declaration
        ,-[ file:///test0.st:24:29 ]
        |
     24 |         VAR_GLOBAL CONSTANT RETAIN
        |                    ^^^^|^^^ ^^^|^^
        |                        `------------ 'CONSTANT' is the first
        |                                |
        |                                `---- 'RETAIN' is a second qualifier on 'VAR_GLOBAL'
        |
        | Help: keep the one you mean
        |
        | Note: a declaration takes one qualifier, CONSTANT, RETAIN or NON_RETAIN
    ----'
    [E0031] Error: two qualifiers on a declaration
        ,-[ file:///test0.st:27:27 ]
        |
     27 |         VAR_GLOBAL RETAIN CONSTANT
        |                    ^^^|^^ ^^^^|^^^
        |                       `------------- 'RETAIN' is the first
        |                               |
        |                               `----- 'CONSTANT' is a second qualifier on 'VAR_GLOBAL'
        |
        | Help: keep the one you mean
        |
        | Note: a declaration takes one qualifier, CONSTANT, RETAIN or NON_RETAIN
    ----'
    [E0031] Error: two qualifiers on a declaration
        ,-[ file:///test0.st:30:27 ]
        |
     30 |         VAR_GLOBAL RETAIN NON_RETAIN
        |                    ^^^|^^ ^^^^^|^^^^
        |                       `--------------- 'RETAIN' is the first
        |                                |
        |                                `------ 'NON_RETAIN' is a second qualifier on 'VAR_GLOBAL'
        |
        | Help: keep the one you mean
        |
        | Note: a declaration takes one qualifier, CONSTANT, RETAIN or NON_RETAIN
    ----'
    ");
}
