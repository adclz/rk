// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

/// The counter read after the loop, left by an EXIT or by the end of the
/// loop: what it holds there is the implementation's.
#[rstest]
fn a_counter_read_after_its_loop(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION FindKey : INT
VAR_INPUT
    words : ARRAY[0..9] OF STRING;
END_VAR
VAR
    i, j, total : INT;
END_VAR
    FOR i := 0 TO 9 DO
        IF words[i] = 'Key' THEN
            EXIT;
        END_IF;
    END_FOR;
    IF i <= 9 THEN
        words[i] := 'Found';
    END_IF;
    FOR j := 1 TO 3 DO
        total := total + j;
    END_FOR;
    FindKey := j;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "loop-var-after-loop"), @r"
    [L0130] Warning: FOR control variable read after the loop
        ,-[ file:///test0.st:14:8 ]
        |
      9 |     FOR i := 0 TO 9 DO
        |         |
        |         `-- 'i' controls this FOR loop
        |
     14 |     IF i <= 9 THEN
        |        |
        |        `-- 'i' is read after the FOR loop it controls
        |
        | Help: inside the loop, copy 'i' into a variable you read afterwards
        |
        | Note 1: after the loop, 'i' holds a value that depends on how the loop ended
        |
        | Note 2: lint rule: loop-var-after-loop
    ----'
    [L0130] Warning: FOR control variable read after the loop
        ,-[ file:///test0.st:20:16 ]
        |
     17 |     FOR j := 1 TO 3 DO
        |         |
        |         `-- 'j' controls this FOR loop
        |
     20 |     FindKey := j;
        |                |
        |                `-- 'j' is read after the FOR loop it controls
        |
        | Help: inside the loop, copy 'j' into a variable you read afterwards
        |
        | Note 1: after the loop, 'j' holds a value that depends on how the loop ended
        |
        | Note 2: lint rule: loop-var-after-loop
    ----'
    ");
}

/// A counter written again holds a value of its own: an assignment, a
/// second loop, an output bound to it. Reads inside the loop are the loop's.
#[rstest]
fn a_counter_written_again_is_not_reported(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Source
VAR_OUTPUT q : INT; END_VAR
    q := 7;
END_FUNCTION_BLOCK

FUNCTION_BLOCK Use
VAR
    i, x : INT;
    src : Source;
    a : ARRAY[0..9] OF INT;
END_VAR
    FOR i := 0 TO 9 DO
        a[i] := i;
    END_FOR;
    i := 0;
    x := i;
    FOR i := 0 TO 3 DO
        x := x + i;
    END_FOR;
    FOR i := 5 TO 9 DO
        x := x + a[i];
    END_FOR;
    src(q => i);
    x := i;
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "loop-var-after-loop"), @r"");
}

/// One path is enough: the IF writes the counter on one branch only. When
/// both branches write it, no path reaches the read.
#[rstest]
fn a_read_one_path_reaches_is_reported(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION Paths : INT
VAR_INPUT c : BOOL; END_VAR
VAR i, x : INT; END_VAR
    FOR i := 0 TO 9 DO
        x := x + 1;
    END_FOR;
    IF c THEN
        i := 0;
    END_IF;
    x := i;
    IF c THEN
        i := 1;
    ELSE
        i := 2;
    END_IF;
    Paths := i;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "loop-var-after-loop"), @r"
    [L0130] Warning: FOR control variable read after the loop
        ,-[ file:///test0.st:11:10 ]
        |
      5 |     FOR i := 0 TO 9 DO
        |         |
        |         `-- 'i' controls this FOR loop
        |
     11 |     x := i;
        |          |
        |          `-- 'i' is read after the FOR loop it controls
        |
        | Help: inside the loop, copy 'i' into a variable you read afterwards
        |
        | Note 1: after the loop, 'i' holds a value that depends on how the loop ended
        |
        | Note 2: lint rule: loop-var-after-loop
    ----'
    ");
}

