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
/// it is no E0904 in a `VAR RETAIN` of the struct.
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
        END_TYPE

        FUNCTION_BLOCK Fb
        VAR
            c : Counter;
        END_VAR
        VAR RETAIN
            r : Pair;
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

        PROGRAM P
        VAR
            fb : Fb;
            c : Counter;
            p : REF_TO Counter;
            u : Use;
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
       | Help: mark 'r' NON_RETAIN in the struct that declares it and set it in the first scan
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
        | Help: keep references out of RETAIN (NON_RETAIN on a member) and set them in the first scan
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
       |                        `-- 'a' is a reference marked RETAIN: its address does not survive a new build
       |
       | Help: leave the qualifier off 'a' and set the reference in the first scan
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
       | Help: keep references out of RETAIN (NON_RETAIN on a member) and set them in the first scan
       |
       | Note: a warm start restores the address even where a new build moved its target
    ---'
    ");
}

/// A second qualifier on an element is a syntax error.
#[rstest]
fn invalid_a_doubled_qualifier(mut with_db: RootDatabase) {
    let source = r#"
        TYPE
            S : STRUCT
                RETAIN NON_RETAIN x : DINT;
                RETAIN RETAIN y : DINT;
            END_STRUCT;
        END_TYPE
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0001] Error: syntax error
       ,-[ file:///test0.st:4:17 ]
       |
     4 |                 RETAIN NON_RETAIN x : DINT;
       |                 ^^^|^^
       |                    `---- unexpected token(s): 'RETAIN'
    ---'
    [E0001] Error: syntax error
       ,-[ file:///test0.st:5:17 ]
       |
     5 |                 RETAIN RETAIN y : DINT;
       |                 ^^^|^^
       |                    `---- unexpected token(s): 'RETAIN'
    ---'
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
       | Help: keep references out of RETAIN (NON_RETAIN on a member) and set them in the first scan
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
        | Help: keep references out of RETAIN (NON_RETAIN on a member) and set them in the first scan
        |
        | Note: a warm start restores the address even where a new build moved its target
    ----'
    ");
}
