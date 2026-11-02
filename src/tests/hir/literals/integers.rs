// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::add_source;
use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

#[rstest]
fn hex_literal_with_underscores(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        a: DWORD := 16#0000_0001;
        b: DWORD := 16#DEAD_BEEF;
        c: WORD := 16#FF_00;
        d: BYTE := 16#F_F;
    END_VAR
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn binary_literal_with_underscores(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        a: BYTE := 2#1010_0101;
        b: WORD := 2#1111_0000_1010_0101;
        c: BYTE := 2#1111_1111;
    END_VAR
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn octal_literal_with_underscores(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        a: WORD := 8#177_777;
        b: BYTE := 8#3_7_7;
    END_VAR
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn decimal_literal_with_underscores(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        a: DINT := 1_000_000;
        b: INT := 10_00;
        c: LINT := 1_000_000_000_000;
    END_VAR
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn signed_literal_with_underscores(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        a: DINT := -1_000;
        b: INT := +1_000;
    END_VAR
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn typed_literal_with_underscores(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        a: DWORD := DWORD#16#FFFF_0000;
        b: WORD := WORD#16#FF_00;
        c: BYTE := BYTE#2#1010_0101;
        d: DINT := DINT#1_000_000;
    END_VAR
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

// A radix literal is a bit pattern of its type's width, signed types
// included: `SINT#16#FF` is -1, as `DINT#16#FFFF_FFFF` is. Eight and
// sixteen bits refused what thirty-two and sixty-four took.
#[rstest]
fn valid_radix_literal_fills_a_signed_type(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        a: SINT := SINT#16#FF;
        b: SINT := 16#80;
        c: SINT := SINT#2#1111_1111;
        d: INT := INT#16#FFFF;
        e: INT := 8#177777;
        f: DINT := DINT#16#FFFF_FFFF;
        g: LINT := 16#FFFF_FFFF_FFFF_FFFF;
        h: SINT (-10..10) := 16#FF;
    END_VAR
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

// A based literal has no sign, so `-16#10` is the unary operator on `16#10`,
// as `- 16#10` is, wherever an expression stands. The lexer took `-16` as the
// longest token there and stopped at the `#`.
#[rstest]
fn valid_sign_before_a_based_literal(mut with_db: RootDatabase) {
    let source = r#"
TYPE Narrow : INT (-16#10 .. +16#10) := -16#1; END_TYPE
TYPE Level : INT (Low := -16#10, High := +2#11); END_TYPE
TYPE Row : ARRAY[-16#1 .. +16#1] OF INT; END_TYPE
TYPE Point :
    STRUCT
        x : INT := -16#10;
        y : INT := +8#7;
    END_STRUCT;
END_TYPE

FUNCTION Take : INT
VAR_INPUT p : INT := -16#10; END_VAR
    Take := p;
END_FUNCTION

FUNCTION Forms : INT
VAR
    a : INT := -16#10;
    b : DINT := -2#101;
    c : LINT := -8#17;
    d : INT := -16#1_0;
    e : INT := +16#10;
    f : REAL := -16#10;
    g : ARRAY[0..3] OF INT := [-16#1, 2(-2#1), [3] := -8#1];
    h : Point := (x := -16#1);
    i : INT;
    j : Row;
    k : Narrow;
    l : Level := Level#Low;
END_VAR
    a := a - -16#10;
    a := a * +16#10;
    a := a-16#10;
    a := - -16#10;
    a := Take(-16#10);
    a := Take(p := -16#10);
    j[-16#1] := 1;
    k := -16#2;
    FOR i := 3 TO 0 BY -16#1 DO
        a := i;
    END_FOR;
    CASE a OF
        -16#10: a := 1;
        -16#F..-16#1: a := 2;
        -2#10_0000, -8#100: a := 3;
    END_CASE;
    Forms := a;
END_FUNCTION
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

// The sign of `-16#10` is a token because of what stands past the digits. An
// edit there has to read the sign again: the tree the editor ends on is the
// one a fresh parse gives.
#[rstest]
#[case::typing_the_base("-16", 3, 0, "#10", "-16#10")]
#[case::deleting_the_base("-16#10", 3, 3, "", "-16")]
#[case::deleting_the_hash("-2#1", 2, 1, "", "-21")]
#[case::typing_the_hash("-21", 2, 0, "#", "-2#1")]
#[case::another_base("-16#10", 1, 2, "8", "-8#10")]
#[case::a_real_instead("-16#10", 3, 3, ".5", "-16.5")]
#[case::closing_the_space("- 16#10", 1, 1, "", "-16#10")]
#[case::opening_a_space("-16#10", 1, 0, " ", "- 16#10")]
#[case::a_subtraction_no_more("a-16#10", 0, 1, "", "-16#10")]
#[case::a_subtraction_now("-16#10", 0, 0, "a", "a-16#10")]
fn an_edit_past_a_sign_reads_the_sign_again(
    mut with_db: RootDatabase,
    #[case] before: &str,
    #[case] at: u32,
    #[case] remove: u32,
    #[case] insert: &str,
    #[case] after: &str,
) {
    use auto_lsp::lsp_types::{
        DidChangeTextDocumentParams, Position, Range, TextDocumentContentChangeEvent,
        VersionedTextDocumentIdentifier,
    };

    let source =
        |value: &str| format!("PROGRAM P\nVAR a : REAL; END_VAR\na := {value};\nEND_PROGRAM\n");
    let file = add_source(&mut with_db, &source(before));
    let start = "a := ".len() as u32 + at;
    let event = DidChangeTextDocumentParams {
        text_document: VersionedTextDocumentIdentifier {
            uri: file.url(&with_db).clone(),
            version: 1,
        },
        content_changes: vec![TextDocumentContentChangeEvent {
            range: Some(Range::new(
                Position::new(2, start),
                Position::new(2, start + remove),
            )),
            range_length: None,
            text: insert.into(),
        }],
    };
    file.update_edit(&mut with_db, &event).unwrap();
    assert_eq!(file.document(&with_db).texter.text, source(after));

    let mut fresh_db = RootDatabase::default();
    let fresh = add_source(&mut fresh_db, &source(after));
    assert_eq!(
        file.document(&with_db).tree.root_node().to_sexp(),
        fresh.document(&fresh_db).tree.root_node().to_sexp(),
    );
}
