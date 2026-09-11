use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

// {allow 'rule-name' ...} silences the named rules: as a statement, on the
// NEXT statement (nested bodies included); above a POU, on the whole POU.
// Unknown names are L0005 and silence nothing.

#[rstest]
fn a_statement_allow_covers_only_the_next_statement(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK adder
    VAR_INPUT
        a : INT;
        b : INT;
    END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK caller
    VAR
        p1 : adder;
        p2 : adder;
    END_VAR
    {allow 'missing-input-param'}
    p1(a := 1);
    p2(b := 2);
END_FUNCTION_BLOCK
"#;
    // Only the UNGUARDED call reports.
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "missing-input-param"), @r"
    [L0303] Hint: missing input parameter
        ,-[ file:///test0.st:16:5 ]
        |
      4 |         a : INT;
        |         ^^^|^^^
        |            `----- 'a' declared here
        |
     16 |     p2(b := 2);
        |     ^^^^^|^^^^
        |          `------ call to 'adder' is missing 1 input parameter: 'a'
        |
        | Note: lint rule: missing-input-param
    ----'
    ");
}

#[rstest]
fn a_pou_allow_covers_the_whole_pou(mut with_db: RootDatabase) {
    let source = r#"
{allow 'input-assignment'}
FUNCTION_BLOCK rebinder
    VAR_INPUT
        x : INT;
    END_VAR
    x := x + 1;
END_FUNCTION_BLOCK

FUNCTION_BLOCK plain
    VAR_INPUT
        y : INT;
    END_VAR
    y := 2;
END_FUNCTION_BLOCK
"#;
    // Only the POU WITHOUT the pragma reports.
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "input-assignment"), @r"
    [L0113] Warning: assignment to input variable
        ,-[ file:///test0.st:14:5 ]
        |
     12 |         y : INT;
        |         |
        |         `-- 'y' is declared here
        |
     14 |     y := 2;
        |     |
        |     `-- assignment to VAR_INPUT 'y'
        |
        | Note: lint rule: input-assignment
    ----'
    ");
}

#[rstest]
fn an_allow_reaches_into_nested_bodies(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK deep
    VAR_INPUT
        x : INT;
    END_VAR
    IF x > 0 THEN
        {allow 'input-assignment'}
        IF x > 1 THEN
            x := 0;
        END_IF;
    END_IF;
END_FUNCTION_BLOCK
"#;
    // The region is the next statement's WHOLE span, nested writes included.
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "input-assignment"), @r"");
}

#[rstest]
fn an_allow_region_ends_with_its_statement(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK deep
    VAR_INPUT
        x : INT;
    END_VAR
    IF x > 0 THEN
        {allow 'input-assignment'}
        IF x > 1 THEN
            x := 0;
        END_IF;
        x := 9;
    END_IF;
END_FUNCTION_BLOCK
"#;
    // The counterpart to the coverage tests: the region has depth AND an end.
    // The write AFTER the allowed statement, inside the same block, must still
    // report — an off-by-one in the region's end span passes every other test.
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "input-assignment"), @r"
    [L0113] Warning: assignment to input variable
        ,-[ file:///test0.st:11:9 ]
        |
      4 |         x : INT;
        |         |
        |         `-- 'x' is declared here
        |
     11 |         x := 9;
        |         |
        |         `-- assignment to VAR_INPUT 'x'
        |
        | Note: lint rule: input-assignment
    ----'
    ");
}

#[rstest]
fn an_allow_of_another_rule_silences_nothing(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK other
    VAR_INPUT
        x : INT;
    END_VAR
    {allow 'yoda-condition'}
    x := 1;
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "input-assignment"), @r"
    [L0113] Warning: assignment to input variable
       ,-[ file:///test0.st:7:5 ]
       |
     4 |         x : INT;
       |         |
       |         `-- 'x' is declared here
       |
     7 |     x := 1;
       |     |
       |     `-- assignment to VAR_INPUT 'x'
       |
       | Note: lint rule: input-assignment
    ---'
    ");
}

// One {allow} with several names: the same source, each rule checked in its
// own case (a db holds one copy of a file).
const MULTI_NAME: &str = r#"
FUNCTION_BLOCK multi
    VAR_INPUT
        x : INT;
    END_VAR
    {allow 'self-assignment' 'input-assignment'}
    x := x;
END_FUNCTION_BLOCK
"#;

#[rstest]
fn one_allow_takes_several_names_first(mut with_db: RootDatabase) {
    assert_snapshot!(test_single_lint(&mut with_db, &[MULTI_NAME], "self-assignment"), @r"");
}

#[rstest]
fn one_allow_takes_several_names_second(mut with_db: RootDatabase) {
    assert_snapshot!(test_single_lint(&mut with_db, &[MULTI_NAME], "input-assignment"), @r"");
}

