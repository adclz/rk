use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

// X = BOOL
#[rstest]
fn mismatch_bool(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        test: REAL;
    END_VAR

    test := %IX0.0

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:7:13 ]
       |
     4 |         test: REAL;
       |         ^^|^
       |           `--- type is declared by variable 'test' here
       |
     7 |     test := %IX0.0
       |             ^^^|^^
       |                `---- expected 'REAL', got 'BOOL'
    ---'
    ");
}

// B = BYTE
#[rstest]
fn mismatch_byte(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        test: REAL;
    END_VAR

    test := %IB0.0

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:7:13 ]
       |
     4 |         test: REAL;
       |         ^^|^
       |           `--- type is declared by variable 'test' here
       |
     7 |     test := %IB0.0
       |             ^^^|^^
       |                `---- expected 'REAL', got 'BYTE'
    ---'
    ");
}

// W = WORD
#[rstest]
fn mismatch_word(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        test: REAL;
    END_VAR

    test := %IW0

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:7:13 ]
       |
     4 |         test: REAL;
       |         ^^|^
       |           `--- type is declared by variable 'test' here
       |
     7 |     test := %IW0
       |             ^^|^
       |               `--- expected 'REAL', got 'WORD'
    ---'
    ");
}

// D = DWORD
#[rstest]
fn mismatch_dword(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        test: REAL;
    END_VAR

    test := %ID0

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:7:13 ]
       |
     4 |         test: REAL;
       |         ^^|^
       |           `--- type is declared by variable 'test' here
       |
     7 |     test := %ID0
       |             ^^|^
       |               `--- expected 'REAL', got 'DWORD'
       |               |
       |               `--- consider explicitly casting with 'DWORD_TO_REAL(%ID0)'
       |
       | Help: insert explicit cast 'DWORD_TO_REAL(%ID0)'
    ---'
    ");
}

// L = LWORD
#[rstest]
fn mismatch_lword(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        test: REAL;
    END_VAR

    test := %IL0

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:7:13 ]
       |
     4 |         test: REAL;
       |         ^^|^
       |           `--- type is declared by variable 'test' here
       |
     7 |     test := %IL0
       |             ^^|^
       |               `--- expected 'REAL', got 'LWORD'
    ---'
    ");
}

#[rstest]
fn valid_multibits(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        Bo: BOOL;
        Byv: BYTE;
        Wo: WORD;
        Dwv: DWORD;
        Lo: LWORD;
    END_VAR

    Bo:= Byv.%X0; // bit 0 of Byv
    Bo:= Byv.7; // bit 7 of Byv; %X is the default and may be omitted.
    Bo:= Lo.63 // bit 63 of Lo;
    Byv:= Wo.%B1; // byte 1 of Wo;

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn mismatch_multibits_bool(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        Byv: REAL;
        Wo: WORD;
    END_VAR

    Byv:= Wo.%X0; // bit 0 of Wo (invalid because we expect a REAL)
    Byv:= Wo.%1; // bit 1 of Wo (same but with omitted %X)

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:8:11 ]
       |
     4 |         Byv: REAL;
       |         ^|^
       |          `--- type is declared by variable 'Byv' here
       |
     8 |     Byv:= Wo.%X0; // bit 0 of Wo (invalid because we expect a REAL)
       |           ^^^|^^
       |              `---- expected 'REAL', got 'BOOL'
    ---'
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:9:11 ]
       |
     4 |         Byv: REAL;
       |         ^|^
       |          `--- type is declared by variable 'Byv' here
       |
     9 |     Byv:= Wo.%1; // bit 1 of Wo (same but with omitted %X)
       |           ^^|^^
       |             `---- expected 'REAL', got 'BOOL'
    ---'
    ");
}

#[rstest]
fn mismatch_multibits_byte(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        Byv: REAL;
        Wo: WORD;
    END_VAR

    Byv:= Wo.%B0; // byte 0 of Wo (invalid because we expect a REAL)

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:8:11 ]
       |
     4 |         Byv: REAL;
       |         ^|^
       |          `--- type is declared by variable 'Byv' here
       |
     8 |     Byv:= Wo.%B0; // byte 0 of Wo (invalid because we expect a REAL)
       |           ^^^|^^
       |              `---- expected 'REAL', got 'BYTE'
    ---'
    ");
}

#[rstest]
fn mismatch_multibits_word(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        Byv: REAL;
        Dwv: DWORD;
    END_VAR

    Byv:= Dwv.%W0; // word 0 of Dwv (invalid because we expect a REAL)

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:8:11 ]
       |
     4 |         Byv: REAL;
       |         ^|^
       |          `--- type is declared by variable 'Byv' here
       |
     8 |     Byv:= Dwv.%W0; // word 0 of Dwv (invalid because we expect a REAL)
       |           ^^^|^^^
       |              `----- expected 'REAL', got 'WORD'
    ---'
    ");
}

#[rstest]
fn mismatch_multibits_d_word(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        Byv: REAL;
        Lo: LWORD;
    END_VAR

    Byv:= Lo.%D0; // dword 0 of Lo (invalid because we expect a REAL)

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:8:11 ]
       |
     4 |         Byv: REAL;
       |         ^|^
       |          `--- type is declared by variable 'Byv' here
       |
     8 |     Byv:= Lo.%D0; // dword 0 of Lo (invalid because we expect a REAL)
       |           ^^^|^^
       |              `---- expected 'REAL', got 'DWORD'
       |              |
       |              `---- consider explicitly casting with 'DWORD_TO_REAL(Lo.%D0)'
       |
       | Help: insert explicit cast 'DWORD_TO_REAL(Lo.%D0)'
    ---'
    ");
}

#[rstest]
fn mismatch_multibits_l_word(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        Byv: REAL;
        Lo: LWORD;
    END_VAR

    Byv:= Lo.%L0; // lword 0 of Lo (invalid because we expect a REAL)

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:8:11 ]
       |
     4 |         Byv: REAL;
       |         ^|^
       |          `--- type is declared by variable 'Byv' here
       |
     8 |     Byv:= Lo.%L0; // lword 0 of Lo (invalid because we expect a REAL)
       |           ^^^|^^
       |              `---- expected 'REAL', got 'LWORD'
    ---'
    ");
}

#[rstest]
fn multibits_offset_out_of_range_byte(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        cnt: BYTE;
        Q0: BOOL;
        Q7: BOOL;
        Q8: BOOL;
    END_VAR

    Q0 := cnt.0;  // valid: bit 0 of BYTE
    Q7 := cnt.7;  // valid: bit 7 of BYTE
    Q8 := cnt.8;  // invalid: bit 8 exceeds BYTE (0..7)

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0808] Error: multibit access out of range
        ,-[ file:///test0.st:12:11 ]
        |
      4 |         cnt: BYTE;
        |         ^^^^|^^^^
        |             `------ 'cnt' is declared here
        |
     12 |     Q8 := cnt.8;  // invalid: bit 8 exceeds BYTE (0..7)
        |           ^|^
        |            `--- offset 8 is out of range for type 'BYTE' (valid range: 0..7)
    ----'
    ");
}

