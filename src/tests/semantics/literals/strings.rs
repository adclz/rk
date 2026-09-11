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

/// A CHAR does not widen to STRING: the widening is an encoding, and the
/// cast machinery has no STRING lane. Accepted, this checked clean and
/// died in MIR ("STRING has no scalar MIR representation"), as a literal
/// initializer, a literal assignment and a variable assignment alike.
#[rstest]
fn invalid_char_literal_does_not_initialize_a_string(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        s : STRING := CHAR#'x';
    END_VAR
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:4:20 ]
       |
     4 |         s : STRING := CHAR#'x';
       |                    ^^^^^|^^^^^
       |                         `------- expected 'STRING', got 'CHAR'
    ---'
    ");
}

/// A CHAR literal is one character of any script, worth its code point:
/// one byte is the Latin-1 range, anything else must be the UTF-8 form of
/// exactly one character.
#[rstest]
fn valid_char_literal_is_one_character(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        a : CHAR := CHAR#'A';
        e : CHAR := CHAR#'é';
        z : CHAR := CHAR#'中';
        b : CHAR := CHAR#'$E9';
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
    [E0306] Error: invalid literal
       ,-[ file:///test0.st:4:21 ]
       |
     4 |         c : CHAR := CHAR#'ab';
       |                     ^^^^|^^^^
       |                         `------ cannot infer '<char>' to 'CHAR': CHAR literal must be exactly 1 character, got 2
    ---'
    ");
}

