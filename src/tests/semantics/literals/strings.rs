use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

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
fn valid_wstring_literal(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        s1 : WSTRING := "hello";
        s2 : WSTRING := WSTRING#"world";
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
fn valid_wchar_literal(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        c1 : WCHAR := WCHAR#"x";
    END_VAR
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn invalid_string_to_wstring_implicit(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        s : WSTRING := 'hello';
    END_VAR
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:4:21 ]
       |
     4 |         s : WSTRING := 'hello';
       |                     ^^^^^|^^^^
       |                          `------ expected 'WSTRING', got 'STRING'
    ---'
    ");
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
fn invalid_char_to_wchar_implicit(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        c : WCHAR := CHAR#'x';
    END_VAR
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:4:19 ]
       |
     4 |         c : WCHAR := CHAR#'x';
       |                   ^^^^^|^^^^^
       |                        `------- expected 'WCHAR', got 'CHAR'
    ---'
    ");
}

#[rstest]
fn invalid_char_to_wstring_implicit(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        s : WSTRING := CHAR#'x';
    END_VAR
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:4:21 ]
       |
     4 |         s : WSTRING := CHAR#'x';
       |                     ^^^^^|^^^^^
       |                          `------- expected 'WSTRING', got 'CHAR'
    ---'
    ");
}

#[rstest]
fn valid_wchar_to_wstring_implicit(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        s : WSTRING := WCHAR#"x";
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
fn invalid_wchar_too_long(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        c : WCHAR := WCHAR#"ab";
    END_VAR
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r#"
    [E0309] Error: invalid literal
       ,-[ file:///test0.st:4:22 ]
       |
     4 |         c : WCHAR := WCHAR#"ab";
       |                      ^^^^^|^^^^
       |                           `------ cannot infer '<char>' to 'WCHAR': WCHAR literal must be exactly 1 character, got 2
    ---'
    "#);
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
fn valid_wstring_with_length(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        s1 : WSTRING[100] := "hello";
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
fn invalid_wstring_exceeds_length(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        s : WSTRING[3] := "hello world";
    END_VAR
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r#"
    [E0309] Error: invalid literal
       ,-[ file:///test0.st:4:27 ]
       |
     4 |         s : WSTRING[3] := "hello world";
       |                           ^^^^^^|^^^^^^
       |                                 `-------- cannot infer '<string>' to 'WSTRING': WSTRING literal exceeds maximum length of 3, got 11
    ---'
    "#);
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

#[rstest]
fn valid_wstring_to_string_coercion(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        s : STRING := "hello";
    END_VAR
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r#"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:4:20 ]
       |
     4 |         s : STRING := "hello";
       |                    ^^^^^|^^^^
       |                         `------ expected 'STRING', got 'WSTRING'
    ---'
    "#);
}
