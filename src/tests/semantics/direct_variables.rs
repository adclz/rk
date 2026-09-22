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
    [E1417] Error: direct variable access is not supported
       ,-[ file:///test0.st:7:13 ]
       |
     7 |     test := %IX0.0
       |             ^^^|^^
       |                `---- '%IX0.0' cannot be read or written: there is no I/O mapping
       |
       | Note: the address is understood and X/B/W/D/L names the width, but nothing connects it to a process image yet
    ---'
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
    [E1417] Error: direct variable access is not supported
       ,-[ file:///test0.st:7:13 ]
       |
     7 |     test := %IB0.0
       |             ^^^|^^
       |                `---- '%IB0.0' cannot be read or written: there is no I/O mapping
       |
       | Note: the address is understood and X/B/W/D/L names the width, but nothing connects it to a process image yet
    ---'
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:7:13 ]
       |
     4 |         test: REAL;
       |         ^^|^
       |           `--- type is declared by variable 'test' here
       |
     7 |     test := %IB0.0
       |             ^^^|^^
       |                `---- expected 'REAL', got 'BOOL'
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
    [E1417] Error: direct variable access is not supported
       ,-[ file:///test0.st:7:13 ]
       |
     7 |     test := %IW0
       |             ^^|^
       |               `--- '%IW0' cannot be read or written: there is no I/O mapping
       |
       | Note: the address is understood and X/B/W/D/L names the width, but nothing connects it to a process image yet
    ---'
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
    [E1417] Error: direct variable access is not supported
       ,-[ file:///test0.st:7:13 ]
       |
     7 |     test := %ID0
       |             ^^|^
       |               `--- '%ID0' cannot be read or written: there is no I/O mapping
       |
       | Note: the address is understood and X/B/W/D/L names the width, but nothing connects it to a process image yet
    ---'
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
    [E1417] Error: direct variable access is not supported
       ,-[ file:///test0.st:7:13 ]
       |
     7 |     test := %IL0
       |             ^^|^
       |               `--- '%IL0' cannot be read or written: there is no I/O mapping
       |
       | Note: the address is understood and X/B/W/D/L names the width, but nothing connects it to a process image yet
    ---'
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

/// A direct variable is TYPED (the width letter decides it) but nothing maps
/// it to a process image, so it cannot be lowered. It is refused at check —
/// this used to pass `rk check` with zero errors and then kill `rk compile`
/// with an internal compiler error telling the user to file an issue.
#[rstest]
fn unsupported_in_a_body(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        test: BOOL;
    END_VAR

    test := %IX0.0;

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1417] Error: direct variable access is not supported
       ,-[ file:///test0.st:7:13 ]
       |
     7 |     test := %IX0.0;
       |             ^^^|^^
       |                `---- '%IX0.0' cannot be read or written: there is no I/O mapping
       |
       | Note: the address is understood and X/B/W/D/L names the width, but nothing connects it to a process image yet
    ---'
    ");
}

/// Writing one is refused the same way as reading one.
#[rstest]
fn unsupported_as_an_assignment_target(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    %QX0.1 := TRUE;
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1417] Error: direct variable access is not supported
       ,-[ file:///test0.st:3:5 ]
       |
     3 |     %QX0.1 := TRUE;
       |     ^^^|^^
       |        `---- '%QX0.1' cannot be read or written: there is no I/O mapping
       |
       | Note: the address is understood and X/B/W/D/L names the width, but nothing connects it to a process image yet
    ---'
    ");
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

/// A POU's variables are instance fields, and an instance is laid out as a
/// unit — so one of its fields cannot also sit in a band the host copies
/// whole. A location there keeps the refusal it has always had.
#[rstest]
fn unsupported_located_variable_in_a_pou(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM pgm
VAR
    VALVE_POS AT %QW28 : INT;
END_VAR
END_PROGRAM"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1417] Error: direct variable access is not supported
       ,-[ file:///test0.st:4:5 ]
       |
     4 |     VALVE_POS AT %QW28 : INT;
       |     ^^^^^^^^^^^^|^^^^^^^^^^^
       |                 `------------- '%QW28' cannot be read or written: there is no I/O mapping
       |
       | Note: the address is understood and X/B/W/D/L names the width, but nothing connects it to a process image yet
    ---'
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
    [E1417] Error: direct variable access is not supported
       ,-[ file:///test0.st:9:12 ]
       |
     9 | VAR_GLOBAL sensor AT %I* : BOOL; END_VAR
       |            ^^^^^^^^^^|^^^^^^^^^
       |                      `----------- '%I*' cannot be read or written: there is no I/O mapping
       |
       | Note: the address is understood and X/B/W/D/L names the width, but nothing connects it to a process image yet
    ---'
    ");
}
