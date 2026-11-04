// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

#[rstest]
fn invalid_alias_type_for_variadic_variable(mut with_db: RootDatabase) {
    let source = r#"
TYPE MyStruct :
    STRUCT
        field1: INT;
    END_STRUCT
END_TYPE

FUNCTION sum_all : INT
    VAR_INPUT
        args: MyStruct...
    END_VAR

END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0811] Error: type that cannot be variadic
        ,-[ file:///test0.st:10:15 ]
        |
     10 |         args: MyStruct...
        |               ^^^^|^^^
        |                   `----- 'MyStruct' cannot be variadic
        |
        | Note: only an elementary type other than STRING can be variadic
    ----'
    ");
}

/// Each argument of a pack is one plain parameter of its copy, which a STRING and
/// a subrange are not. `rk check` took both, called or not, and the build stopped
/// on an internal compiler error at the first call.
#[rstest]
fn invalid_variadic_string(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION F : BOOL
VAR_INPUT v : STRING...; END_VAR
    F := ...v=;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0811] Error: type that cannot be variadic
       ,-[ file:///test0.st:3:15 ]
       |
     3 | VAR_INPUT v : STRING...; END_VAR
       |               ^^^|^^
       |                  `---- 'STRING' cannot be variadic
       |
       | Note: only an elementary type other than STRING can be variadic
    ---'
    ");
}

#[rstest]
fn invalid_variadic_subrange(mut with_db: RootDatabase) {
    let source = r#"
TYPE Small : INT (0..5); END_TYPE

FUNCTION F : BOOL
VAR_INPUT v : Small...; END_VAR
    F := ...v=;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0811] Error: type that cannot be variadic
       ,-[ file:///test0.st:5:15 ]
       |
     5 | VAR_INPUT v : Small...; END_VAR
       |               ^^|^^
       |                 `---- 'Small' cannot be variadic
       |
       | Note: only an elementary type other than STRING can be variadic
    ---'
    ");
}

/// E0811 alone, however the type is spelled, in each POU a pack can stand in,
/// with a call and a fold that report nothing more.
#[rstest]
#[case::string("", "STRING", "'a'")]
#[case::string_with_a_capacity("", "STRING[10]", "'a'")]
#[case::alias_of_a_string("TYPE T : STRING; END_TYPE", "T", "'a'")]
#[case::alias_of_a_string_with_a_capacity("TYPE T : STRING[10]; END_TYPE", "T", "'a'")]
#[case::alias_of_an_alias_of_a_string("TYPE U : STRING; END_TYPE TYPE T : U; END_TYPE", "T", "'a'")]
#[case::subrange("TYPE T : INT (0..5); END_TYPE", "T", "1")]
#[case::alias_of_a_subrange("TYPE U : INT (0..5); END_TYPE TYPE T : U; END_TYPE", "T", "1")]
fn invalid_variadic_of_a_type_a_pack_cannot_hold(
    #[case] declarations: &str,
    #[case] ty: &str,
    #[case] argument: &str,
) {
    let count_codes = |source: String| {
        let report = test_diagnostics(&mut RootDatabase::default(), &[&source]);
        let codes: Vec<String> = report
            .lines()
            .filter(|l| l.starts_with('['))
            .map(|l| l[1..6].to_string())
            .collect();
        (codes, report)
    };
    let function = format!(
        "{declarations}\n\
         FUNCTION F : BOOL\nVAR_INPUT v : {ty}...; END_VAR\n    F := ...v=;\nEND_FUNCTION\n\
         FUNCTION G : BOOL\n    G := F({argument}, {argument});\nEND_FUNCTION\n"
    );
    let (codes, report) = count_codes(function);
    assert_eq!(codes, ["E0811"], "{report}");

    let method = format!(
        "{declarations}\n\
         FUNCTION_BLOCK B\n\
         METHOD M : BOOL\nVAR_INPUT v : {ty}...; END_VAR\n    M := ...v=;\nEND_METHOD\n\
         END_FUNCTION_BLOCK\n\
         FUNCTION G : BOOL\nVAR b : B; END_VAR\n    G := b.M({argument}, {argument});\nEND_FUNCTION\n"
    );
    let (codes, report) = count_codes(method);
    assert_eq!(codes, ["E0811"], "{report}");

    let uncalled =
        format!("{declarations}\nFUNCTION F : INT\nVAR_INPUT v : {ty}...; END_VAR\nEND_FUNCTION\n");
    let (codes, report) = count_codes(uncalled);
    assert_eq!(codes, ["E0811"], "{report}");
}

/// A composite pack is refused where it is declared, and its fold says no more.
#[rstest]
fn invalid_variadic_of_a_composite_type_reports_nothing_at_its_fold(mut with_db: RootDatabase) {
    let source = r#"
TYPE S : STRUCT x : INT; END_STRUCT; END_TYPE

FUNCTION F : BOOL
VAR_INPUT v : S...; END_VAR
    F := ...v=;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0811] Error: type that cannot be variadic
       ,-[ file:///test0.st:5:15 ]
       |
     5 | VAR_INPUT v : S...; END_VAR
       |               |
       |               `-- 'S' cannot be variadic
       |
       | Note: only an elementary type other than STRING can be variadic
    ---'
    ");
}

