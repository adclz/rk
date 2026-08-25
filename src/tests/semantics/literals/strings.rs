use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

// STRING is now a single UTF-8 type — the legacy WSTRING / WCHAR variants
// were dropped (UCS-2 wide-string content is losslessly representable in
// UTF-8). Both single-quoted (`'…'`) and double-quoted (`"…"`) literal
// forms still parse and resolve to STRING / CHAR.

#[rstest]
fn valid_string_literal(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        s1 : STRING := 'hello';
        s2 : STRING := STRING#'world';
    END_VAR
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn valid_double_quoted_string_literal(mut with_db: RootDatabase) {
    // The double-quoted form was historically WSTRING; with the unified
    // STRING type it folds into the same semantic — no error expected.
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        s1 : STRING := "hello";
    END_VAR
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn valid_char_literal(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        c1 : CHAR := CHAR#'x';
    END_VAR
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn valid_char_to_string_implicit(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        s : STRING := CHAR#'x';
    END_VAR
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn invalid_char_too_long(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        c : CHAR := CHAR#'ab';
    END_VAR
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0309] Error: invalid literal
       ,-[ file:///test0.st:4:21 ]
       |
     4 |         c : CHAR := CHAR#'ab';
       |                     ^^^^|^^^^
       |                         `------ cannot infer '<char>' to 'CHAR': CHAR literal must be exactly 1 character, got 2
    ---'
    ");
}

#[rstest]
fn invalid_string_to_int_mismatch(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        x : INT := 'hello';
    END_VAR
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:4:17 ]
       |
     4 |         x : INT := 'hello';
       |                 ^^^^^|^^^^
       |                      `------ expected 'INT', got 'STRING'
    ---'
    ");
}

#[rstest]
fn invalid_int_to_string_mismatch(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        s : STRING := 42;
    END_VAR
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:4:20 ]
       |
     4 |         s : STRING := 42;
       |                    ^^|^^
       |                      `---- expected 'STRING', got 'INT'
    ---'
    ");
}

#[rstest]
fn valid_string_with_length(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        s1 : STRING[50] := 'hello';
    END_VAR
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn string_with_length_coerces_with_plain(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        s1 : STRING[50];
        s2 : STRING;
    END_VAR
        s2 := s1;
        s1 := s2;
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn string_with_length_assignment(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        s1 : STRING[50];
        s2 : STRING[100];
    END_VAR
        s2 := s1;
        s1 := s2;
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn invalid_string_exceeds_length(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        s : STRING[2] := 'hello';
    END_VAR
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0309] Error: invalid literal
       ,-[ file:///test0.st:4:26 ]
       |
     4 |         s : STRING[2] := 'hello';
       |                          ^^^|^^^
       |                             `----- cannot infer '<string>' to 'STRING': STRING literal exceeds maximum length of 2, got 5
    ---'
    ");
}

#[rstest]
fn valid_string_within_length(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        s : STRING[10] := 'hello';
    END_VAR
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn valid_string_exact_length(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        s : STRING[5] := 'hello';
    END_VAR
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

// STRING[K] with a CONSTANT length is refused by the GRAMMAR - the sized
// length only accepts a literal. Pinned as the current wall: widening it is
// a grammar change, and until then no non-literal length can reach the
// capacity fold (which would otherwise silently default to 80).
#[rstest]
fn sized_string_constant_length_is_a_syntax_error(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION fn1 : INT
        VAR CONSTANT K : INT := 4; END_VAR
        VAR s : STRING[K]; END_VAR
            s := 'toolong';
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0050] Error: syntax
       ,-[ file:///test0.st:4:23 ]
       |
     4 |         VAR s : STRING[K]; END_VAR
       |                       ^|^
       |                        `--- Unexpected token(s): '[ K ]'
    ---'
    ");
}

/// A literal wider than its destination is refused at the ASSIGNMENT door,
/// as the initializer door always has.
///
/// The store runs at the destination's capacity — 80 unless the spec says
/// otherwise — so an over-long literal was silently cut there: a 149-byte
/// literal read back as its first 80 bytes, from a compile that said nothing.
#[rstest]
fn assigned_literal_must_fit_the_destination(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION f : INT
        VAR
            sized : STRING[5];
            plain : STRING;
        END_VAR
            sized := 'far too long for five';
            plain := 'this literal is well beyond eighty characters long, which is the silent default capacity a plain STRING declaration gets when nothing is said';
            f := 1;
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0309] Error: invalid literal
       ,-[ file:///test0.st:7:22 ]
       |
     7 |             sized := 'far too long for five';
       |                      ^^^^^^^^^^^|^^^^^^^^^^^
       |                                 `------------- cannot infer '<string>' to 'STRING': STRING literal exceeds maximum length of 5, got 21
    ---'
    [E0309] Error: invalid literal
       ,-[ file:///test0.st:8:22 ]
       |
     8 |             plain := 'this literal is well beyond eighty characters long, which is the silent default capacity a plain STRING declaration gets when nothing is said';
       |                      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^|^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
       |                                                                                             `------------------------------------------------------------------------- cannot infer '<string>' to 'STRING': STRING literal exceeds maximum length of 80, got 141
    ---'
    ");
}

/// Filling the destination exactly is not an overflow.
#[rstest]
fn assigned_literal_at_exact_capacity_is_fine(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION f : INT
        VAR
            s : STRING[5];
        END_VAR
            s := 'five!';
            f := 1;
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}