#[rstest]
fn multibits_offset_out_of_range_word(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        w: WORD;
        Q: BOOL;
    END_VAR

    Q := w.15;  // valid: bit 15 of WORD
    Q := w.16;  // invalid: bit 16 exceeds WORD (0..15)

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0808] Error: multibit access out of range
       ,-[ file:///test0.st:9:10 ]
       |
     4 |         w: WORD;
       |         ^^^|^^^
       |            `----- 'w' is declared here
       |
     9 |     Q := w.16;  // invalid: bit 16 exceeds WORD (0..15)
       |          |
       |          `-- offset 16 is out of range for type 'WORD' (valid range: 0..15)
    ---'
    ");
}

#[rstest]
fn multibits_access_offset_out_of_range(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        w: WORD;
        b: BYTE;
    END_VAR

    b := w.%B1;  // valid: byte 1 of WORD (bytes 0-1)
    b := w.%B2;  // invalid: byte 2 exceeds WORD (0..1)

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0808] Error: multibit access out of range
       ,-[ file:///test0.st:9:10 ]
       |
     4 |         w: WORD;
       |         ^^^|^^^
       |            `----- 'w' is declared here
       |
     9 |     b := w.%B2;  // invalid: byte 2 exceeds WORD (0..1)
       |          |
       |          `-- offset 2 is out of range for type 'WORD' (valid range: 0..1)
    ---'
    ");
}

