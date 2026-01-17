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
    Error: 
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
    Error: 
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
    Error: 
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
    Error: 
       ,-[ file:///test0.st:7:13 ]
       |
     4 |         test: REAL;
       |         ^^|^  
       |           `--- type is declared by variable 'test' here
       | 
     7 |     test := %ID0
       |             ^^|^  
       |               `--- expected 'REAL', got 'DWORD'
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
    Error: 
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
    Error: 
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
    Error: 
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
    Error: 
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
    Error: 
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
    Error: 
       ,-[ file:///test0.st:8:10 ]
       |
     4 |         By: REAL;
       |         ^|  
       |          `-- type is declared by variable 'By' here
       | 
     8 |     By:= Lo.%D0; // dword 0 of Lo (invalid because we expect a REAL)
       |          ^^^|^^  
       |             `---- expected 'REAL', got 'DWORD'
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
    Error: 
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
