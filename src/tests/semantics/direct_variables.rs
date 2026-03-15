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
       | Help: insert explicit cast 'REAL_TO_DWORD(%ID0)'
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
        By: BYTE;
        Wo: WORD;
        Do: DWORD;
        Lo: LWORD;
    END_VAR

    Bo:= By.%X0; // bit 0 of By
    Bo:= By.7; // bit 7 of By; %X is the default and may be omitted.
    Bo:= Lo.63 // bit 63 of Lo;
    By:= Wo.%B1; // byte 1 of Wo;

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn mismatch_multibits_bool(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        By: REAL;
        Wo: WORD;
    END_VAR

    By:= Wo.%X0; // bit 0 of Wo (invalid because we expect a REAL)
    By:= Wo.%1; // bit 1 of Wo (same but with omitted %X)

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:8:10 ]
       |
     4 |         By: REAL;
       |         ^|
       |          `-- type is declared by variable 'By' here
       |
     8 |     By:= Wo.%X0; // bit 0 of Wo (invalid because we expect a REAL)
       |          ^^^|^^
       |             `---- expected 'REAL', got 'BOOL'
    ---'
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:9:10 ]
       |
     4 |         By: REAL;
       |         ^|
       |          `-- type is declared by variable 'By' here
       |
     9 |     By:= Wo.%1; // bit 1 of Wo (same but with omitted %X)
       |          ^^|^^
       |            `---- expected 'REAL', got 'BOOL'
    ---'
    ");
}

#[rstest]
fn mismatch_multibits_byte(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        By: REAL;
        Wo: WORD;
    END_VAR

    By:= Wo.%B0; // byte 0 of Wo (invalid because we expect a REAL)

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:8:10 ]
       |
     4 |         By: REAL;
       |         ^|
       |          `-- type is declared by variable 'By' here
       |
     8 |     By:= Wo.%B0; // byte 0 of Wo (invalid because we expect a REAL)
       |          ^^^|^^
       |             `---- expected 'REAL', got 'BYTE'
    ---'
    ");
}

#[rstest]
fn mismatch_multibits_word(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        By: REAL;
        Do: DWORD;
    END_VAR

    By:= Do.%W0; // word 0 of Do (invalid because we expect a REAL)

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:8:10 ]
       |
     4 |         By: REAL;
       |         ^|
       |          `-- type is declared by variable 'By' here
       |
     8 |     By:= Do.%W0; // word 0 of Do (invalid because we expect a REAL)
       |          ^^^|^^
       |             `---- expected 'REAL', got 'WORD'
    ---'
    ");
}

#[rstest]
fn mismatch_multibits_d_word(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        By: REAL;
        Lo: LWORD;
    END_VAR

    By:= Lo.%D0; // dword 0 of Lo (invalid because we expect a REAL)

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:8:10 ]
       |
     4 |         By: REAL;
       |         ^|
       |          `-- type is declared by variable 'By' here
       |
     8 |     By:= Lo.%D0; // dword 0 of Lo (invalid because we expect a REAL)
       |          ^^^|^^
       |             `---- expected 'REAL', got 'DWORD'
       |             |
       |             `---- consider explicitly casting with 'DWORD_TO_REAL(Lo.%D0)'
       |
       | Help: insert explicit cast 'REAL_TO_DWORD(Lo.%D0)'
    ---'
    ");
}

#[rstest]
fn mismatch_multibits_l_word(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        By: REAL;
        Lo: LWORD;
    END_VAR

    By:= Lo.%L0; // lword 0 of Lo (invalid because we expect a REAL)

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:8:10 ]
       |
     4 |         By: REAL;
       |         ^|
       |          `-- type is declared by variable 'By' here
       |
     8 |     By:= Lo.%L0; // lword 0 of Lo (invalid because we expect a REAL)
       |          ^^^|^^
       |             `---- expected 'REAL', got 'LWORD'
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
    [E0229] Error: multibit access out of range
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
    [E0229] Error: multibit access out of range
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
    [E0229] Error: multibit access out of range
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