/// A type that does not resolve is reported where it is written, and nothing is
/// said of what a pack can hold, or of its fold.
#[rstest]
fn invalid_variadic_of_an_unknown_type_reports_the_name_alone(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION F : BOOL
VAR_INPUT v : Nothing...; END_VAR
    F := ...v=;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0203] Error: unknown name in a namespace
       ,-[ file:///test0.st:3:15 ]
       |
     3 | VAR_INPUT v : Nothing...; END_VAR
       |               ^^^|^^^
       |                  `----- no item found for path 'Nothing'
    ---'
    ");
}

#[rstest]
fn valid_type_for_variadic_variable(mut with_db: RootDatabase) {
    let source = r#"
TYPE MyValue: INT END_TYPE

FUNCTION sum_all : INT
    VAR_INPUT
        args: MyValue...
    END_VAR

END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn invalid_variadic_expression_on_non_variadic_variable(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION sum_all : INT
    VAR_INPUT
        args: INT
    END_VAR
    sum_all := ...args+
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0814] Error: not a variadic parameter
       ,-[ file:///test0.st:6:16 ]
       |
     6 |     sum_all := ...args+
       |                ^^^^|^^^
       |                    `----- 'args' is not a variadic parameter
       |
       | Help: declare one in VAR_INPUT, as `values : INT...`
       |
       | Note: the POU declares no variadic parameter
    ---'
    ");
}

#[rstest]
fn invalid_fold_math_operator_on_non_math_type(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION sum_all : BOOL
    VAR_INPUT
        args: BOOL...
    END_VAR

    sum_all := ...args+

END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0305] Error: operator not supported by the type
       ,-[ file:///test0.st:7:16 ]
       |
     4 |         args: BOOL...
       |         ^^|^
       |           `--- 'args' is declared here
       |
     7 |     sum_all := ...args+
       |                ^^^^|^^^
       |                    `----- operator '+' cannot be applied to type 'BOOL'
    ---'
    ");
}