#[rstest]
fn valid_multibits_on_indexed_array(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        SX: ARRAY[1..7] OF BYTE;
        SN: INT;
        Q0: BOOL;
    END_VAR

    Q0 := SX[SN].0;

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn valid_comparison_byte_with_integer_literal(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        SX: ARRAY[1..7] OF BYTE;
        SN: INT;
    END_VAR

    IF SX[SN] = 0 THEN
    END_IF;

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// An address written bare declares nothing, but it is still storage:
/// lowering gives each distinct one a cell in its area's band, so reading it
/// is as ordinary as reading a variable. A VAR_GLOBAL declared `AT` the same
/// address already has a cell, and the bare mention is that one — see
/// `a_bare_address_and_its_declaration_are_one_cell`.
#[rstest]
fn valid_bare_address_in_a_body(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        test: BOOL;
    END_VAR

    test := %IX0.0;

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// Writing an output is what an output is for.
#[rstest]
fn valid_write_to_a_bare_output(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    %QX0.1 := TRUE;
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// Writing a bare input is refused for the same reason a named one is: the
/// copy-in before the next scan overwrites it (E1419).
#[rstest]
fn invalid_write_to_a_bare_input(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    %IX0.1 := TRUE;
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1419] Error: write to an input location
       ,-[ file:///test0.st:3:5 ]
       |
     3 |     %IX0.1 := TRUE;
       |     ^^^|^^
       |        `---- '%IX0.1' is an input: it is written by the host, not by the program
       |
       | Note: the host copies the input image in before each scan, so this write is overwritten before anything can read it
    ---'
    ");
}

/// The size character may be left out, and then the address is a bit
/// (Table 16 row 4b): `%I1` is `%IX1`, and `%Q0.3` is `%QX0.3` — bit 3 of a
/// `%QW0` beside it.
#[rstest]
fn valid_address_without_a_width_letter_is_a_bit(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        test: BOOL;
        w: WORD;
    END_VAR

    test := %I1;
    %Q0.3 := test;
    w := %QW0;

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// An address that names no band keeps E1417: here the width letter is none
/// of `X`, `B`, `W`, `D` or `L`.
#[rstest]
fn invalid_bare_address_with_an_unknown_width_letter(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        test: WORD;
    END_VAR

    test := %IZ0;

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1417] Error: address cannot be located
       ,-[ file:///test0.st:7:13 ]
       |
     7 |     test := %IZ0;
       |             ^^|^
       |               `--- '%IZ0' does not name an area and a width
       |
       | Note: an address names its area with I, Q or M and its width with X, B, W, D or L, as in '%IX0.0'; a bit may leave the width out, as in '%I0.0'
    ---'
    ");
}

/// Three levels are three levels (Table 16 row 10): the width comes from the
/// size letter, never from the last level. Every level after the first used
/// to be taken as a partial access, so `%ID0.1.2` did not parse at all and
/// `%IB0.0` read back as the BOOL a bare `.0` selects.
#[rstest]
fn valid_hierarchical_address_keeps_its_width(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        b: BYTE;
        d: DWORD;
    END_VAR

    b := %IB0.0;
    d := %ID0.1.2;

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

// ---------------------------------------------------------------------------
// Located (`AT %…`) declarations. A VAR_GLOBAL bound to an address is storage
// in one of the three I/O bands; everywhere else a location still has nowhere
// to live and keeps E1417.
// ---------------------------------------------------------------------------

/// A VAR_GLOBAL in each of the three areas is accepted: the address names a
/// band, and the declaration names the variable a host binds a channel to.
#[rstest]
fn valid_located_globals_in_each_area(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM P
VAR_EXTERNAL sensor : BOOL; valve : BOOL; flag : INT; END_VAR
    valve := sensor;
    flag := flag + 1;
END_PROGRAM

CONFIGURATION Cfg
VAR_GLOBAL
    sensor AT %IX0.0 : BOOL;
    valve AT %QX0.1 : BOOL;
    flag AT %MW2 : INT;
END_VAR
    RESOURCE R ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH T : P;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// The host owns `%I`: it copies the process image in before every scan, so a
/// write the program makes is gone before anything can read it. Accepting it
/// silently produced a program whose assignments simply vanished.
#[rstest]
fn invalid_write_to_an_input_location(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM P
VAR_EXTERNAL sensor : BOOL; END_VAR
    sensor := TRUE;
END_PROGRAM

CONFIGURATION Cfg
VAR_GLOBAL sensor AT %IX0.0 : BOOL; END_VAR
    RESOURCE R ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH T : P;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1419] Error: write to an input location
       ,-[ file:///test0.st:4:5 ]
       |
     4 |     sensor := TRUE;
       |     ^^^|^^
       |        `---- '%IX0.0' is an input: it is written by the host, not by the program
       |
       | Note: the host copies the input image in before each scan, so this write is overwritten before anything can read it
    ---'
    ");
}

/// A VAR_IN_OUT binding hands the callee a writable alias, so an input passed
/// to one is written just as surely as one on the left of a `:=`.
#[rstest]
fn invalid_input_bound_to_an_in_out(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Bump
VAR_IN_OUT v : INT; END_VAR
    v := v + 1;
END_FUNCTION_BLOCK

PROGRAM P
VAR_EXTERNAL sensor : INT; END_VAR
VAR b : Bump; END_VAR
    b(v := sensor);
END_PROGRAM

CONFIGURATION Cfg
VAR_GLOBAL sensor AT %IW0 : INT; END_VAR
    RESOURCE R ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH T : P;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1419] Error: write to an input location
        ,-[ file:///test0.st:10:12 ]
        |
     10 |     b(v := sensor);
        |            ^^^|^^
        |               `---- '%IW0' is an input: it is written by the host, not by the program
        |
        | Note: the host copies the input image in before each scan, so this write is overwritten before anything can read it
    ----'
    ");
}

/// Reading an input is the whole point of one, so it stays clean.
#[rstest]
fn valid_read_of_an_input_location(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM P
VAR_EXTERNAL sensor : BOOL; END_VAR
VAR seen : BOOL; END_VAR
    seen := sensor;
END_PROGRAM

CONFIGURATION Cfg
VAR_GLOBAL sensor AT %IX0.0 : BOOL; END_VAR
    RESOURCE R ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH T : P;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// The retain band is restored at startup, so a retained input image would
/// run the first scan on the values of the last power cycle — before the
/// field bus has refreshed them.
#[rstest]
fn invalid_retain_on_an_input_location(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM P
VAR_EXTERNAL sensor : BOOL; END_VAR
VAR seen : BOOL; END_VAR
    seen := sensor;
END_PROGRAM

CONFIGURATION Cfg
VAR_GLOBAL RETAIN sensor AT %IX0.0 : BOOL; END_VAR
    RESOURCE R ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH T : P;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1420] Error: RETAIN on an I/O location
       ,-[ file:///test0.st:9:19 ]
       |
     9 | VAR_GLOBAL RETAIN sensor AT %IX0.0 : BOOL; END_VAR
       |                   ^^^^^^^^^^^|^^^^^^^^^^^
       |                              `------------- 'sensor' is located at '%IX0.0' and cannot be RETAIN
       |
       | Note: the retain band is restored at startup, so a retained I/O image would run the first scan on the values of the last power cycle; only '%M' may persist
    ---'
    ");
}

/// `%M` is the area that may legitimately persist: it is the program's own,
/// and no copy-in overwrites it.
#[rstest]
fn valid_retain_on_a_marker_location(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM P
VAR_EXTERNAL count : INT; END_VAR
    count := count + 1;
END_PROGRAM

CONFIGURATION Cfg
VAR_GLOBAL RETAIN count AT %MW0 : INT; END_VAR
    RESOURCE R ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH T : P;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// A function's, a function block's or a class's variables belong to each
/// call or instance, so one address cannot be theirs (E1417). A PROGRAM's
/// can: see `valid_located_variables_in_a_program`.
#[rstest]
fn invalid_located_variable_outside_a_program(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Valve
VAR
    POS AT %QW28 : INT;
END_VAR
END_FUNCTION_BLOCK

FUNCTION Probe : BOOL
VAR
    raw AT %IX0.0 : BOOL;
END_VAR
    Probe := raw;
END_FUNCTION"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1417] Error: address cannot be located
       ,-[ file:///test0.st:4:5 ]
       |
     4 |     POS AT %QW28 : INT;
       |     ^^^^^^^^^|^^^^^^^^
       |              `---------- '%QW28' cannot locate a variable of this POU
       |
       | Note: a function's, function block's or class's variables belong to each call or instance, so one address cannot be theirs; declare it in a PROGRAM, or as a VAR_GLOBAL of the CONFIGURATION, and name it from here
    ---'
    [E1417] Error: address cannot be located
        ,-[ file:///test0.st:10:5 ]
        |
     10 |     raw AT %IX0.0 : BOOL;
        |     ^^^^^^^^^^|^^^^^^^^^
        |               `----------- '%IX0.0' cannot locate a variable of this POU
        |
        | Note: a function's, function block's or class's variables belong to each call or instance, so one address cannot be theirs; declare it in a PROGRAM, or as a VAR_GLOBAL of the CONFIGURATION, and name it from here
    ----'
    ");
}

/// A PROGRAM's VAR may be located (Table 16, `Loc_Var_Decls`), named or
/// not, RETAIN in `%M`, and as a part of a wider address: it is the channel,
/// shared by every instance of the program.
#[rstest]
fn valid_located_variables_in_a_program(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM P
VAR
    start AT %IX0.0 : BOOL;
    level AT %IW1   : INT;
    lamp  AT %QX0.0 : BOOL;
    AT %QB4 : BYTE;
END_VAR
VAR RETAIN
    count AT %MW0 : INT;
END_VAR
    lamp := start AND level > 0;
    count := count + 1;
    %QB4 := 16#0F;
END_PROGRAM

CONFIGURATION Cfg
VAR_GLOBAL image AT %ID0 : DWORD; END_VAR
    RESOURCE R ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH T : P;
        PROGRAM P2 WITH T : P;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// A PROGRAM's located VAR is held to the rules a located VAR_GLOBAL is: no
/// RETAIN on I/O (E1420), no initial value on an input (E1419), a type as
/// wide as its address (E1422), and one declaration per address across the
/// whole workspace, a VAR_GLOBAL included (E1421).
#[rstest]
fn invalid_located_variables_in_a_program(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM P
VAR RETAIN
    kept AT %IW0 : WORD;
END_VAR
VAR
    preset AT %IB8 : BYTE := 3;
    wide   AT %QX1.0 : INT;
    twin   AT %QW4 : WORD;
END_VAR
END_PROGRAM

CONFIGURATION Cfg
VAR_GLOBAL valve AT %QW4 : WORD; END_VAR
    RESOURCE R ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH T : P;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1420] Error: RETAIN on an I/O location
       ,-[ file:///test0.st:4:5 ]
       |
     4 |     kept AT %IW0 : WORD;
       |     ^^^^^^^^^|^^^^^^^^^
       |              `----------- 'kept' is located at '%IW0' and cannot be RETAIN
       |
       | Note: the retain band is restored at startup, so a retained I/O image would run the first scan on the values of the last power cycle; only '%M' may persist
    ---'
    [E1419] Error: write to an input location
       ,-[ file:///test0.st:7:5 ]
       |
     7 |     preset AT %IB8 : BYTE := 3;
       |     ^^^^^^^^^^^^^|^^^^^^^^^^^^
       |                  `-------------- '%IB8' is an input, so an initial value is overwritten before anything reads it
       |
       | Note: the host writes the input image before every scan, the first one included
    ---'
    [E1422] Error: location type mismatch
       ,-[ file:///test0.st:8:5 ]
       |
     8 |     wide   AT %QX1.0 : INT;
       |     ^^^^^^^^^^^|^^^^^^^^^^
       |                `------------ '%QX1.0' is 1 bit, but 'wide' is declared 'INT', which is 16
       |
       | Note: a located variable holds one value as wide as its address; declare it as an elementary type of 1 bit, such as BOOL
    ---'
    [E1421] Error: duplicate location
        ,-[ file:///test0.st:9:5 ]
        |
      9 |     twin   AT %QW4 : WORD;
        |     ^^^^^^^^^^|^^^^^^^^^^
        |               `------------ 'twin' is located at '%QW4', which 'valve' also claims
        |
     14 | VAR_GLOBAL valve AT %QW4 : WORD; END_VAR
        |            ^^^^^^^^^^|^^^^^^^^^
        |                      `----------- 'valve' is located here
        |
        | Note: an address is one channel, and each declaration is given storage of its own, so the two would never see each other's value; name the one variable from wherever it is needed
    ----'
    [E1421] Error: duplicate location
        ,-[ file:///test0.st:14:12 ]
        |
      9 |     twin   AT %QW4 : WORD;
        |     ^^^^^^^^^^|^^^^^^^^^^
        |               `------------ 'twin' is located here
        |
     14 | VAR_GLOBAL valve AT %QW4 : WORD; END_VAR
        |            ^^^^^^^^^^|^^^^^^^^^
        |                      `----------- 'valve' is located at '%QW4', which 'twin' also claims
        |
        | Note: an address is one channel, and each declaration is given storage of its own, so the two would never see each other's value; name the one variable from wherever it is needed
    ----'
    ");
}

/// A VAR_CONFIG entry locates a variable declared `AT %I*`, `%Q*` or `%M*`
/// with a complete address in that area, as wide as its type, that a pointer
/// can reach (E1424). A refused entry locates nothing, so the variable is
/// also reported as never located (E1425).
#[rstest]
fn invalid_config_locations(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Drive
VAR
    run   AT %Q* : BOOL;
    level AT %I* : INT;
END_VAR
VAR plain : INT; END_VAR
END_FUNCTION_BLOCK

PROGRAM P
VAR d : Drive; END_VAR
END_PROGRAM

CONFIGURATION Cfg
VAR_GLOBAL panel AT %QW0 : WORD; END_VAR
VAR_CONFIG
    Res.P1.d.plain AT %MW4   : INT;
    Res.P1.d.level AT %QW2   : INT;
    Res.P1.d.run   AT %QX0.3 : BOOL;
    Res.P2.d.run   AT %QW4   : BOOL;
    Res.P2.d.level AT %I*    : INT;
END_VAR
    RESOURCE Res ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH T : P;
        PROGRAM P2 WITH T : P;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1424] Error: location refused
        ,-[ file:///test0.st:17:14 ]
        |
     17 |     Res.P1.d.plain AT %MW4   : INT;
        |              ^^|^^
        |                `---- 'plain' is not declared AT %I*, %Q* or %M*, so its address is not VAR_CONFIG's to give
        |
        | Note: declare it AT %I*, %Q* or %M* in its POU to leave its address to the configuration
    ----'
    [E1424] Error: location refused
        ,-[ file:///test0.st:18:14 ]
        |
     18 |     Res.P1.d.level AT %QW2   : INT;
        |              ^^|^^
        |                `---- 'level' is declared AT %I*, and '%QW2' is not in that area
        |
        | Note: the area is the declaration's: give an input an address in %I, an output one in %Q, a marker one in %M
    ----'
    [E1424] Error: location refused
        ,-[ file:///test0.st:19:14 ]
        |
     19 |     Res.P1.d.run   AT %QX0.3 : BOOL;
        |              ^|^
        |               `--- '%QX0.3' is a bit of '%QW0', and a bit has no address to locate 'run' at
        |
        | Note: the variable points at its channel, so give it a byte or wider, or a bit nothing wider around it is named
    ----'
    [E1424] Error: location refused
        ,-[ file:///test0.st:20:14 ]
        |
     20 |     Res.P2.d.run   AT %QW4   : BOOL;
        |              ^|^
        |               `--- '%QW4' is 16 bits, but 'run' is declared 'BOOL', which is 1
        |
        | Note: the variable holds one value as wide as its address; give it an address of its type's width
    ----'
    [E1424] Error: location refused
        ,-[ file:///test0.st:21:14 ]
        |
     21 |     Res.P2.d.level AT %I*    : INT;
        |              ^^|^^
        |                `---- '%I*' is not a complete address
        |
        | Note: VAR_CONFIG gives the complete address, such as '%IX0.0' or '%QW4'
    ----'
    ");
}

/// A VAR_CONFIG path reaches an inherited member as it does a member of the
/// instance's own type, and an entry may sit in another block of the same
/// configuration. A VAR_IN_OUT points at an instance located where it is
/// declared, so nothing locates what it holds: not in `h`, not in a
/// VAR_GLOBAL.
#[rstest]
fn valid_config_locations_of_inherited_members_across_fragments(mut with_db: RootDatabase) {
    let machine = r#"
FUNCTION_BLOCK Base
VAR run AT %Q* : BOOL; END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK Drive EXTENDS Base
END_FUNCTION_BLOCK

FUNCTION_BLOCK Holder
VAR_IN_OUT io : Drive; END_VAR
END_FUNCTION_BLOCK

PROGRAM P
VAR d : Drive; h : Holder; END_VAR
END_PROGRAM

CONFIGURATION Cfg
VAR_GLOBAL spare : Holder; END_VAR
    RESOURCE Res ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH T : P;
    END_RESOURCE
END_CONFIGURATION
"#;
    let wiring = r#"
CONFIGURATION Cfg
VAR_CONFIG
    Res.P1.d.run AT %QX0.0 : BOOL;
END_VAR
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[machine, wiring]), @r"");
}

/// An entry is wrong about its variable when it names what a path cannot
/// (an element, a dereference) or writes another type than the variable's
/// (E1426), and two entries cannot both locate one variable (E1424, at each).
/// An entry that does not resolve or is refused gives no address: `flag`,
/// which would be a bit of `%QW1`, stays a cell of its own.
#[rstest]
fn invalid_config_entries(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Drive
VAR
    run   AT %Q* : BOOL;
    level AT %I* : INT;
END_VAR
END_FUNCTION_BLOCK

PROGRAM P
VAR d : Drive; r : REF_TO Drive; END_VAR
END_PROGRAM

CONFIGURATION Cfg
VAR_GLOBAL flag AT %QX2.3 : BOOL := TRUE; END_VAR
VAR_CONFIG
    Res.P1.d.run   AT %QX0.0 : BOOL;
    Res.P1.d.run   AT %QX0.1 : BOOL;
    Res.P1.d.level AT %IW2   : UINT;
    Res.P1.r^.run  AT %QX1.0 : BOOL;
    Res.P1.d.lvl   AT %QW1   : INT;
    Res.P1.d.run   : BOOL := TRUE;
END_VAR
    RESOURCE Res ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH T : P;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1426] Error: configuration entry refused
        ,-[ file:///test0.st:18:14 ]
        |
     18 |     Res.P1.d.level AT %IW2   : UINT;
        |              ^^|^^
        |                `---- the entry says 'UINT', but 'level' is declared 'INT'
        |
        | Note: the entry repeats the variable's type; write the declared one
    ----'
    [E1426] Error: configuration entry refused
        ,-[ file:///test0.st:19:15 ]
        |
     19 |     Res.P1.r^.run  AT %QX1.0 : BOOL;
        |               ^|^
        |                `--- a VAR_CONFIG path names instances and variables, not an element or what a reference points at
        |
        | Note: name the variable itself; an element of an array or a referenced value cannot be configured
    ----'
    [E1414] Error: configuration error
        ,-[ file:///test0.st:20:14 ]
        |
     20 |     Res.P1.d.lvl   AT %QW1   : INT;
        |              ^|^
        |               `--- 'Drive' has no field named 'lvl'
    ----'
    [E1424] Error: location refused
        ,-[ file:///test0.st:16:14 ]
        |
     16 |     Res.P1.d.run   AT %QX0.0 : BOOL;
        |              ^|^
        |               `--- 'run' is located at '%QX0.0' here and at '%QX0.1' by another entry
        |
        | Note: an instance's variable has one address; keep one of the entries
    ----'
    [E1424] Error: location refused
        ,-[ file:///test0.st:17:14 ]
        |
     17 |     Res.P1.d.run   AT %QX0.1 : BOOL;
        |              ^|^
        |               `--- 'run' is located at '%QX0.1' here and at '%QX0.0' by another entry
        |
        | Note: an instance's variable has one address; keep one of the entries
    ----'
    [E1416] Error: unsupported configuration element
        ,-[ file:///test0.st:21:14 ]
        |
     21 |     Res.P1.d.run   : BOOL := TRUE;
        |              ^|^
        |               `--- a VAR_CONFIG value is checked but not applied yet, so it never reaches the instance
        |
        | Note: a variable VAR_CONFIG locates starts at its type's default, or at the value its channel's own declaration gives it
    ----'
    ");
}

/// An instance whose type holds a variable declared `AT %I*` has to be one a
/// VAR_CONFIG path names: a PROGRAM instance, or an instance it holds by
/// name. An array's element, a VAR_GLOBAL and an instance made for each call
/// are not (E1425). RETAIN is refused on the variable itself (E1420): it
/// points at its channel and has no storage of its own.
#[rstest]
fn invalid_partly_located_variables_out_of_reach(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Drive
VAR run AT %Q* : BOOL; END_VAR
VAR RETAIN kept AT %M* : INT; END_VAR
END_FUNCTION_BLOCK

PROGRAM P
VAR drives : ARRAY[0..1] OF Drive; END_VAR
VAR_TEMP scratch : Drive; END_VAR
END_PROGRAM

CONFIGURATION Cfg
VAR_GLOBAL spare : Drive; END_VAR
    RESOURCE Res ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH T : P;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1420] Error: RETAIN on an I/O location
       ,-[ file:///test0.st:4:12 ]
       |
     4 | VAR RETAIN kept AT %M* : INT; END_VAR
       |            ^^^^^^^^|^^^^^^^^
       |                    `---------- 'kept' is located at '%M*' and cannot be RETAIN
       |
       | Note: a variable VAR_CONFIG locates points at its channel and has no storage of its own to retain; to persist a marker, declare it located in full, RETAIN, in a PROGRAM or as a VAR_GLOBAL
    ---'
    [E1425] Error: variable not located
       ,-[ file:///test0.st:8:5 ]
       |
     8 | VAR drives : ARRAY[0..1] OF Drive; END_VAR
       |     ^^^^^^^^^^^^^^|^^^^^^^^^^^^^^
       |                   `---------------- 'drives' holds 'run', declared AT %Q*, in the elements of an array, which VAR_CONFIG cannot name
       |
       | Note: a VAR_CONFIG path names a PROGRAM instance and the instances it holds by name; hold this one there
    ---'
    [E1425] Error: variable not located
       ,-[ file:///test0.st:9:10 ]
       |
     9 | VAR_TEMP scratch : Drive; END_VAR
       |          ^^^^^^^|^^^^^^^
       |                 `--------- 'scratch' holds 'run', declared AT %Q*, in an instance made for each call, which VAR_CONFIG cannot name
       |
       | Note: a VAR_CONFIG path names a PROGRAM instance and the instances it holds by name; hold this one there
    ---'
    [E1425] Error: variable not located
        ,-[ file:///test0.st:13:12 ]
        |
     13 | VAR_GLOBAL spare : Drive; END_VAR
        |            ^^^^^^|^^^^^^
        |                  `-------- 'spare' holds 'run', declared AT %Q*, in a VAR_GLOBAL, which VAR_CONFIG cannot name
        |
        | Note: a VAR_CONFIG path names a PROGRAM instance and the instances it holds by name; hold this one there
    ----'
    ");
}

/// `%I*` names no address at all — the binding comes from VAR_CONFIG, which
/// is not applied yet — so there is nothing to allocate.
#[rstest]
fn unsupported_incomplete_location(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM P
VAR_EXTERNAL sensor : BOOL; END_VAR
VAR seen : BOOL; END_VAR
    seen := sensor;
END_PROGRAM

CONFIGURATION Cfg
VAR_GLOBAL sensor AT %I* : BOOL; END_VAR
    RESOURCE R ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH T : P;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1417] Error: address cannot be located
       ,-[ file:///test0.st:9:12 ]
       |
     9 | VAR_GLOBAL sensor AT %I* : BOOL; END_VAR
       |            ^^^^^^^^^^|^^^^^^^^^
       |                      `----------- '%I*' is not a complete address
       |
       | Note: VAR_CONFIG completes a partial address for a variable of a PROGRAM, FUNCTION_BLOCK or CLASS, instance by instance; anywhere else, write the address in full
    ---'
    ");
}

/// Assignment is not the only way to write. Every by-reference route into an
/// input is refused the same way: an output binding, a FOR control variable
/// and a partial write all end in a store the copy-in would overwrite.
#[rstest]
fn invalid_write_routes_into_an_input(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Src
VAR_OUTPUT o : INT; END_VAR
    o := 1;
END_FUNCTION_BLOCK

PROGRAM P
VAR_EXTERNAL sensor : INT; bits : WORD; END_VAR
VAR s : Src; i : INT; END_VAR
    s(o => sensor);
    FOR sensor := 1 TO 10 DO i := i + 1; END_FOR;
    bits.3 := TRUE;
END_PROGRAM

CONFIGURATION Cfg
VAR_GLOBAL sensor AT %IW0 : INT; bits AT %IW2 : WORD; END_VAR
    RESOURCE R ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH T : P;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1419] Error: write to an input location
        ,-[ file:///test0.st:10:12 ]
        |
     10 |     s(o => sensor);
        |            ^^^|^^
        |               `---- '%IW0' is an input: it is written by the host, not by the program
        |
        | Note: the host copies the input image in before each scan, so this write is overwritten before anything can read it
    ----'
    [E1419] Error: write to an input location
        ,-[ file:///test0.st:11:9 ]
        |
     11 |     FOR sensor := 1 TO 10 DO i := i + 1; END_FOR;
        |         ^^^|^^
        |            `---- '%IW0' is an input: it is written by the host, not by the program
        |
        | Note: the host copies the input image in before each scan, so this write is overwritten before anything can read it
    ----'
    [E1419] Error: write to an input location
        ,-[ file:///test0.st:12:5 ]
        |
     12 |     bits.3 := TRUE;
        |     ^^^|^^
        |        `---- '%IW2' is an input: it is written by the host, not by the program
        |
        | Note: the host copies the input image in before each scan, so this write is overwritten before anything can read it
    ----'
    ");
}

/// `REF()` is the route that does not store yet — it hands out the capability
/// to. Nothing tracks what a pointer is stored through, so the reference is
/// refused where it is taken rather than where it is used.
#[rstest]
fn invalid_reference_to_an_input(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM P
VAR_EXTERNAL sensor : INT; END_VAR
VAR p : REF_TO INT; END_VAR
    p := REF(sensor);
    p^ := 99;
END_PROGRAM

CONFIGURATION Cfg
VAR_GLOBAL sensor AT %IW0 : INT; END_VAR
    RESOURCE R ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH T : P;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1419] Error: write to an input location
       ,-[ file:///test0.st:5:14 ]
       |
     5 |     p := REF(sensor);
       |              ^^^|^^
       |                 `---- '%IW0' is an input, so a writable reference to it cannot be taken
       |
       | Note: a REF_TO is a writable pointer and nothing tracks what is stored through it, so the reference is refused where it is taken
    ---'
    ");
}

/// An output may be written through every one of those routes: the rule is
/// about `%I`, not about located variables in general.
#[rstest]
fn valid_write_routes_into_an_output(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Src
VAR_OUTPUT o : INT; END_VAR
    o := 1;
END_FUNCTION_BLOCK

PROGRAM P
VAR_EXTERNAL valve : INT; bits : WORD; END_VAR
VAR s : Src; p : REF_TO INT; END_VAR
    s(o => valve);
    bits.3 := TRUE;
    p := REF(valve);
    p^ := 99;
END_PROGRAM

CONFIGURATION Cfg
VAR_GLOBAL valve AT %QW0 : INT; bits AT %QW2 : WORD; END_VAR
    RESOURCE R ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH T : P;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// An address is one channel, so two declarations cannot claim it. Each is
/// given storage of its own, which would leave the program with two variables
/// the plant cannot tell apart — and a host binding by address would find it
/// twice.
#[rstest]
fn invalid_two_declarations_at_one_address(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM P
VAR_EXTERNAL a : INT; b : INT; END_VAR
VAR t : INT; END_VAR
    t := a + b;
END_PROGRAM

CONFIGURATION Cfg
VAR_GLOBAL
    a AT %IW0 : INT;
    b AT %IW0 : INT;
END_VAR
    RESOURCE R ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH T : P;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1421] Error: duplicate location
        ,-[ file:///test0.st:10:5 ]
        |
     10 |     a AT %IW0 : INT;
        |     ^^^^^^^|^^^^^^^
        |            `--------- 'a' is located at '%IW0', which 'b' also claims
     11 |     b AT %IW0 : INT;
        |     ^^^^^^^|^^^^^^^
        |            `--------- 'b' is located here
        |
        | Note: an address is one channel, and each declaration is given storage of its own, so the two would never see each other's value; name the one variable from wherever it is needed
    ----'
    [E1421] Error: duplicate location
        ,-[ file:///test0.st:11:5 ]
        |
     10 |     a AT %IW0 : INT;
        |     ^^^^^^^|^^^^^^^
        |            `--------- 'a' is located here
     11 |     b AT %IW0 : INT;
        |     ^^^^^^^|^^^^^^^
        |            `--------- 'b' is located at '%IW0', which 'a' also claims
        |
        | Note: an address is one channel, and each declaration is given storage of its own, so the two would never see each other's value; name the one variable from wherever it is needed
    ----'
    ");
}

/// The size character says how wide the channel is; the declared type says
/// how wide the value read there is. They have to agree.
#[rstest]
fn invalid_width_against_the_declared_type(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM P
VAR_EXTERNAL wide : INT; END_VAR
VAR t : INT; END_VAR
    t := wide;
END_PROGRAM

CONFIGURATION Cfg
VAR_GLOBAL wide AT %IX0.0 : INT; END_VAR
    RESOURCE R ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH T : P;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1422] Error: location type mismatch
       ,-[ file:///test0.st:9:12 ]
       |
     9 | VAR_GLOBAL wide AT %IX0.0 : INT; END_VAR
       |            ^^^^^^^^^^|^^^^^^^^^
       |                      `----------- '%IX0.0' is 1 bit, but 'wide' is declared 'INT', which is 16
       |
       | Note: a located variable holds one value as wide as its address; declare it as an elementary type of 1 bit, such as BOOL
    ---'
    ");
}

/// Every size character has more than one type of its width, and each is
/// accepted: the rule is about bits, not about the name. CHAR is 8 bits, and
/// an alias is the type it names.
#[rstest]
fn valid_widths_that_agree_with_the_address(mut with_db: RootDatabase) {
    let source = r#"
TYPE Speed : INT; END_TYPE

PROGRAM P
VAR_EXTERNAL bit : BOOL; sb : SINT; octet : BYTE; n : INT; r : REAL; el : TIME; big : LREAL; ch : CHAR; speed : Speed; END_VAR
VAR t : BOOL; END_VAR
    t := bit;
END_PROGRAM

CONFIGURATION Cfg
VAR_GLOBAL
    bit AT %IX0.0 : BOOL;
    sb  AT %IB0   : SINT;
    octet AT %IB1 : BYTE;
    n   AT %IW1   : INT;
    r   AT %ID2   : REAL;
    el  AT %ID3   : TIME;
    big AT %IL4   : LREAL;
    ch  AT %IB20  : CHAR;
    speed AT %IW20 : Speed;
END_VAR
    RESOURCE R ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH T : P;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// A DT is 64 bits, seconds since the epoch, so it takes an `L` address.
#[rstest]
fn invalid_date_and_time_at_a_double_word(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM P
VAR_EXTERNAL stamp : DT; ok : DT; END_VAR
    ok := stamp;
END_PROGRAM

CONFIGURATION Cfg
VAR_GLOBAL
    stamp AT %ID0 : DT;
    ok    AT %QL0 : DT;
END_VAR
    RESOURCE R ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH T : P;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1422] Error: location type mismatch
       ,-[ file:///test0.st:9:5 ]
       |
     9 |     stamp AT %ID0 : DT;
       |     ^^^^^^^^^|^^^^^^^^
       |              `---------- '%ID0' is 32 bits, but 'stamp' is declared 'DT', which is 64
       |
       | Note: a located variable holds one value as wide as its address; declare it as an elementary type of 32 bits, such as DWORD, DINT or REAL
    ---'
    ");
}

/// A part is declared with any type of its width, as a whole address is: a
/// signed byte in a word, a REAL or a TIME in a long word, a CHAR in a word.
#[rstest]
fn valid_parts_of_any_type_of_their_width(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM P
VAR_EXTERNAL hi : SINT; gain : REAL; since : TIME; ch : CHAR; END_VAR
VAR t : REAL; END_VAR
    t := gain;
END_PROGRAM

CONFIGURATION Cfg
VAR_GLOBAL
    status AT %IW0 : WORD;
    hi     AT %IB1 : SINT;
    frame  AT %IL1 : LWORD;
    gain   AT %ID2 : REAL;
    since  AT %ID3 : TIME;
    text   AT %IW8 : WORD;
    ch     AT %IB16 : CHAR;
END_VAR
    RESOURCE R ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH T : P;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// A located variable holds one value as wide as its address, so it is an
/// elementary type of that width — as matiec requires, and as every vendor
/// accepts. An enum, a subrange, an aggregate and a STRING have no single
/// width and are refused rather than guessed at.
#[rstest]
fn invalid_located_variable_without_an_elementary_type(mut with_db: RootDatabase) {
    let source = r#"
TYPE
    Colour : (Red, Green);
    Pct : INT (0..100);
    Pair : STRUCT a : BYTE; b : BYTE; END_STRUCT;
END_TYPE

PROGRAM P
VAR_EXTERNAL c : Colour; p : Pct; s : Pair; bits : ARRAY[0..7] OF BOOL; name : STRING; END_VAR
END_PROGRAM

CONFIGURATION Cfg
VAR_GLOBAL
    c    AT %IB0 : Colour;
    p    AT %IW1 : Pct;
    s    AT %IW2 : Pair;
    bits AT %IB6 : ARRAY[0..7] OF BOOL;
    name AT %ID2 : STRING;
END_VAR
    RESOURCE R ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH T : P;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1422] Error: location type mismatch
        ,-[ file:///test0.st:14:5 ]
        |
     14 |     c    AT %IB0 : Colour;
        |     ^^^^^^^^^^|^^^^^^^^^^
        |               `------------ '%IB0' is 8 bits, but 'c' is declared 'Colour', which is not an elementary type of any width
        |
        | Note: a located variable holds one value as wide as its address; declare it as an elementary type of 8 bits, such as BYTE, SINT or USINT
    ----'
    [E1422] Error: location type mismatch
        ,-[ file:///test0.st:15:5 ]
        |
     15 |     p    AT %IW1 : Pct;
        |     ^^^^^^^^^|^^^^^^^^
        |              `---------- '%IW1' is 16 bits, but 'p' is declared 'Pct', which is not an elementary type of any width
        |
        | Note: a located variable holds one value as wide as its address; declare it as an elementary type of 16 bits, such as WORD, INT or UINT
    ----'
    [E1422] Error: location type mismatch
        ,-[ file:///test0.st:16:5 ]
        |
     16 |     s    AT %IW2 : Pair;
        |     ^^^^^^^^^|^^^^^^^^^
        |              `----------- '%IW2' is 16 bits, but 's' is declared 'Pair', which is not an elementary type of any width
        |
        | Note: a located variable holds one value as wide as its address; declare it as an elementary type of 16 bits, such as WORD, INT or UINT
    ----'
    [E1422] Error: location type mismatch
        ,-[ file:///test0.st:17:5 ]
        |
     17 |     bits AT %IB6 : ARRAY[0..7] OF BOOL;
        |     ^^^^^^^^^^^^^^^^^|^^^^^^^^^^^^^^^^
        |                      `------------------ '%IB6' is 8 bits, but 'bits' is declared 'ARRAY [0..7] OF BOOL', which is not an elementary type of any width
        |
        | Note: a located variable holds one value as wide as its address; declare it as an elementary type of 8 bits, such as BYTE, SINT or USINT
    ----'
    [E1422] Error: location type mismatch
        ,-[ file:///test0.st:18:5 ]
        |
     18 |     name AT %ID2 : STRING;
        |     ^^^^^^^^^^|^^^^^^^^^^
        |               `------------ '%ID2' is 32 bits, but 'name' is declared 'STRING', which is not an elementary type of any width
        |
        | Note: a located variable holds one value as wide as its address; declare it as an elementary type of 32 bits, such as DWORD, DINT or REAL
    ----'
    ");
}

// ---------------------------------------------------------------------------
// Parts of a wider address (E1423). `%QX0.3` beside a `%QW0` is that word's
// bit 3: it is stored in the word's cell and has no address of its own, so
// the uses that need one are refused. Reading it, assigning it and binding
// an output to it are fine. A part of a byte or more is whole bytes of the
// cell, which do have an address.
// ---------------------------------------------------------------------------

/// Reading a part, assigning it, and binding an FB's or a function's output
/// to it are all ordinary: the output is put into the part once the call
/// returns.
#[rstest]
fn valid_uses_of_a_part_of_a_wider_address(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Pass
VAR_INPUT i : BOOL; END_VAR
VAR_OUTPUT q : BOOL; END_VAR
    q := i;
END_FUNCTION_BLOCK

FUNCTION Out : BOOL
VAR_OUTPUT o : BOOL; END_VAR
    o := TRUE;
    Out := TRUE;
END_FUNCTION

PROGRAM P
VAR_EXTERNAL ready : BOOL; END_VAR
VAR f : Pass; x : BOOL; END_VAR
    x := %IX0.3;
    %QX0.3 := ready;
    f(i := x, q => %QX0.4);
    x := Out(o => %QX0.5);
END_PROGRAM

CONFIGURATION Cfg
VAR_GLOBAL
    status AT %IW0 : WORD;
    ready  AT %IX1.7 : BOOL;
    lamps  AT %QW0 : WORD;
END_VAR
    RESOURCE R ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH T : P;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// A part of a byte or more is whole bytes of its owner's cell, so it can be
/// passed to a VAR_IN_OUT or referenced like any variable.
#[rstest]
fn valid_references_to_a_part_of_a_byte_or_more(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Bump
VAR_IN_OUT b : BYTE; END_VAR
    b := b + 1;
END_FUNCTION_BLOCK

FUNCTION Flip : BOOL
VAR_IN_OUT w : WORD; END_VAR
    w := w XOR 16#FFFF;
    Flip := TRUE;
END_FUNCTION

PROGRAM P
VAR_EXTERNAL level : SINT; END_VAR
VAR g : Bump; ok : BOOL; r : REF_TO SINT; END_VAR
    %QD0 := 0;
    g(b := %QB1);
    ok := Flip(w := %QW1);
    r := REF(level);
END_PROGRAM

CONFIGURATION Cfg
VAR_GLOBAL level AT %QB0 : SINT; END_VAR
    RESOURCE R ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH T : P;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// A VAR_IN_OUT takes an address, and three bits of a word have none.
#[rstest]
fn invalid_part_of_a_wider_address_passed_to_an_in_out(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Flip
VAR_IN_OUT v : BOOL; END_VAR
    v := NOT v;
END_FUNCTION_BLOCK

PROGRAM P
VAR f : Flip; w : WORD; END_VAR
    w := %QW0;
    f(v := %QX0.3);
END_PROGRAM
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1423] Error: part of a wider address
        ,-[ file:///test0.st:10:12 ]
        |
     10 |     f(v := %QX0.3);
        |            ^^^|^^
        |               `---- '%QX0.3' is part of '%QW0' and has no address of its own to pass to a VAR_IN_OUT
        |
        | Note: copy it into a variable, pass that, and assign it back
    ----'
    ");
}

/// `REF()` takes an address too, for a declared part as for a bare one.
#[rstest]
fn invalid_reference_to_a_part_of_a_wider_address(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM P
VAR_EXTERNAL named : BOOL; END_VAR
VAR p : REF_TO BOOL; w : WORD; END_VAR
    w := %QW0;
    p := REF(named);
END_PROGRAM

CONFIGURATION Cfg
VAR_GLOBAL named AT %QX0.5 : BOOL; END_VAR
    RESOURCE R ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH T : P;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1423] Error: part of a wider address
       ,-[ file:///test0.st:6:14 ]
       |
     6 |     p := REF(named);
       |              ^^|^^
       |                `---- '%QX0.5' is part of '%QW0' and has no address of its own to take a reference to
       |
       | Note: take the reference of '%QW0' as a whole
    ---'
    ");
}

/// A FOR counter needs storage of its own.
#[rstest]
fn invalid_for_counter_in_a_part_of_a_wider_address(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM P
VAR_EXTERNAL cnt : USINT; whole : WORD; END_VAR
VAR x : BOOL; END_VAR
    FOR cnt := 1 TO 3 DO x := TRUE; END_FOR;
    whole := whole + 1;
END_PROGRAM

CONFIGURATION Cfg
VAR_GLOBAL whole AT %MW0 : WORD; cnt AT %MB1 : USINT; END_VAR
    RESOURCE R ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH T : P;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1423] Error: part of a wider address
       ,-[ file:///test0.st:5:9 ]
       |
     5 |     FOR cnt := 1 TO 3 DO x := TRUE; END_FOR;
       |         ^|^
       |          `--- '%MB1' is part of '%MW0' and cannot count a FOR loop
       |
       | Note: count in a variable and assign '%MB1' from it
    ---'
    ");
}

/// A declared part persists only as its owner does, so RETAIN on it is
/// refused. Its type is any of its width, signed or not.
#[rstest]
fn invalid_retain_on_a_declared_part(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM P
VAR_EXTERNAL whole : WORD; lo : USINT; hi : SINT; END_VAR
    whole := whole + 1;
END_PROGRAM

CONFIGURATION Cfg
VAR_GLOBAL
    whole AT %MW0 : WORD;
    lo    AT %MB0 : USINT;
    hi    AT %MB1 : SINT;
END_VAR
VAR_GLOBAL RETAIN
    keep AT %MX0.2 : BOOL;
END_VAR
    RESOURCE R ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH T : P;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1423] Error: part of a wider address
        ,-[ file:///test0.st:14:5 ]
        |
     14 |     keep AT %MX0.2 : BOOL;
        |     ^^^^^^^^^^|^^^^^^^^^^
        |               `------------ '%MX0.2' is part of '%MW0' and cannot be RETAIN on its own
        |
        | Note: RETAIN belongs on the variable located at '%MW0', whose storage this is
    ----'
    ");
}

/// `__init` would write an input's initial value, and the host's copy-in
/// before the first scan overwrites it before anything reads it.
#[rstest]
fn invalid_initial_value_on_an_input(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM P
VAR_EXTERNAL sensor : BOOL; END_VAR
VAR t : BOOL; END_VAR
    t := sensor;
END_PROGRAM

CONFIGURATION Cfg
VAR_GLOBAL sensor AT %IX0.0 : BOOL := TRUE; END_VAR
    RESOURCE R ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH T : P;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1419] Error: write to an input location
       ,-[ file:///test0.st:9:12 ]
       |
     9 | VAR_GLOBAL sensor AT %IX0.0 : BOOL := TRUE; END_VAR
       |            ^^^^^^^^^^^^^^^|^^^^^^^^^^^^^^^
       |                           `----------------- '%IX0.0' is an input, so an initial value is overwritten before anything reads it
       |
       | Note: the host writes the input image before every scan, the first one included
    ---'
    ");
}

/// A part has no cell of its own for `__init` to write; its initial value
/// was silently dropped. The bit belongs in the owner's initial value.
#[rstest]
fn invalid_initial_value_on_a_part(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM P
VAR_EXTERNAL lamps : WORD; END_VAR
    lamps := lamps;
END_PROGRAM

CONFIGURATION Cfg
VAR_GLOBAL
    lamps AT %QW0   : WORD := 16#00F0;
    lamp  AT %QX0.3 : BOOL := TRUE;
END_VAR
    RESOURCE R ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH T : P;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1423] Error: part of a wider address
        ,-[ file:///test0.st:10:5 ]
        |
     10 |     lamp  AT %QX0.3 : BOOL := TRUE;
        |     ^^^^^^^^^^^^^^^|^^^^^^^^^^^^^^
        |                    `---------------- '%QX0.3' is part of '%QW0' and cannot have an initial value of its own
        |
        | Note: give the variable located at '%QW0' an initial value with this part set in it
    ----'
    ");
}

/// An output's initial value is the state it starts in, and a marker's is
/// its starting value: both are the program's to set.
#[rstest]
fn valid_initial_values_on_outputs_and_markers(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM P
VAR_EXTERNAL lamps : WORD; count : INT; END_VAR
    count := count + 1;
    lamps := lamps;
END_PROGRAM

CONFIGURATION Cfg
VAR_GLOBAL
    lamps AT %QW0 : WORD := 16#00F0;
    count AT %MW0 : INT := 10;
END_VAR
    RESOURCE R ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH T : P;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}