/// An outer loop runs its body again: a read at its top sees what an inner
/// loop further down left on the pass before.
#[rstest]
fn an_outer_loop_sees_what_an_inner_loop_left(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION Nested : INT
VAR i, j, k, x : INT; END_VAR
    FOR i := 1 TO 3 DO
        x := x + j;
        FOR j := 1 TO 2 DO
            x := x + 1;
        END_FOR;
    END_FOR;
    WHILE x < 100 DO
        x := x + k;
        FOR k := 1 TO 2 DO
            x := x + 1;
        END_FOR;
    END_WHILE;
    Nested := x;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "loop-var-after-loop"), @r"
    [L0130] Warning: FOR control variable read after the loop
       ,-[ file:///test0.st:5:18 ]
       |
     5 |         x := x + j;
       |                  |
       |                  `-- 'j' is read after the FOR loop it controls
     6 |         FOR j := 1 TO 2 DO
       |             |
       |             `-- 'j' controls this FOR loop
       |
       | Help: inside the loop, copy 'j' into a variable you read afterwards
       |
       | Note 1: after the loop, 'j' holds a value that depends on how the loop ended
       |
       | Note 2: lint rule: loop-var-after-loop
    ---'
    [L0130] Warning: FOR control variable read after the loop
        ,-[ file:///test0.st:11:18 ]
        |
     11 |         x := x + k;
        |                  |
        |                  `-- 'k' is read after the FOR loop it controls
     12 |         FOR k := 1 TO 2 DO
        |             |
        |             `-- 'k' controls this FOR loop
        |
        | Help: inside the loop, copy 'k' into a variable you read afterwards
        |
        | Note 1: after the loop, 'k' holds a value that depends on how the loop ended
        |
        | Note 2: lint rule: loop-var-after-loop
    ----'
    ");
}

/// A subscript, an argument, `REF()`, a CASE selector and an UNTIL read
/// the counter too. The first read on a path is the one reported.
#[rstest]
fn every_read_counts(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION Twice : INT
VAR_INPUT n : INT; END_VAR
    Twice := n * 2;
END_FUNCTION

FUNCTION Reads : INT
VAR
    i, x : INT;
    a : ARRAY[0..10] OF INT;
    p : REF_TO INT;
END_VAR
    FOR i := 0 TO 9 DO x := x + 1; END_FOR;
    a[i] := 1;
    x := i;
    FOR i := 0 TO 9 DO x := x + 1; END_FOR;
    x := Twice(n := i);
    FOR i := 0 TO 9 DO x := x + 1; END_FOR;
    p := REF(i);
    FOR i := 0 TO 9 DO x := x + 1; END_FOR;
    CASE i OF
        10: x := 1;
    END_CASE;
    FOR i := 0 TO 9 DO x := x + 1; END_FOR;
    REPEAT
        x := x + 1;
    UNTIL x > i
    END_REPEAT;
    Reads := x;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "loop-var-after-loop"), @r"
    [L0130] Warning: FOR control variable read after the loop
        ,-[ file:///test0.st:14:7 ]
        |
     13 |     FOR i := 0 TO 9 DO x := x + 1; END_FOR;
        |         |
        |         `-- 'i' controls this FOR loop
     14 |     a[i] := 1;
        |       |
        |       `-- 'i' is read after the FOR loop it controls
        |
        | Help: inside the loop, copy 'i' into a variable you read afterwards
        |
        | Note 1: after the loop, 'i' holds a value that depends on how the loop ended
        |
        | Note 2: lint rule: loop-var-after-loop
    ----'
    [L0130] Warning: FOR control variable read after the loop
        ,-[ file:///test0.st:17:21 ]
        |
     16 |     FOR i := 0 TO 9 DO x := x + 1; END_FOR;
        |         |
        |         `-- 'i' controls this FOR loop
     17 |     x := Twice(n := i);
        |                     |
        |                     `-- 'i' is read after the FOR loop it controls
        |
        | Help: inside the loop, copy 'i' into a variable you read afterwards
        |
        | Note 1: after the loop, 'i' holds a value that depends on how the loop ended
        |
        | Note 2: lint rule: loop-var-after-loop
    ----'
    [L0130] Warning: FOR control variable read after the loop
        ,-[ file:///test0.st:19:14 ]
        |
     18 |     FOR i := 0 TO 9 DO x := x + 1; END_FOR;
        |         |
        |         `-- 'i' controls this FOR loop
     19 |     p := REF(i);
        |              |
        |              `-- 'i' is read after the FOR loop it controls
        |
        | Help: inside the loop, copy 'i' into a variable you read afterwards
        |
        | Note 1: after the loop, 'i' holds a value that depends on how the loop ended
        |
        | Note 2: lint rule: loop-var-after-loop
    ----'
    [L0130] Warning: FOR control variable read after the loop
        ,-[ file:///test0.st:21:10 ]
        |
     20 |     FOR i := 0 TO 9 DO x := x + 1; END_FOR;
        |         |
        |         `-- 'i' controls this FOR loop
     21 |     CASE i OF
        |          |
        |          `-- 'i' is read after the FOR loop it controls
        |
        | Help: inside the loop, copy 'i' into a variable you read afterwards
        |
        | Note 1: after the loop, 'i' holds a value that depends on how the loop ended
        |
        | Note 2: lint rule: loop-var-after-loop
    ----'
    [L0130] Warning: FOR control variable read after the loop
        ,-[ file:///test0.st:27:15 ]
        |
     24 |     FOR i := 0 TO 9 DO x := x + 1; END_FOR;
        |         |
        |         `-- 'i' controls this FOR loop
        |
     27 |     UNTIL x > i
        |               |
        |               `-- 'i' is read after the FOR loop it controls
        |
        | Help: inside the loop, copy 'i' into a variable you read afterwards
        |
        | Note 1: after the loop, 'i' holds a value that depends on how the loop ended
        |
        | Note 2: lint rule: loop-var-after-loop
    ----'
    ");
}

/// Twenty-four nested loops, each reading at its top the counter of the
/// loop inside it: every read is reported, and the walk stays quick. A
/// loop's second pass starts with what the first one left, so the loops
/// inside it take one pass each.
#[rstest]
fn deep_nesting_is_followed_quickly(mut with_db: RootDatabase) {
    let depth = 24;
    let counters: Vec<String> = (0..depth).map(|n| format!("i{n}")).collect();
    let mut body = String::new();
    for (n, counter) in counters.iter().enumerate() {
        let inner = counters.get(n + 1).unwrap_or(counter);
        body.push_str(&format!("FOR {counter} := 1 TO 2 DO\nx := x + {inner};\n"));
    }
    body.push_str(&"END_FOR;\n".repeat(depth));
    let source = format!(
        "FUNCTION Deep : INT\nVAR x, {} : INT; END_VAR\n{body}Deep := x;\nEND_FUNCTION\n",
        counters.join(", ")
    );
    let started = std::time::Instant::now();
    let report = test_single_lint(&mut with_db, &[source.as_str()], "loop-var-after-loop");
    assert_eq!(
        report
            .matches("is read after the FOR loop it controls")
            .count(),
        depth - 1
    );
    assert!(started.elapsed().as_secs() < 30, "{:?}", started.elapsed());
}
