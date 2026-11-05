// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! The standard functions called as the standard's tables call them, by the names
//! they give the inputs. A name is part of a function's interface: the same call
//! with another name is E0803.

use rstest::rstest;

use super::{compile_with_libraries, execute_wasm, with_db};

/// Each call answers TRUE, in a program that reports the first which does not.
fn assert_each_holds(db: &mut db::RootDatabase, library: &str, using: &str, calls: &[&str]) {
    let mut source = format!("USING {using};\nFUNCTION test : DINT\n");
    for (i, call) in calls.iter().enumerate() {
        source += &format!(
            "    IF NOT ({call}) THEN test := {}; RETURN; END_IF;\n",
            i + 1
        );
    }
    source += "END_FUNCTION\n";
    let wasm = compile_with_libraries(db, &[library], &source);
    let failed: i32 = execute_wasm(&wasm, "test", ());
    if failed != 0 {
        panic!("{} is false", calls[failed as usize - 1]);
    }
}

#[rstest]
fn the_string_functions(mut with_db: db::RootDatabase) {
    assert_each_holds(
        &mut with_db,
        include_str!("../../../stdlib/Strings.st"),
        "Std.Strings",
        &[
            "LEN(IN := 'ASTRING') = 7",
            "LEFT(IN := 'ASTR', L := 3) = 'AST'",
            "RIGHT(IN := 'ASTR', L := 3) = 'STR'",
            "MID(IN := 'ASTR', L := 2, P := 2) = 'ST'",
            "CONCAT(IN1 := 'AB', IN2 := 'CD') = 'ABCD'",
            "INSERT(IN1 := 'ABC', IN2 := 'XY', P := 2) = 'ABXYC'",
            "DELETE(IN := 'ABXYC', L := 2, P := 3) = 'ABC'",
            "REPLACE(IN1 := 'ABCDE', IN2 := 'X', L := 2, P := 3) = 'ABXE'",
            "FIND(IN1 := 'ABCBC', IN2 := 'BC') = 2",
            "CHAR_COUNT(IN := 'ASTR') = 4",
            "CHAR_AT(IN := 'ASTR', P := 2) = CHAR#'S'",
            "CHAR_LEFT(IN := 'ASTR', L := 3) = 'AST'",
            "CHAR_RIGHT(IN := 'ASTR', L := 3) = 'STR'",
            "CHAR_MID(IN := 'ASTR', L := 2, P := 2) = 'ST'",
            "CHAR_INSERT(IN1 := 'ABC', IN2 := 'XY', P := 2) = 'ABXYC'",
            "CHAR_DELETE(IN := 'ABXYC', L := 2, P := 3) = 'ABC'",
            "CHAR_REPLACE(IN1 := 'ABCDE', IN2 := 'X', L := 2, P := 3) = 'ABXE'",
            "CHAR_FIND(IN1 := 'ABCBC', IN2 := 'BC') = 2",
        ],
    );
}

/// `CONCAT` is extensible: a STRING cannot be variadic, so it has one overload per
/// input count, up to eight.
#[rstest]
fn concat_takes_up_to_eight_inputs(mut with_db: db::RootDatabase) {
    assert_each_holds(
        &mut with_db,
        include_str!("../../../stdlib/Strings.st"),
        "Std.Strings",
        &[
            "CONCAT('AB', 'CD', 'E') = 'ABCDE'",
            "CONCAT('a', 'b', 'c', 'd') = 'abcd'",
            "CONCAT('a', 'b', 'c', 'd', 'e') = 'abcde'",
            "CONCAT('a', 'b', 'c', 'd', 'e', 'f') = 'abcdef'",
            "CONCAT('a', 'b', 'c', 'd', 'e', 'f', 'g') = 'abcdefg'",
            "CONCAT('a', 'b', 'c', 'd', 'e', 'f', 'g', 'h') = 'abcdefgh'",
            "CONCAT(IN1 := 'a', IN2 := 'b', IN3 := 'c') = 'abc'",
            "CONCAT(IN3 := 'c', IN1 := 'a', IN2 := 'b') = 'abc'",
            "CONCAT('', 'a', '') = 'a'",
        ],
    );
}

#[rstest]
fn the_numerical_functions(mut with_db: db::RootDatabase) {
    assert_each_holds(
        &mut with_db,
        include_str!("../../../stdlib/Math.st"),
        "Std.Math",
        &[
            "EXPT(IN1 := 2.0, IN2 := 3.0) = 8.0",
            "EXPT(IN2 := 3.0, IN1 := 2.0) = 8.0",
            "EXPT(IN1 := LREAL#2.0, IN2 := LREAL#3.0) = LREAL#8.0",
            // The angle of the point (0, 1), a quarter of a turn: the other way
            // round it is 0.
            "ATAN2(Y := 1.0, X := 0.0) > 1.5",
            "ATAN2(X := 0.0, Y := 1.0) > 1.5",
            "ATAN2(Y := 0.0, X := 1.0) = 0.0",
            "ATAN2(Y := LREAL#0.0, X := LREAL#1.0) = LREAL#0.0",
        ],
    );
}