/// A bare literal carries no type of its own, exactly like a bare number:
/// the slot decides. `'A'` is a STRING where a STRING is expected and a CHAR
/// where a CHAR is, so the typed `CHAR#'A'` becomes the way to SAY it, not the
/// only way to write it.
#[rstest]
fn valid_bare_literal_is_a_char_where_a_char_is_expected(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn1 : INT
    VAR
        a : CHAR := 'A';
        e : CHAR := 'e';
        z : CHAR := '$E9';
        s : STRING := 'A';
        c : CHAR;
    END_VAR
    c := 'Z';
    s := 'hello';
    fn1 := 0;
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// The length rule is the CHAR literal's, whichever form wrote it.
#[rstest]
fn invalid_bare_literal_too_long_for_a_char(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        c : CHAR := 'ab';
    END_VAR
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0306] Error: invalid literal
       ,-[ file:///test0.st:4:21 ]
       |
     4 |         c : CHAR := 'ab';
       |                     ^^|^
       |                       `--- cannot infer '<string>' to 'CHAR': CHAR literal must be exactly 1 character, got 2
    ---'
    ");
}

/// Being untyped does not make the literal a wildcard: a slot that takes
/// neither STRING nor CHAR leaves it at its default type and refuses it
/// there, the way `t : TIME := 5` is refused as an INT.
#[rstest]
fn invalid_bare_literal_in_a_numeric_slot(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        t : TIME := 'ab';
    END_VAR
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:4:18 ]
       |
     4 |         t : TIME := 'ab';
       |                  ^^^|^^^
       |                     `----- expected 'TIME', got 'STRING'
    ---'
    ");
}

/// A CHAR still does not widen to STRING - that widening is an encoding the
/// cast machinery has no lane for. The literal being untyped changes what
/// `'A'` may BECOME, not what a CHAR-typed value converts to.
#[rstest]
fn invalid_char_variable_still_does_not_initialize_a_string(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn1 : INT
    VAR
        c : CHAR := 'A';
        s : STRING;
    END_VAR
    s := c;
    fn1 := 0;
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:7:10 ]
       |
     5 |         s : STRING;
       |         |
       |         `-- type is declared by variable 's' here
       |
     7 |     s := c;
       |          |
       |          `-- expected 'STRING', got 'CHAR'
       |          |
       |          `-- consider explicitly casting with 'CHAR_TO_STRING(c)'
       |
       | Help: insert explicit cast 'CHAR_TO_STRING(c)'
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
    [E0306] Error: invalid literal
       ,-[ file:///test0.st:4:20 ]
       |
     4 |         x : INT := 'hello';
       |                    ^^^|^^^
       |                       `----- cannot infer '<string>' to 'INT': cannot use string literal as INT
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
    [E0306] Error: invalid literal
       ,-[ file:///test0.st:4:23 ]
       |
     4 |         s : STRING := 42;
       |                       ^|
       |                        `-- cannot infer '<integer>' to 'STRING': cannot use numeric literal as STRING
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
    [E0306] Error: invalid literal
       ,-[ file:///test0.st:4:26 ]
       |
     4 |         s : STRING[2] := 'hello';
       |                          ^^^|^^^
       |                             `----- cannot infer '<string>' to 'STRING': STRING literal exceeds the capacity of 2 bytes, got 5
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
    [E0306] Error: invalid literal
       ,-[ file:///test0.st:5:18 ]
       |
     5 |             s := 'toolong';
       |                  ^^^^|^^^^
       |                      `------ cannot infer '<string>' to 'STRING': STRING literal exceeds the capacity of 4 bytes, got 7
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
    [E0307] Error: length is not constant
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
    [E0306] Error: invalid literal
       ,-[ file:///test0.st:7:22 ]
       |
     7 |             sized := 'far too long for five';
       |                      ^^^^^^^^^^^|^^^^^^^^^^^
       |                                 `------------- cannot infer '<string>' to 'STRING': STRING literal exceeds the capacity of 5 bytes, got 21
    ---'
    [E0306] Error: invalid literal
       ,-[ file:///test0.st:8:22 ]
       |
     8 |             plain := 'this literal is well beyond eighty characters long, which is the silent default capacity a plain STRING declaration gets when nothing is said';
       |                      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^|^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
       |                                                                                             `------------------------------------------------------------------------- cannot infer '<string>' to 'STRING': STRING literal exceeds the capacity of 80 bytes, got 141
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
/// unchecked, which is what the assignment door was doing before E0306
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

/// An ARRAY ELEMENT is a destination like any other. A subscripted target still
/// resolves to the array VARIABLE, so the capacity has to be read from the
/// element spec — reading the array's own spec found no string and measured
/// nothing, and `a[1] := <over-long>` was cut silently while the same literal
/// into a plain `STRING[4]` was refused.
#[rstest]
fn assigned_literal_must_fit_an_array_element(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION f : INT
        VAR
            a : ARRAY[0..1] OF STRING[4];
        END_VAR
            a[1] := 'ABCDEFGHIJKLMNOP';
            f := 1;
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0306] Error: invalid literal
       ,-[ file:///test0.st:6:21 ]
       |
     6 |             a[1] := 'ABCDEFGHIJKLMNOP';
       |                     ^^^^^^^^^|^^^^^^^^
       |                              `---------- cannot infer '<string>' to 'STRING': STRING literal exceeds the capacity of 4 bytes, got 16
    ---'
    ");
}

/// The same through an alias, and through two dimensions — the walk to the
/// element descends every array hop, not just the first.
#[rstest]
fn assigned_literal_must_fit_a_nested_array_element(mut with_db: RootDatabase) {
    let source = r#"
        TYPE Small : STRING[4]; END_TYPE

        FUNCTION f : INT
        VAR
            a : ARRAY[0..1] OF Small;
            b : ARRAY[0..1, 0..1] OF STRING[4];
        END_VAR
            a[1] := 'ABCDEFGHIJKLMNOP';
            b[1, 1] := 'ABCDEFGHIJKLMNOP';
            f := 1;
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0306] Error: invalid literal
       ,-[ file:///test0.st:9:21 ]
       |
     9 |             a[1] := 'ABCDEFGHIJKLMNOP';
       |                     ^^^^^^^^^|^^^^^^^^
       |                              `---------- cannot infer '<string>' to 'STRING': STRING literal exceeds the capacity of 4 bytes, got 16
    ---'
    [E0306] Error: invalid literal
        ,-[ file:///test0.st:10:24 ]
        |
     10 |             b[1, 1] := 'ABCDEFGHIJKLMNOP';
        |                        ^^^^^^^^^|^^^^^^^^
        |                                 `---------- cannot infer '<string>' to 'STRING': STRING literal exceeds the capacity of 4 bytes, got 16
    ----'
    ");
}

/// Comment markers inside a literal are characters, not comments. The
/// literal was a sequence of tokens, so the lexer could take a comment extra
/// between the quote and the text: `'a(*b'` opened a comment that swallowed
/// the rest of the file, with a "no item a" and two missing tokens to show
/// for it.
#[rstest]
fn valid_comment_markers_inside_a_string_literal(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION fn1 : STRING
        VAR
            s : STRING := 'a(*b*)c//d/*e';
            w : STRING := "x(*y*)z//w/*v";
            url : STRING := 'https://example.com/a?b=1&c=2#top';
        END_VAR
            fn1 := s;
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// A literal's characters are their UTF-8 bytes, so the capacity check
/// counts bytes: `'café'` fits a STRING[5] and not a STRING[4], and `'€'`
/// is three. Transcoding to Latin-1 used to make `'café'` four bytes the
/// runtime could not decode and refuse `'€'` outright.
#[rstest]
fn valid_string_literal_is_utf8_bytes(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION fn1
        VAR
            s : STRING := 'café €';
            t : STRING[5] := 'café';
            e : STRING[3] := '€';
        END_VAR
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn invalid_string_literal_capacity_counts_bytes(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION fn1
        VAR
            t : STRING[4] := 'café';
        END_VAR
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0306] Error: invalid literal
       ,-[ file:///test0.st:4:30 ]
       |
     4 |             t : STRING[4] := 'café';
       |                              ^^^|^^
       |                                 `---- cannot infer '<string>' to 'STRING': STRING literal exceeds the capacity of 4 bytes, got 5
    ---'
    ");
}