#[rstest]
fn valid_variadic_function_with_fold_add(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION sum_all : INT
    VAR_INPUT
        args: INT...
    END_VAR
    sum_all := ...args+
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn valid_variadic_function_with_fold_mul(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION product_all : INT
    VAR_INPUT
        args: INT...
    END_VAR
    product_all := ...args*
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn valid_variadic_function_with_fold_and(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION all_true : BOOL
    VAR_INPUT
        args: BOOL...
    END_VAR
    all_true := ...args&
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn valid_variadic_function_with_fold_comparison(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION all_equal : BOOL
    VAR_INPUT
        args: INT...
    END_VAR
    all_equal := ...args=
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn multiple_variadic_variables(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn1 : INT
    VAR_INPUT
        a: INT...
        b: INT...
    END_VAR
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0812] Error: more than one variadic parameter
       ,-[ file:///test0.st:5:9 ]
       |
     4 |         a: INT...
       |         |
       |         `-- first variadic parameter 'a' is declared here
     5 |         b: INT...
       |         |
       |         `-- 'b' is a second variadic parameter
    ---'
    ");
}

#[rstest]
fn valid_variadic_fn_call(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION all_equal : BOOL
    VAR_INPUT
        args: INT...
    END_VAR
    all_equal := ...args=
END_FUNCTION

FUNCTION fn 
    all_equal(1, 2, 3);
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn valid_variadic_fn_call_with_output(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION all_equal : BOOL
    VAR_INPUT
        args: INT...
    END_VAR

    VAR_OUTPUT
        result: BOOL;
    END_VAR
    all_equal := ...args=
END_FUNCTION

FUNCTION fn 
    VAR_OUTPUT
        result: BOOL;
    END_VAR
    
    all_equal(1, 2, 3, result => result);
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn valid_variadic_fn_call_with_in_out(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION sum_all : INT
    VAR_INPUT
        args: INT...
    END_VAR

    VAR_IN_OUT
        accumulator: INT;
    END_VAR
    sum_all := ...args+
END_FUNCTION

FUNCTION fn
    VAR
        acc: INT := 0;
    END_VAR

    sum_all(1, 2, 3, accumulator := acc);
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn valid_variadic_fn_call_with_output_and_in_out(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION sum_all : INT
    VAR_INPUT
        args: INT...
    END_VAR

    VAR_OUTPUT
        count: INT;
    END_VAR

    VAR_IN_OUT
        accumulator: INT;
    END_VAR
    sum_all := ...args+
END_FUNCTION

FUNCTION fn
    VAR
        acc: INT := 0;
        cnt: INT;
    END_VAR

    sum_all(1, 2, 3, count => cnt, accumulator := acc);
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn invalid_variadic_fn_call_type_mismatch(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION sum_all : INT
    VAR_INPUT
        args: INT...
    END_VAR
    sum_all := ...args+
END_FUNCTION

FUNCTION fn
    sum_all(1, 2, 'hello');
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0308] Error: literal of the wrong kind
        ,-[ file:///test0.st:10:19 ]
        |
      4 |         args: INT...
        |         ^^|^
        |           `--- 'args' is declared here
        |
     10 |     sum_all(1, 2, 'hello');
        |                   ^^^|^^^
        |                      `----- cannot use string literal as INT
    ----'
    ");
}

#[rstest]
fn valid_variadic_fn_call_single_arg(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION sum_all : INT
    VAR_INPUT
        args: INT...
    END_VAR
    sum_all := ...args+
END_FUNCTION

FUNCTION fn
    sum_all(42);
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn valid_variadic_fn_call_many_args(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION sum_all : INT
    VAR_INPUT
        args: INT...
    END_VAR
    sum_all := ...args+
END_FUNCTION

FUNCTION fn
    sum_all(1, 2, 3, 4, 5, 6, 7, 8, 9, 10);
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn invalid_input_after_a_variadic(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn1 : INT
    VAR_INPUT
        args: INT...;
        x: INT;
    END_VAR
END_FUNCTION

FUNCTION fn2 : INT
    VAR_INPUT
        before: INT;
        args: INT...;
    END_VAR
    VAR_INPUT
        after: INT;
    END_VAR
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0815] Error: input after a variadic parameter
       ,-[ file:///test0.st:5:9 ]
       |
     4 |         args: INT...;
       |         ^^|^
       |           `--- variadic parameter 'args' is declared here
     5 |         x: INT;
       |         |
       |         `-- 'x' is declared after the variadic parameter 'args'
       |
       | Note: a variadic parameter takes the arguments left in the call
    ---'
    [E0815] Error: input after a variadic parameter
        ,-[ file:///test0.st:15:9 ]
        |
     12 |         args: INT...;
        |         ^^|^
        |           `--- variadic parameter 'args' is declared here
        |
     15 |         after: INT;
        |         ^^|^^
        |           `---- 'after' is declared after the variadic parameter 'args'
        |
        | Note: a variadic parameter takes the arguments left in the call
    ----'
    ");
}

/// The inputs before a variadic parameter take their arguments first, by
/// position or by name, on a FUNCTION and on a METHOD.
#[rstest]
fn valid_inputs_before_a_variadic(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION Pick : INT
    VAR_INPUT
        k : LINT;
        values : INT...;
    END_VAR
    Pick := ...values[k];
END_FUNCTION

FUNCTION_BLOCK Fb
    METHOD Pick : INT
        VAR_INPUT
            k : LINT;
            values : INT...;
        END_VAR
        Pick := ...values[k];
    END_METHOD
END_FUNCTION_BLOCK

FUNCTION Uses : INT
    VAR fb : Fb; END_VAR
    Uses := Pick(0, 1) + Pick(1, 2, 3) + Pick(k := 1, 2, 3) + Pick(2, 3, k := 1);
    Uses := fb.Pick(0, 1) + fb.Pick(k := 1, 2, 3);
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// An input before a variadic parameter is required as any input is, and the
/// variadic parameter still takes at least one argument.
#[rstest]
fn invalid_call_short_of_a_variadic(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION Pick : INT
    VAR_INPUT
        k : LINT;
        values : INT...;
    END_VAR
    Pick := ...values[k];
END_FUNCTION

FUNCTION Uses : INT
    Uses := Pick();
    Uses := Pick(1);
    Uses := Pick(k := 1);
    Uses := Pick(values := 1);
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0813] Error: variadic call without arguments
        ,-[ file:///test0.st:11:13 ]
        |
      5 |         values : INT...;
        |         ^^^|^^
        |            `---- variadic parameter 'values' is declared here
        |
     11 |     Uses := Pick();
        |             ^^|^
        |               `--- the call to 'Pick' passes no argument to the variadic parameter 'values'
    ----'
    [E0802] Error: missing required parameter
        ,-[ file:///test0.st:11:13 ]
        |
      4 |         k : LINT;
        |         |
        |         `-- parameter 'k' is declared here
        |
     11 |     Uses := Pick();
        |             ^^|^
        |               `--- call to 'Pick' is missing 1 required parameter: 'k'
        |
        | Note: a FUNCTION or METHOD call supplies every VAR_INPUT without a default
    ----'
    [E0813] Error: variadic call without arguments
        ,-[ file:///test0.st:12:13 ]
        |
      5 |         values : INT...;
        |         ^^^|^^
        |            `---- variadic parameter 'values' is declared here
        |
     12 |     Uses := Pick(1);
        |             ^^|^
        |               `--- the call to 'Pick' passes no argument to the variadic parameter 'values'
    ----'
    [E0813] Error: variadic call without arguments
        ,-[ file:///test0.st:13:13 ]
        |
      5 |         values : INT...;
        |         ^^^|^^
        |            `---- variadic parameter 'values' is declared here
        |
     13 |     Uses := Pick(k := 1);
        |             ^^|^
        |               `--- the call to 'Pick' passes no argument to the variadic parameter 'values'
    ----'
    [E0802] Error: missing required parameter
        ,-[ file:///test0.st:14:13 ]
        |
      4 |         k : LINT;
        |         |
        |         `-- parameter 'k' is declared here
        |
     14 |     Uses := Pick(values := 1);
        |             ^^|^
        |               `--- call to 'Pick' is missing 1 required parameter: 'k'
        |
        | Note: a FUNCTION or METHOD call supplies every VAR_INPUT without a default
    ----'
    ");
}

/// An index is any integer type, and any expression of one.
#[rstest]
fn valid_index_of_every_integer_type(mut with_db: RootDatabase) {
    let source = r#"
TYPE Small : INT(0..3); END_TYPE
TYPE Named : DINT; END_TYPE

FUNCTION One : INT
    One := 1;
END_FUNCTION

FUNCTION Pick : INT
    VAR_INPUT values : INT...; END_VAR
    VAR
        a : SINT; b : INT; c : DINT; d : LINT;
        e : USINT; f : UINT; g : UDINT; h : ULINT;
        i : Small; j : Named;
        r : ARRAY[0..1] OF INT;
    END_VAR
    VAR CONSTANT two : INT := 2; END_VAR
    Pick := ...values[a] + ...values[b] + ...values[c] + ...values[d];
    Pick := ...values[e] + ...values[f] + ...values[g] + ...values[h];
    Pick := ...values[i] + ...values[j];
    Pick := ...values[0] + ...values[-1] + ...values[two] + ...values[1 + 1];
    Pick := ...values[LINT#5000000000] + ...values[One()] + ...values[r[b]];
    Pick := ...values[...values[0]];
END_FUNCTION

FUNCTION Uses : INT
    Uses := Pick(1, 2, 3);
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn invalid_index_not_an_integer(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION Pick : INT
    VAR_INPUT
        r : REAL;
        values : INT...;
    END_VAR
    Pick := ...values[r];
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0819] Error: variadic index not an integer
       ,-[ file:///test0.st:7:23 ]
       |
     7 |     Pick := ...values[r];
       |                       |
       |                       `-- the index is 'REAL', not an integer
    ---'
    ");
}

/// What a subscript refuses, an index refuses, a bit string included.
#[rstest]
#[case::bool("BOOL")]
#[case::byte("BYTE")]
#[case::lword("LWORD")]
#[case::real("REAL")]
#[case::lreal("LREAL")]
#[case::string("STRING")]
#[case::char("CHAR")]
#[case::time("TIME")]
#[case::date("DATE")]
#[case::enumeration("Color")]
#[case::structure("Point")]
fn invalid_index_of_type(mut with_db: RootDatabase, #[case] ty: &str) {
    let source = format!(
        r#"
TYPE Color : (Red, Green); END_TYPE
TYPE Point : STRUCT x : INT; END_STRUCT; END_TYPE

FUNCTION Pick : INT
    VAR_INPUT values : INT...; END_VAR
    VAR k : {ty}; END_VAR
    Pick := ...values[k];
END_FUNCTION
"#
    );
    let report = test_diagnostics(&mut with_db, &[&source]);
    let codes: Vec<&str> = report
        .lines()
        .filter(|l| l.starts_with('['))
        .map(|l| &l[1..6])
        .collect();
    assert_eq!(codes, ["E0819"], "{report}");
}

/// `...` reads the POU's own variadic parameter. A fault in the index is
/// reported where it is, and the element it leaves says nothing more.
#[rstest]
fn invalid_read_by_position(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION Pick : INT
    VAR_INPUT values : INT...; END_VAR
    VAR x : INT; a : ARRAY[0..1] OF INT; END_VAR
    Pick := ...x[0];
    Pick := ...a[0];
    Pick := ...nothing[0];
    Pick := ...values[missing];
    Pick := ...values[values];
    Pick := ...nothing[missing];
END_FUNCTION

PROGRAM P
    VAR n : INT; END_VAR
    n := ...n[0];
END_PROGRAM
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0814] Error: not a variadic parameter
       ,-[ file:///test0.st:5:13 ]
       |
     3 |     VAR_INPUT values : INT...; END_VAR
       |               ^^^|^^
       |                  `---- the variadic parameter here is 'values'
       |
     5 |     Pick := ...x[0];
       |             ^^^|^^^
       |                `----- 'x' is not a variadic parameter
       |
       | Note: `...values+` adds every argument the call passed, `...values[0]` reads the first
    ---'
    [E0814] Error: not a variadic parameter
       ,-[ file:///test0.st:6:13 ]
       |
     3 |     VAR_INPUT values : INT...; END_VAR
       |               ^^^|^^
       |                  `---- the variadic parameter here is 'values'
       |
     6 |     Pick := ...a[0];
       |             ^^^|^^^
       |                `----- 'a' is not a variadic parameter
       |
       | Note: `...values+` adds every argument the call passed, `...values[0]` reads the first
    ---'
    [E0814] Error: not a variadic parameter
       ,-[ file:///test0.st:7:13 ]
       |
     3 |     VAR_INPUT values : INT...; END_VAR
       |               ^^^|^^
       |                  `---- the variadic parameter here is 'values'
       |
     7 |     Pick := ...nothing[0];
       |             ^^^^^^|^^^^^^
       |                   `-------- 'nothing' is not a variadic parameter
       |
       | Note: `...values+` adds every argument the call passed, `...values[0]` reads the first
    ---'
    [E0201] Error: unknown name
       ,-[ file:///test0.st:8:23 ]
       |
     8 |     Pick := ...values[missing];
       |                       ^^^|^^^
       |                          `----- no item 'missing' found in scope
    ---'
    [E0816] Error: variadic parameter used as a variable
       ,-[ file:///test0.st:9:23 ]
       |
     3 |     VAR_INPUT values : INT...; END_VAR
       |               ^^^|^^
       |                  `---- 'values' is declared variadic here
       |
     9 |     Pick := ...values[values];
       |                       ^^^|^^
       |                          `---- variadic parameter 'values' is used as a variable
       |
       | Help: read one with `...values[0]`, or all of them with a fold: `...values+` adds them
       |
       | Note: a pack stands for as many parameters as the call passed
    ---'
    [E0814] Error: not a variadic parameter
        ,-[ file:///test0.st:10:13 ]
        |
      3 |     VAR_INPUT values : INT...; END_VAR
        |               ^^^|^^
        |                  `---- the variadic parameter here is 'values'
        |
     10 |     Pick := ...nothing[missing];
        |             ^^^^^^^^^|^^^^^^^^^
        |                      `----------- 'nothing' is not a variadic parameter
        |
        | Note: `...values+` adds every argument the call passed, `...values[0]` reads the first
    ----'
    [E0201] Error: unknown name
        ,-[ file:///test0.st:10:24 ]
        |
     10 |     Pick := ...nothing[missing];
        |                        ^^^|^^^
        |                           `----- no item 'missing' found in scope
    ----'
    [E0814] Error: not a variadic parameter
        ,-[ file:///test0.st:15:10 ]
        |
     15 |     n := ...n[0];
        |          ^^^|^^^
        |             `----- 'n' is not a variadic parameter
        |
        | Help: declare one in VAR_INPUT, as `values : INT...`
        |
        | Note: the POU declares no variadic parameter
    ----'
    ");
}

/// An argument read by position is a value and has no place: nothing can take
/// its address or write to it.
#[rstest]
fn invalid_read_by_position_as_a_place(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION Both : INT
    VAR_IN_OUT io : INT; END_VAR
    VAR_OUTPUT o : INT; END_VAR
END_FUNCTION

FUNCTION Pick : INT
    VAR_INPUT values : INT...; END_VAR
    VAR r : REF_TO INT; n : INT; END_VAR
    r := REF(...values[0]);
    Pick := Both(...values[0], o => n);
    Pick := Both(io := n, o => ...values[0]);
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0905] Error: REF() argument not a variable
        ,-[ file:///test0.st:10:14 ]
        |
     10 |     r := REF(...values[0]);
        |              ^^^^^^|^^^^^
        |                    `------- REF() takes a variable, not a value
        |
        | Help: store the value in a variable, and take REF() of the variable
        |
        | Note: a literal, an expression or a call result has no storage to point at
    ----'
    [E0806] Error: VAR_IN_OUT argument not a variable
        ,-[ file:///test0.st:11:18 ]
        |
      3 |     VAR_IN_OUT io : INT; END_VAR
        |                ^|
        |                 `-- parameter 'io' is declared here
        |
     11 |     Pick := Both(...values[0], o => n);
        |                  ^^^^^^|^^^^^
        |                        `------- VAR_IN_OUT parameter 'io' of 'Both' requires a variable, not a value
        |
        | Note: a literal, an expression or a call result has no address for a VAR_IN_OUT to bind
    ----'
    [E0805] Error: VAR_OUTPUT argument not a variable
        ,-[ file:///test0.st:12:32 ]
        |
      4 |     VAR_OUTPUT o : INT; END_VAR
        |                |
        |                `-- parameter 'o' is declared here
        |
     12 |     Pick := Both(io := n, o => ...values[0]);
        |                                ^^^^^^|^^^^^
        |                                      `------- VAR_OUTPUT parameter 'o' of 'Both' requires a variable, not a value
        |
        | Note: the call writes the output into the variable bound to it
    ----'
    ");
}

/// A read by position in a FUNCTION's initializer runs at each call. An input's
/// default and a member's are constants, and it is none.
#[rstest]
fn read_by_position_in_an_initializer(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION Pick : INT
    VAR_INPUT
        k : INT := ...values[0];
        values : INT...;
    END_VAR
    VAR first : INT := ...values[0]; END_VAR
    Pick := k + first;
END_FUNCTION

FUNCTION_BLOCK Fb
    VAR x : INT := ...values[0]; END_VAR
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0401] Error: initial value not constant
       ,-[ file:///test0.st:4:20 ]
       |
     4 |         k : INT := ...values[0];
       |                    ^^^^^^|^^^^^
       |                          `------- this initial value must be a constant: the caller passes it
    ---'
    [E0401] Error: initial value not constant
        ,-[ file:///test0.st:12:20 ]
        |
     12 |     VAR x : INT := ...values[0]; END_VAR
        |                    ^^^^^^|^^^^^
        |                          `------- this initial value must be a constant: it is fixed before the program runs
    ----'
    [E0814] Error: not a variadic parameter
        ,-[ file:///test0.st:12:20 ]
        |
     12 |     VAR x : INT := ...values[0]; END_VAR
        |                    ^^^^^^|^^^^^
        |                          `------- 'values' is not a variadic parameter
        |
        | Help: declare one in VAR_INPUT, as `values : INT...`
        |
        | Note: the POU declares no variadic parameter
    ----'
    ");
}

/// A variadic pack must collect at least one argument: `...args+` has no value
/// over an empty pack, so an empty call has nothing to fold. Refused in HIR so
/// MIR never receives an arity it cannot lower.
#[rstest]
fn invalid_variadic_fn_call_with_no_arguments(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION sum_all : INT
    VAR_INPUT
        args: INT...
    END_VAR
    sum_all := ...args+
END_FUNCTION

FUNCTION fn1 : INT
    fn1 := sum_all()
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0813] Error: variadic call without arguments
        ,-[ file:///test0.st:10:12 ]
        |
      4 |         args: INT...
        |         ^^|^
        |           `--- variadic parameter 'args' is declared here
        |
     10 |     fn1 := sum_all()
        |            ^^^|^^^
        |               `----- the call to 'sum_all' passes no argument to the variadic parameter 'args'
    ----'
    ");
}

/// One argument is enough — the fold of a single element is that element.
#[rstest]
fn valid_variadic_fn_call_with_one_argument(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION sum_all : INT
    VAR_INPUT
        args: INT...
    END_VAR
    sum_all := ...args+
END_FUNCTION

FUNCTION fn1 : INT
    fn1 := sum_all(1)
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

/// The same rule on a METHOD, the other POU kind that may declare a variadic.
#[rstest]
fn invalid_variadic_method_call_with_no_arguments(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    METHOD PUBLIC sum_all : INT
    VAR_INPUT
        args: INT...
    END_VAR
        sum_all := ...args+
    END_METHOD
END_FUNCTION_BLOCK

FUNCTION fn1 : INT
VAR
    f: fb1;
END_VAR
    fn1 := f.sum_all()
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0813] Error: variadic call without arguments
        ,-[ file:///test0.st:15:12 ]
        |
      5 |         args: INT...
        |         ^^|^
        |           `--- variadic parameter 'args' is declared here
        |
     15 |     fn1 := f.sum_all()
        |            ^^^^|^^^^
        |                `------ the call to 'sum_all' passes no argument to the variadic parameter 'args'
    ----'
    ");
}

/// A fold over a name that is not the POU's pack: a member, a name that is
/// nothing, or a POU without a pack. Each was a silent `Never` and an
/// internal compiler error in MIR.
#[rstest]
fn invalid_fold_over_something_else(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Fb
    VAR m : INT; END_VAR
    METHOD Sum : INT
        VAR_INPUT values : INT...; END_VAR
        Sum := ...m+ + ...unknown+;
    END_METHOD
END_FUNCTION_BLOCK

FUNCTION g : INT
    g := ...nothing+;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0814] Error: not a variadic parameter
       ,-[ file:///test0.st:6:16 ]
       |
     5 |         VAR_INPUT values : INT...; END_VAR
       |                   ^^^|^^
       |                      `---- the variadic parameter here is 'values'
     6 |         Sum := ...m+ + ...unknown+;
       |                ^^|^^
       |                  `---- 'm' is not a variadic parameter
       |
       | Note: `...values+` adds every argument the call passed, `...values[0]` reads the first
    ---'
    [E0814] Error: not a variadic parameter
       ,-[ file:///test0.st:6:24 ]
       |
     5 |         VAR_INPUT values : INT...; END_VAR
       |                   ^^^|^^
       |                      `---- the variadic parameter here is 'values'
     6 |         Sum := ...m+ + ...unknown+;
       |                        ^^^^^|^^^^^
       |                             `------- 'unknown' is not a variadic parameter
       |
       | Note: `...values+` adds every argument the call passed, `...values[0]` reads the first
    ---'
    [E0814] Error: not a variadic parameter
        ,-[ file:///test0.st:11:10 ]
        |
     11 |     g := ...nothing+;
        |          ^^^^^|^^^^^
        |               `------- 'nothing' is not a variadic parameter
        |
        | Help: declare one in VAR_INPUT, as `values : INT...`
        |
        | Note: the POU declares no variadic parameter
    ----'
    ");
}

/// A pack read, written, counted, passed or put in an initializer: only a
/// fold consumes it. The reads panicked in codegen; the writes went nowhere.
#[rstest]
fn invalid_variadic_parameter_outside_a_fold(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION sum_all : INT
    VAR_INPUT values : INT...; END_VAR
    sum_all := ...values+;
END_FUNCTION

FUNCTION misuse : INT
    VAR_INPUT values : INT...; END_VAR
    VAR first : INT := values; i : INT; END_VAR
    misuse := values;
    values := 0;
    FOR values := 1 TO 3 DO i := i + 1; END_FOR;
    misuse := sum_all(values);
    misuse := values[1];
    misuse := values^;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0816] Error: variadic parameter used as a variable
       ,-[ file:///test0.st:9:24 ]
       |
     8 |     VAR_INPUT values : INT...; END_VAR
       |               ^^^|^^
       |                  `---- 'values' is declared variadic here
     9 |     VAR first : INT := values; i : INT; END_VAR
       |                        ^^^|^^
       |                           `---- variadic parameter 'values' is used as a variable
       |
       | Help: read one with `...values[0]`, or all of them with a fold: `...values+` adds them
       |
       | Note: a pack stands for as many parameters as the call passed
    ---'
    [E0816] Error: variadic parameter used as a variable
        ,-[ file:///test0.st:10:15 ]
        |
      8 |     VAR_INPUT values : INT...; END_VAR
        |               ^^^|^^
        |                  `---- 'values' is declared variadic here
        |
     10 |     misuse := values;
        |               ^^^|^^
        |                  `---- variadic parameter 'values' is used as a variable
        |
        | Help: read one with `...values[0]`, or all of them with a fold: `...values+` adds them
        |
        | Note: a pack stands for as many parameters as the call passed
    ----'
    [E0816] Error: variadic parameter used as a variable
        ,-[ file:///test0.st:11:5 ]
        |
      8 |     VAR_INPUT values : INT...; END_VAR
        |               ^^^|^^
        |                  `---- 'values' is declared variadic here
        |
     11 |     values := 0;
        |     ^^^|^^
        |        `---- variadic parameter 'values' is used as a variable
        |
        | Help: read one with `...values[0]`, or all of them with a fold: `...values+` adds them
        |
        | Note: a pack stands for as many parameters as the call passed
    ----'
    [E0816] Error: variadic parameter used as a variable
        ,-[ file:///test0.st:12:9 ]
        |
      8 |     VAR_INPUT values : INT...; END_VAR
        |               ^^^|^^
        |                  `---- 'values' is declared variadic here
        |
     12 |     FOR values := 1 TO 3 DO i := i + 1; END_FOR;
        |         ^^^|^^
        |            `---- variadic parameter 'values' is used as a variable
        |
        | Help: read one with `...values[0]`, or all of them with a fold: `...values+` adds them
        |
        | Note: a pack stands for as many parameters as the call passed
    ----'
    [E0816] Error: variadic parameter used as a variable
        ,-[ file:///test0.st:13:23 ]
        |
      8 |     VAR_INPUT values : INT...; END_VAR
        |               ^^^|^^
        |                  `---- 'values' is declared variadic here
        |
     13 |     misuse := sum_all(values);
        |                       ^^^|^^
        |                          `---- variadic parameter 'values' is used as a variable
        |
        | Help: read one with `...values[0]`, or all of them with a fold: `...values+` adds them
        |
        | Note: a pack stands for as many parameters as the call passed
    ----'
    [E0816] Error: variadic parameter used as a variable
        ,-[ file:///test0.st:14:15 ]
        |
      8 |     VAR_INPUT values : INT...; END_VAR
        |               ^^^|^^
        |                  `---- 'values' is declared variadic here
        |
     14 |     misuse := values[1];
        |               ^^^|^^
        |                  `---- variadic parameter 'values' is used as a variable
        |
        | Help: read one with `...values[0]`, or all of them with a fold: `...values+` adds them
        |
        | Note: a pack stands for as many parameters as the call passed
    ----'
    [E0816] Error: variadic parameter used as a variable
        ,-[ file:///test0.st:15:15 ]
        |
      8 |     VAR_INPUT values : INT...; END_VAR
        |               ^^^|^^
        |                  `---- 'values' is declared variadic here
        |
     15 |     misuse := values^;
        |               ^^^|^^
        |                  `---- variadic parameter 'values' is used as a variable
        |
        | Help: read one with `...values[0]`, or all of them with a fold: `...values+` adds them
        |
        | Note: a pack stands for as many parameters as the call passed
    ----'
    ");
}

/// A PROGRAM's or a FUNCTION_BLOCK's input cannot be a pack: neither has an
/// argument count to be specialized for. Refused as it is parsed, so no
/// variadic variable exists outside a FUNCTION or METHOD.
#[rstest]
fn invalid_variadic_parameter_outside_a_function(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM Main
    VAR_INPUT values : INT...; END_VAR
END_PROGRAM

FUNCTION_BLOCK Fb
    VAR_INPUT values : INT...; END_VAR
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0029] Error: variadic parameter outside a FUNCTION or METHOD
       ,-[ file:///test0.st:3:15 ]
       |
     3 |     VAR_INPUT values : INT...; END_VAR
       |               ^^^^^^^|^^^^^^^
       |                      `--------- a PROGRAM takes no variadic parameter
       |
       | Note: the inputs of a PROGRAM are members of one instance
    ---'
    [E0029] Error: variadic parameter outside a FUNCTION or METHOD
       ,-[ file:///test0.st:7:15 ]
       |
     7 |     VAR_INPUT values : INT...; END_VAR
       |               ^^^^^^^|^^^^^^^
       |                      `--------- a FUNCTION_BLOCK takes no variadic parameter
       |
       | Note: the inputs of a FUNCTION_BLOCK are members of one instance
    ---'
    ");
}

/// `MIN` and `MAX` fold to the pack's own type, in any case, wherever an
/// expression stands.
#[rstest]
fn valid_fold_min_max(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION Take : INT
VAR_INPUT x : INT; END_VAR
    Take := x;
END_FUNCTION

FUNCTION Lo : INT
VAR_INPUT v : INT...; END_VAR
VAR m : INT := ...v MIN; END_VAR
    Lo := m + ...v min * 2 + Take(...v MAX) + Take(x := ...v max);
    IF ...v MIN > 0 AND ...v MAX < 9 THEN
        Lo := 0;
    END_IF;
END_FUNCTION

FUNCTION Hi : REAL
VAR_INPUT v : REAL...; END_VAR
    Hi := ...v MAX;
END_FUNCTION

FUNCTION Use : INT
    Use := Lo(1, 2, 3);
    IF Hi(1.5, 2.5) > 2.0 THEN
        Use := Lo(4);
    END_IF;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// `MIN` and `MAX` are no keywords: a FUNCTION, a pack, a variable and a call
/// take the same words.
#[rstest]
fn min_and_max_stay_names(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION MIN : INT
VAR_INPUT v : INT...; END_VAR
    MIN := ...v MIN;
END_FUNCTION

FUNCTION MAX : INT
VAR_INPUT v : INT...; END_VAR
    MAX := ...v MAX;
END_FUNCTION

FUNCTION Both : INT
VAR_INPUT min : INT...; END_VAR
    Both := ...min MIN + ...min max;
END_FUNCTION

FUNCTION Locals : INT
VAR min : INT := 1; max : INT := 2; END_VAR
    Locals := min + max;
END_FUNCTION

FUNCTION Calls : INT
    Calls := MIN(1, 2) + max(3, 4, 5) + Both(6, 7);
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// A `MIN` or `MAX` fold gives the pack's type, where a comparison fold gives a
/// BOOL.
#[rstest]
fn invalid_fold_min_max_gives_the_type_of_the_pack(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION Lo : BOOL
VAR_INPUT v : INT...; END_VAR
    Lo := ...v MIN;
END_FUNCTION

FUNCTION Hi : INT
VAR_INPUT v : LINT...; END_VAR
    Hi := ...v MAX;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:4:11 ]
       |
     2 | FUNCTION Lo : BOOL
       |          ^|
       |           `-- FUNCTION 'Lo' is declared here, with return type 'BOOL'
       |
     4 |     Lo := ...v MIN;
       |           ^^^^|^^^
       |               `----- expected 'BOOL', got 'INT'
    ---'
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:9:11 ]
       |
     7 | FUNCTION Hi : INT
       |          ^|
       |           `-- FUNCTION 'Hi' is declared here, with return type 'INT'
       |
     9 |     Hi := ...v MAX;
       |           ^^^^|^^^
       |               `----- expected 'INT', got 'LINT'
       |
       | Help: insert explicit cast 'LINT_TO_INT(...v MAX)'
    ---'
    ");
}

/// `MIN` and `MAX` read the POU's own pack and nothing else, as every fold does,
/// and the word needs its space, or it is part of the name.
#[rstest]
fn invalid_fold_min_max_over_something_else(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION F : INT
VAR_INPUT v : INT...; END_VAR
VAR x : INT; END_VAR
    F := ...x MIN;
    F := ...nothing MAX;
    F := ...vMIN;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0814] Error: not a variadic parameter
       ,-[ file:///test0.st:5:10 ]
       |
     3 | VAR_INPUT v : INT...; END_VAR
       |           |
       |           `-- the variadic parameter here is 'v'
       |
     5 |     F := ...x MIN;
       |          ^^^^|^^^
       |              `----- 'x' is not a variadic parameter
       |
       | Note: `...v+` adds every argument the call passed, `...v[0]` reads the first
    ---'
    [E0814] Error: not a variadic parameter
       ,-[ file:///test0.st:6:10 ]
       |
     3 | VAR_INPUT v : INT...; END_VAR
       |           |
       |           `-- the variadic parameter here is 'v'
       |
     6 |     F := ...nothing MAX;
       |          ^^^^^^^|^^^^^^
       |                 `-------- 'nothing' is not a variadic parameter
       |
       | Note: `...v+` adds every argument the call passed, `...v[0]` reads the first
    ---'
    [E0814] Error: not a variadic parameter
       ,-[ file:///test0.st:7:10 ]
       |
     3 | VAR_INPUT v : INT...; END_VAR
       |           |
       |           `-- the variadic parameter here is 'v'
       |
     7 |     F := ...vMIN;
       |          ^^^|^^^
       |             `----- 'vMIN' is not a variadic parameter
       |
       | Note: `...v+` adds every argument the call passed, `...v[0]` reads the first
    ---'
    [E0002] Error: missing element
       ,-[ file:///test0.st:7:17 ]
       |
     7 |     F := ...vMIN;
       |                 |
       |                 `- missing '='
       |
       | Help: insert missing '='
    ---'
    ");
}

/// What each fold refuses, a type at a time. What each one takes runs in
/// `tests::codegen::variadics`, a module per operator.
#[rstest]
#[case::add("+", &["BOOL", "DATE", "LDATE", "TOD", "LTOD", "DT", "LDT", "CHAR"])]
#[case::sub("-", &["BOOL", "DATE", "LDATE", "TOD", "LTOD", "DT", "LDT", "CHAR"])]
#[case::mul("*", &["BOOL", "TIME", "LTIME", "DATE", "LDATE", "TOD", "LTOD", "DT", "LDT", "CHAR"])]
#[case::div("/", &["BOOL", "TIME", "LTIME", "DATE", "LDATE", "TOD", "LTOD", "DT", "LDT", "CHAR"])]
#[case::modulo(
    "%",
    &[
        "BOOL", "BYTE", "WORD", "DWORD", "LWORD", "REAL", "LREAL", "TIME", "LTIME",
        "DATE", "LDATE", "TOD", "LTOD", "DT", "LDT", "CHAR",
    ]
)]
#[case::power(
    "**",
    &[
        "BOOL", "SINT", "INT", "DINT", "LINT", "USINT", "UINT", "UDINT", "ULINT", "BYTE", "WORD",
        "DWORD", "LWORD", "TIME", "LTIME", "DATE", "LDATE", "TOD", "LTOD", "DT", "LDT", "CHAR",
    ]
)]
#[case::and("&", &["REAL", "LREAL", "TIME", "LTIME", "DATE", "LDATE", "TOD", "LTOD", "DT", "LDT", "CHAR"])]
#[case::or("|", &["REAL", "LREAL", "TIME", "LTIME", "DATE", "LDATE", "TOD", "LTOD", "DT", "LDT", "CHAR"])]
#[case::xor("^", &["REAL", "LREAL", "TIME", "LTIME", "DATE", "LDATE", "TOD", "LTOD", "DT", "LDT", "CHAR"])]
fn invalid_fold_on_a_type_its_operator_refuses(#[case] op: &str, #[case] refused: &[&str]) {
    for ty in refused {
        let mut db = RootDatabase::default();
        let source = format!(
            "FUNCTION F : {ty}\nVAR_INPUT v : {ty}...; END_VAR\n    F := ...v{op};\nEND_FUNCTION\n"
        );
        let report = test_diagnostics(&mut db, &[&source]);
        assert!(report.contains("[E0305]"), "...v{op} on {ty}: {report}");
    }
}