#[rstest]
fn an_unknown_name_is_reported(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION f : INT
    {allow 'not-a-rule'}
    f := 1;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "unknown-allow"), @r"
    [L0005] Warning: unknown-allow
       ,-[ file:///test0.st:3:12 ]
       |
     3 |     {allow 'not-a-rule'}
       |            ^^^^^^|^^^^^
       |                  `------- no lint rule is named 'not-a-rule'
       |
       | Note: lint rule: unknown-allow
    ---'
    ");
}

#[rstest]
fn an_unknown_name_silences_nothing(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK typo
    VAR_INPUT
        x : INT;
    END_VAR
    {allow 'input-asignment'}
    x := 1;
END_FUNCTION_BLOCK
"#;
    // The misspelled name must not silence the rule it resembles.
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "input-assignment"), @r"
    [L0113] Warning: assignment to input variable
       ,-[ file:///test0.st:7:5 ]
       |
     4 |         x : INT;
       |         |
       |         `-- 'x' is declared here
       |
     7 |     x := 1;
       |     |
       |     `-- assignment to VAR_INPUT 'x'
       |
       | Note: lint rule: input-assignment
    ---'
    ");
}

#[rstest]
fn a_trailing_allow_covers_nothing_and_breaks_nothing(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK tail
    VAR_INPUT
        x : INT;
    END_VAR
    x := 1;
    {allow 'input-assignment'}
END_FUNCTION_BLOCK
"#;
    // The pragma has no next statement: the write BEFORE it still reports.
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "input-assignment"), @r"
    [L0113] Warning: assignment to input variable
       ,-[ file:///test0.st:6:5 ]
       |
     4 |         x : INT;
       |         |
       |         `-- 'x' is declared here
       |
     6 |     x := 1;
       |     |
       |     `-- assignment to VAR_INPUT 'x'
       |
       | Note: lint rule: input-assignment
    ---'
    ");
}

#[rstest]
fn an_allow_above_a_method_covers_the_method(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK holder
    VAR_INPUT
        x : INT;
    END_VAR
    {allow 'input-assignment'}
    METHOD reset : INT
        x := 0;
        reset := 1;
    END_METHOD
END_FUNCTION_BLOCK
"#;
    // The pragma parses as the METHOD's pou_pragma (not as a body statement),
    // and the region is the whole method.
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "input-assignment"), @r"");
}

#[rstest]
fn a_pou_allow_works_on_a_function(mut with_db: RootDatabase) {
    let source = r#"
{allow 'unused-variable'}
FUNCTION f : INT
    VAR
        scratch : INT;
    END_VAR
    f := 1;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "unused-variable"), @r"");
}

#[rstest]
fn a_pou_allow_works_on_a_program(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK adder
    VAR_INPUT
        a : INT;
    END_VAR
END_FUNCTION_BLOCK

{allow 'missing-input-param'}
PROGRAM main
    VAR
        inst : adder;
    END_VAR
    inst();
END_PROGRAM
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "missing-input-param"), @r"");
}

#[rstest]
fn a_statement_allow_works_inside_a_method_body(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK holder
    VAR_INPUT
        x : INT;
    END_VAR
    METHOD poke : INT
        {allow 'input-assignment'}
        x := 0;
        poke := 1;
    END_METHOD
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "input-assignment"), @r"");
}

#[rstest]
fn a_statement_allow_works_inside_case_and_loop_bodies(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK branches
    VAR_INPUT
        x : INT;
    END_VAR
    CASE x OF
        1:
            {allow 'input-assignment'}
            x := 0;
    ELSE
        WHILE x > 0 DO
            {allow 'input-assignment'}
            x := x - 1;
        END_WHILE;
    END_CASE;
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "input-assignment"), @r"");
}

// The next two tests share one source with a POU-level {allow 'A'} AND a
// statement-level {allow 'B'} inside it. Together they pin TWO facts: that
// namespaced POUs get regions at all (they ride collect_namespace_scopes,
// where a regression silences nothing), and that STACKED allows do not
// shadow each other — the inner pragma adds to the outer, both apply.
#[rstest]
fn a_namespaced_stack_silences_the_pou_rule(mut with_db: RootDatabase) {
    let source = r#"
NAMESPACE Plant.Cell
    {allow 'input-assignment'}
    FUNCTION_BLOCK rebinder
        VAR_INPUT
            x : INT;
        END_VAR
        {allow 'self-assignment'}
        x := x;
    END_FUNCTION_BLOCK
END_NAMESPACE
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "input-assignment"), @r"");
}

#[rstest]
fn a_namespaced_stack_silences_the_statement_rule(mut with_db: RootDatabase) {
    let source = r#"
NAMESPACE Plant.Cell
    {allow 'input-assignment'}
    FUNCTION_BLOCK rebinder
        VAR_INPUT
            x : INT;
        END_VAR
        {allow 'self-assignment'}
        x := x;
    END_FUNCTION_BLOCK
END_NAMESPACE
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "self-assignment"), @r"");
}
