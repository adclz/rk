// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

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
