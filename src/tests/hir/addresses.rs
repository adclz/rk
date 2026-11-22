// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! What `[addresses]` changes in what a workspace may declare. Counting
//! bytes, two addresses can share bytes with neither holding the other:
//! they are stored together, so they persist together (E1434) and neither
//! has an initial value (E1435). Counted by width, any two addresses are
//! nested or apart, and none of this arises.

use crate::tests::utils::{test_diagnostics, with_config, with_db};
use insta::assert_snapshot;
use rstest::*;

const BY_BYTE: &str = "[addresses]\nnumbering = \"byte\"\n";

/// `first` at `%MW7`, RETAIN, and `second` at `%MW8`, which is not.
const RETAINED_IN_PART: &str = "
PROGRAM P
VAR_EXTERNAL first : WORD; second : WORD; END_VAR
    first := second;
END_PROGRAM

CONFIGURATION C
    VAR_GLOBAL RETAIN
        first AT %MW7 : WORD;
    END_VAR
    VAR_GLOBAL
        second AT %MW8 : WORD;
    END_VAR
    RESOURCE R ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM Main WITH T : P;
    END_RESOURCE
END_CONFIGURATION
";

/// Sharing byte 8, `%MW7` and `%MW8` persist together or not at all: the
/// one that is not RETAIN is refused.
#[rstest]
fn addresses_that_share_bytes_are_retained_alike(mut with_db: db::RootDatabase) {
    let _config = with_config(&mut with_db, BY_BYTE);
    assert_snapshot!(test_diagnostics(&mut with_db, &[RETAINED_IN_PART]), @r"
    [E1434] Error: RETAIN mismatch on shared bytes
        ,-[ file:///test0.st:12:19 ]
        |
      9 |         first AT %MW7 : WORD;
        |         ^^|^^
        |           `---- 'first' is declared here
        |
     12 |         second AT %MW8 : WORD;
        |                   ^^|^
        |                     `--- 'second' is not RETAIN but is stored with 'first', which is
        |
        | Help: declare 'second' RETAIN too
        |
        | Note: addresses that share bytes are stored together, and persist together or not at all
    ----'
    ");
}

/// Counted by width, `%MW7` and `%MW8` are apart, and each persists as it
/// says.
#[rstest]
fn addresses_counted_by_width_are_retained_each_as_it_says(mut with_db: db::RootDatabase) {
    assert_snapshot!(test_diagnostics(&mut with_db, &[RETAINED_IN_PART]), @"");
}

/// `first` at `%MW7` with an initial value, beside `second` at `%MW8`.
const INITIALIZED: &str = "
PROGRAM P
VAR_EXTERNAL first : WORD; second : WORD; END_VAR
    first := second;
END_PROGRAM

CONFIGURATION C
    VAR_GLOBAL
        first AT %MW7 : WORD := WORD#16#1234;
        second AT %MW8 : WORD;
    END_VAR
    RESOURCE R ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM Main WITH T : P;
    END_RESOURCE
END_CONFIGURATION
";

/// An initial value of `%MW7` would set part of `%MW8`: it is refused, and
/// the help names an address that holds both.
#[rstest]
fn an_address_that_shares_bytes_has_no_initial_value(mut with_db: db::RootDatabase) {
    let _config = with_config(&mut with_db, BY_BYTE);
    assert_snapshot!(test_diagnostics(&mut with_db, &[INITIALIZED]), @r"
    [E1435] Error: initial value on shared bytes
        ,-[ file:///test0.st:9:18 ]
        |
      9 |         first AT %MW7 : WORD := WORD#16#1234;
        |                  ^^|^
        |                    `--- '%MW7' shares bytes with '%MW8' and cannot have an initial value
     10 |         second AT %MW8 : WORD;
        |         ^^^|^^
        |            `---- 'second' is declared here
        |
        | Help: locate a variable at '%MD7', which holds all their bytes, and give it the initial value
        |
        | Note: its initial value would set part of '%MW8' too
    ----'
    ");
}

/// Counted by width, `%MW7` and `%MW8` are apart, and each starts from its
/// own value.
#[rstest]
fn an_address_counted_by_width_has_its_initial_value(mut with_db: db::RootDatabase) {
    assert_snapshot!(test_diagnostics(&mut with_db, &[INITIALIZED]), @"");
}

/// A run of addresses sharing bytes longer than the widest address: no
/// address holds them all, and the help says nothing of one.
#[rstest]
fn a_run_no_address_holds_has_no_help(mut with_db: db::RootDatabase) {
    let _config = with_config(&mut with_db, BY_BYTE);
    let source = "
PROGRAM P
VAR_EXTERNAL first : DWORD; END_VAR
    %MD3 := %MD6;
END_PROGRAM

CONFIGURATION C
    VAR_GLOBAL
        first AT %MD0 : DWORD := DWORD#1;
    END_VAR
    RESOURCE R ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM Main WITH T : P;
    END_RESOURCE
END_CONFIGURATION
";
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1435] Error: initial value on shared bytes
       ,-[ file:///test0.st:9:18 ]
       |
     4 |     %MD3 := %MD6;
       |     ^^|^
       |       `--- '%MD3' is mentioned here
       |
     9 |         first AT %MD0 : DWORD := DWORD#1;
       |                  ^^|^
       |                    `--- '%MD0' shares bytes with '%MD3' and cannot have an initial value
       |
       | Note: its initial value would set part of '%MD3' too
    ---'
    ");
}

/// VAR_CONFIG gives `%MW8` to an instance's variable with a starting value,
/// beside a VAR_GLOBAL at `%MW7`: the value would set part of it, and is
/// refused the same way.
#[rstest]
fn a_configured_address_that_shares_bytes_has_no_starting_value(mut with_db: db::RootDatabase) {
    let _config = with_config(&mut with_db, BY_BYTE);
    let source = "
FUNCTION_BLOCK Motor
VAR
    speed AT %M* : WORD;
END_VAR
    speed := WORD#1;
END_FUNCTION_BLOCK

PROGRAM Line
VAR_EXTERNAL other : WORD; END_VAR
VAR
    a : Motor;
END_VAR
    a();
    other := WORD#2;
END_PROGRAM

CONFIGURATION C
    VAR_GLOBAL
        other AT %MW7 : WORD;
    END_VAR
    VAR_CONFIG
        R.L1.a.speed AT %MW8 : WORD := WORD#16#1234;
    END_VAR
    RESOURCE R ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM L1 WITH T : Line;
    END_RESOURCE
END_CONFIGURATION
";
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1435] Error: initial value on shared bytes
        ,-[ file:///test0.st:23:16 ]
        |
     20 |         other AT %MW7 : WORD;
        |         ^^|^^
        |           `---- 'other' is declared here
        |
     23 |         R.L1.a.speed AT %MW8 : WORD := WORD#16#1234;
        |                ^^|^^
        |                  `---- '%MW8' shares bytes with '%MW7' and cannot have an initial value
        |
        | Help: locate a variable at '%MD7', which holds all their bytes, and give it the initial value
        |
        | Note: its initial value would set part of '%MW7' too
    ----'
    ");
}

/// A type that starts every value at its own initial value gives one to an
/// address that shares bytes, as a `:=` would: refused at a declaration and
/// at a VAR_CONFIG entry, with no address to suggest, since what has to go
/// is the type's value.
#[rstest]
fn an_address_that_shares_bytes_has_no_initial_value_from_its_type(mut with_db: db::RootDatabase) {
    let _config = with_config(&mut with_db, BY_BYTE);
    let source = "
TYPE Tally : INT := 5; END_TYPE

FUNCTION_BLOCK Motor
VAR
    speed AT %M* : Tally;
END_VAR
    speed := 1;
END_FUNCTION_BLOCK

PROGRAM Line
VAR_EXTERNAL first : Tally; second : WORD; END_VAR
VAR
    a : Motor;
END_VAR
    a();
    first := 2;
    second := WORD#3;
END_PROGRAM

CONFIGURATION C
    VAR_GLOBAL
        first AT %MW7 : Tally;
        second AT %MW8 : WORD;
    END_VAR
    VAR_CONFIG
        R.L1.a.speed AT %MW9 : Tally;
    END_VAR
    RESOURCE R ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM L1 WITH T : Line;
    END_RESOURCE
END_CONFIGURATION
";
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1435] Error: initial value on shared bytes
        ,-[ file:///test0.st:23:18 ]
        |
     23 |         first AT %MW7 : Tally;
        |                  ^^|^
        |                    `--- '%MW7' shares bytes with '%MW8' and cannot have an initial value
     24 |         second AT %MW8 : WORD;
        |         ^^^|^^
        |            `---- 'second' is declared here
        |
        | Help: declare it with a type that has no initial value
        |
        | Note: the initial value of 'Tally' would set part of '%MW8' too
    ----'
    [E1435] Error: initial value on shared bytes
        ,-[ file:///test0.st:27:16 ]
        |
     24 |         second AT %MW8 : WORD;
        |         ^^^|^^
        |            `---- 'second' is declared here
        |
     27 |         R.L1.a.speed AT %MW9 : Tally;
        |                ^^|^^
        |                  `---- '%MW9' shares bytes with '%MW8' and cannot have an initial value
        |
        | Help: declare it with a type that has no initial value
        |
        | Note: the initial value of 'Tally' would set part of '%MW8' too
    ----'
    ");
}
