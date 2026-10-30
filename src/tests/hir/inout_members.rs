// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! A FUNCTION_BLOCK's `VAR_IN_OUT` of a scalar type is a member: a start
//! value, `RETAIN` or `NON_RETAIN` on its section, a call that leaves it
//! out. Any other `VAR_IN_OUT` is bound to the caller's variable at each
//! call, as before.

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

/// The forms the member takes: a start value, an enum and a subrange, the
/// qualifiers on the section, an instance initializer naming it, and calls
/// that bind it or leave it out. The aggregate beside it is bound at each
/// call.
#[rstest]
fn valid_a_scalar_in_out_is_a_member(mut with_db: RootDatabase) {
    let source = r#"
        TYPE Pt : STRUCT x : INT; y : INT; END_STRUCT; END_TYPE
        TYPE Gear : (Low, High); END_TYPE

        FUNCTION_BLOCK Counter
        VAR_IN_OUT RETAIN
            count : INT := 10;
            gear : Gear := Gear#High;
        END_VAR
        VAR_IN_OUT NON_RETAIN
            step : INT (1..5) := 1;
        END_VAR
        VAR_IN_OUT
            pos : Pt;
        END_VAR
            count := count + step;
            pos.x := count;
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK Holder
        VAR
            c : Counter := (count := 3, gear := Gear#Low);
            n : INT;
            g : Gear;
            s : INT (1..5) := 2;
            p : Pt;
        END_VAR
            c(pos := p);
            c(count := n, pos := p);
            c(count := n, gear := g, step := s, pos := p);
            n := c.count;
        END_FUNCTION_BLOCK
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// A VAR_IN_OUT bound by reference takes no start value: a FUNCTION's, a
/// METHOD's, and a FUNCTION_BLOCK's of an aggregate type or a STRING.
#[rstest]
fn invalid_a_start_value_on_an_in_out_bound_by_reference(mut with_db: RootDatabase) {
    let source = r#"
        TYPE Pt : STRUCT x : INT; y : INT; END_STRUCT; END_TYPE

        FUNCTION fn1 : INT
        VAR_IN_OUT
            x : INT := 1;
        END_VAR
            fn1 := x;
        END_FUNCTION

        FUNCTION_BLOCK Fb
        VAR_IN_OUT
            p : Pt := (x := 1);
            s : STRING := 'a';
        END_VAR
            p.x := p.x + 1;
            s := 'b';
        END_FUNCTION_BLOCK

        CLASS Cl
            METHOD m
            VAR_IN_OUT
                y : INT := 2;
            END_VAR
                y := y + 1;
            END_METHOD
        END_CLASS
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0004] Error: unexpected variable initialization
       ,-[ file:///test0.st:6:21 ]
       |
     6 |             x : INT := 1;
       |                     ^^|^
       |                       `--- an initial value is not allowed here
    ---'
    [E0004] Error: unexpected variable initialization
        ,-[ file:///test0.st:13:20 ]
        |
     13 |             p : Pt := (x := 1);
        |                    ^^^^^|^^^^^
        |                         `------- an initial value is not allowed here
    ----'
    [E0004] Error: unexpected variable initialization
        ,-[ file:///test0.st:14:24 ]
        |
     14 |             s : STRING := 'a';
        |                        ^^^|^^
        |                           `---- an initial value is not allowed here
    ----'
    [E0004] Error: unexpected variable initialization
        ,-[ file:///test0.st:23:25 ]
        |
     23 |                 y : INT := 2;
        |                         ^^|^
        |                           `--- an initial value is not allowed here
    ----'
    ");
}

/// `RETAIN` or `NON_RETAIN` on an in-out section: a FUNCTION_BLOCK's
/// aggregate, STRING or REF_TO is bound by reference (E0906), a FUNCTION
/// keeps nothing at all (E0208), and two qualifiers are E0031 here as
/// anywhere.
#[rstest]
fn invalid_a_qualifier_on_an_in_out_bound_by_reference(mut with_db: RootDatabase) {
    let source = r#"
        TYPE Pt : STRUCT x : INT; y : INT; END_STRUCT; END_TYPE
        TYPE PInt : REF_TO INT; END_TYPE

        FUNCTION_BLOCK Fb
        VAR_IN_OUT RETAIN
            p : Pt;
        END_VAR
        VAR_IN_OUT NON_RETAIN
            s : STRING;
            r : PInt;
        END_VAR
        VAR_IN_OUT RETAIN NON_RETAIN
            n : INT;
        END_VAR
            p.x := p.x + 1;
            s := 'b';
            r^ := n;
            n := n + 1;
        END_FUNCTION_BLOCK

        FUNCTION fn1 : INT
        VAR_IN_OUT RETAIN
            x : INT;
        END_VAR
            fn1 := x;
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0906] Error: qualifier on a VAR_IN_OUT bound by reference
       ,-[ file:///test0.st:7:13 ]
       |
     7 |             p : Pt;
       |             |
       |             `-- 'p' is bound to the caller's variable at each call
       |
       | Help: drop the RETAIN
       |
       | Note: a FUNCTION_BLOCK keeps a VAR_IN_OUT of a scalar type as a member, and nothing of a STRING, a reference or an aggregate
    ---'
    [E0906] Error: qualifier on a VAR_IN_OUT bound by reference
        ,-[ file:///test0.st:10:13 ]
        |
     10 |             s : STRING;
        |             |
        |             `-- 's' is bound to the caller's variable at each call
        |
        | Help: drop the NON_RETAIN
        |
        | Note: a FUNCTION_BLOCK keeps a VAR_IN_OUT of a scalar type as a member, and nothing of a STRING, a reference or an aggregate
    ----'
    [E0906] Error: qualifier on a VAR_IN_OUT bound by reference
        ,-[ file:///test0.st:11:13 ]
        |
     11 |             r : PInt;
        |             |
        |             `-- 'r' is bound to the caller's variable at each call
        |
        | Help: drop the NON_RETAIN
        |
        | Note: a FUNCTION_BLOCK keeps a VAR_IN_OUT of a scalar type as a member, and nothing of a STRING, a reference or an aggregate
    ----'
    [E0031] Error: two qualifiers on a declaration
        ,-[ file:///test0.st:13:27 ]
        |
     13 |         VAR_IN_OUT RETAIN NON_RETAIN
        |                    ^^^|^^ ^^^^^|^^^^
        |                       `--------------- 'RETAIN' is the first
        |                                |
        |                                `------ 'NON_RETAIN' is a second qualifier on 'VAR_IN_OUT'
        |
        | Help: keep the one you mean
        |
        | Note: a declaration takes one qualifier, CONSTANT, RETAIN or NON_RETAIN
    ----'
    [E0208] Error: RETAIN in a stateless POU
        ,-[ file:///test0.st:24:13 ]
        |
     24 |             x : INT;
        |             |
        |             `-- 'x' cannot be RETAIN: a FUNCTION is stateless
        |
        | Note: only a variable of a FUNCTION_BLOCK, CLASS or PROGRAM, or a VAR_GLOBAL, has storage to retain
    ----'
    ");
}

/// A call binds every VAR_IN_OUT bound by reference: a FUNCTION_BLOCK's
/// aggregate and reference ones and a FUNCTION's scalar one. The member
/// still takes a variable (E0806), since the value comes back into it.
#[rstest]
fn invalid_a_call_binds_every_in_out_bound_by_reference(mut with_db: RootDatabase) {
    let source = r#"
        TYPE Pt : STRUCT x : INT; y : INT; END_STRUCT; END_TYPE
        TYPE PInt : REF_TO INT; END_TYPE

        FUNCTION_BLOCK Counter
        VAR_IN_OUT
            count : INT;
            pos : Pt;
            r : PInt;
        END_VAR
            count := count + 1;
            pos.x := count;
            r^ := count;
        END_FUNCTION_BLOCK

        FUNCTION Bump : INT
        VAR_IN_OUT
            v : INT;
        END_VAR
            v := v + 1;
            Bump := v;
        END_FUNCTION

        FUNCTION_BLOCK Holder
        VAR
            c : Counter;
            n : INT;
            p : Pt;
            pr : PInt;
        END_VAR
            c(count := n);
            c(count := n + 1, pos := p, r := pr);
            n := Bump();
        END_FUNCTION_BLOCK
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0802] Error: missing required parameter
        ,-[ file:///test0.st:31:13 ]
        |
      8 |             pos : Pt;
        |             ^|^
        |              `--- parameter 'pos' is declared here
      9 |             r : PInt;
        |             |
        |             `-- parameter 'r' is declared here
        |
     31 |             c(count := n);
        |             |
        |             `-- call to 'Counter' is missing 2 required parameters: 'pos', 'r'
        |
        | Note: a VAR_IN_OUT of a FUNCTION or a METHOD, or of a STRING, a reference or an aggregate, is bound to the caller's variable at each call
    ----'
    [E0806] Error: VAR_IN_OUT argument not a variable
        ,-[ file:///test0.st:32:24 ]
        |
      7 |             count : INT;
        |             ^^|^^
        |               `---- parameter 'count' is declared here
        |
     32 |             c(count := n + 1, pos := p, r := pr);
        |                        ^^|^^
        |                          `---- VAR_IN_OUT parameter 'count' of 'Counter' requires a variable, not a value
        |
        | Note: a FUNCTION_BLOCK copies a scalar VAR_IN_OUT back into its argument when the call ends, and a literal, an expression or a call result has nowhere to receive it
    ----'
    [E0802] Error: missing required parameter
        ,-[ file:///test0.st:33:18 ]
        |
     18 |             v : INT;
        |             |
        |             `-- parameter 'v' is declared here
        |
     33 |             n := Bump();
        |                  ^^|^
        |                    `--- call to 'Bump' is missing 1 required parameter: 'v'
        |
        | Note: a VAR_IN_OUT of a FUNCTION or a METHOD, or of a STRING, a reference or an aggregate, is bound to the caller's variable at each call
    ----'
    ");
}

/// A CONSTANT bound to a member is refused as it is for any VAR_IN_OUT: the
/// copy back would write it.
#[rstest]
fn invalid_a_constant_bound_to_a_member(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Counter
        VAR_IN_OUT
            count : INT;
        END_VAR
            count := count + 1;
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK Holder
        VAR
            c : Counter;
        END_VAR
        VAR CONSTANT
            k : INT := 1;
        END_VAR
            c(count := k);
        END_FUNCTION_BLOCK
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0404] Error: write to a constant
        ,-[ file:///test0.st:16:24 ]
        |
     14 |             k : INT := 1;
        |             |
        |             `-- 'k' is declared CONSTANT here
        |
     16 |             c(count := k);
        |                        |
        |                        `-- cannot pass constant 'k' to a VAR_IN_OUT
        |
        | Help: pass it to a VAR_INPUT, or copy it into a variable and pass that
        |
        | Note: a VAR_IN_OUT could change it
    ----'
    ");
}

/// An input location bound to a member: the copy back would write it, as
/// an assignment to it would (E1419).
#[rstest]
fn invalid_an_input_location_bound_to_a_member(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Flip
        VAR_IN_OUT v : BOOL; END_VAR
            v := NOT v;
        END_FUNCTION_BLOCK

        PROGRAM P
        VAR f : Flip; END_VAR
            f(v := %IX0.3);
        END_PROGRAM
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1419] Error: write to an input location
       ,-[ file:///test0.st:9:20 ]
       |
     9 |             f(v := %IX0.3);
       |                    ^^^|^^
       |                       `---- '%IX0.3' is an input: it is written by the host, not by the program
       |
       | Note: the host copies the input image in before each scan: the rest of this scan would read a value the input never had, and the next scan overwrites it
    ---'
    ");
}
