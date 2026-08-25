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

/// A `STRING[K]` whose length names a CONSTANT: legal, and measured at K.
///
/// The length is a constant EXPRESSION, as an array bound is — the grammar
/// used to accept only a literal and refused this as a syntax error. The
/// capacity has to FOLD, not merely parse: `declared_string_capacity` answering
/// None is read as "unsized" and lowered at the default 80, so a literal is
/// checked against 4 here, not against 80.
#[rstest]
fn sized_string_length_may_name_a_constant(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION fn1 : INT
        VAR CONSTANT K : INT := 4; END_VAR
        VAR s : STRING[K]; END_VAR
            s := 'toolong';
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0309] Error: invalid literal
       ,-[ file:///test0.st:5:18 ]
       |
     5 |             s := 'toolong';
       |                  ^^^^|^^^^
       |                      `------ cannot infer '<string>' to 'STRING': STRING literal exceeds maximum length of 4, got 7
    ---'
    ");
}

/// A length the compiler cannot work out is refused, not defaulted.
///
/// It is part of the type: it decides how many bytes the variable occupies.
/// Silently taking 80 would size the storage wrongly and say nothing.
#[rstest]
fn sized_string_length_must_fold(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION fn1 : INT
        VAR n : INT; END_VAR
        VAR s : STRING[n]; END_VAR
            fn1 := 1;
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0319] Error: length is not constant
       ,-[ file:///test0.st:4:24 ]
       |
     4 |         VAR s : STRING[n]; END_VAR
       |                        |
       |                        `-- a STRING length must be known at compile time
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

/// A capacity is enforced where it CAN be, and only there.
///
/// The compiler knows a LITERAL's length, so an over-long one is refused —
/// every capacity test in this file has a literal on the right for that
/// reason. It does not know a VARIABLE's, so `s5 := s100` says nothing here
/// and the store truncates to the destination instead, which
/// `codegen::string_audit::a_variable_wider_than_its_destination_truncates`
/// pins.
///
/// That is a declared split, not an oversight: refusing `s5 := s100` outright
/// would reject code whose value fits at runtime, and checking it at runtime
/// would cost a length compare on every string assignment. What must not
/// happen is the third thing — a length the compiler COULD have known going
/// unchecked, which is what the assignment door was doing before E0309
/// reached it.
#[rstest]
fn a_variable_source_is_not_length_checked(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION f : INT
        VAR
            wide : STRING[100];
            narrow : STRING[5];
        END_VAR
            narrow := wide;
            f := 1;
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
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
