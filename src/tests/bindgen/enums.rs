// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! Enums: a typed enum whose base holds its cases as the canonical ABI
//! does, a byte up to 256 cases and 16 bits past them, its cases numbered
//! in WIT's order; one goes in and comes back like any scalar, in a record
//! or an option too.

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use super::{checked, generated, reported};
use crate::tests::utils::with_db;

/// An enum taken, given, and held in what a function returns.
#[rstest]
fn an_enum_goes_in_and_comes_back(mut with_db: RootDatabase) {
    assert_snapshot!(checked(&mut with_db, r#"
        package wasi:example@0.3.0;

        interface lights {
            /// What a light shows.
            enum color { red, amber, green }
            record lamp { shown: color, lit: bool }
            set: func(to: color) -> color;
            look: func() -> lamp;
            guess: func() -> option<color>;
            check: func() -> result<_, color>;
            place: func(at: lamp);
        }

        world example {
            import lights;
        }
    "#), @r"
    // wasi-example-lights.st
    NAMESPACE INTERNAL Wasi
    	NAMESPACE Example
    		NAMESPACE Lights
    			/* What a light shows. */
    			TYPE COLOR : USINT (
    				RED := 0,
    				AMBER := 1,
    				GREEN := 2
    			); END_TYPE

    			{extern 'wasi:example/lights@0.3.0' 'set'}
    			FUNCTION SET : COLOR
    			VAR_INPUT
    				to_ : COLOR;
    			END_VAR
    			END_FUNCTION

    			{extern 'wasi:example/lights@0.3.0' 'look'}
    			FUNCTION LOOK
    			VAR_OUTPUT
    				shown : COLOR;
    				lit : BOOL;
    			END_VAR
    			END_FUNCTION

    			{extern 'wasi:example/lights@0.3.0' 'guess'}
    			FUNCTION GUESS
    			VAR_OUTPUT
    				is_some : BOOL;
    				value : COLOR;
    			END_VAR
    			END_FUNCTION

    			{extern 'wasi:example/lights@0.3.0' 'check'}
    			FUNCTION CHECK
    			VAR_OUTPUT
    				failed : BOOL;
    				error : COLOR;
    			END_VAR
    			END_FUNCTION

    			{extern 'wasi:example/lights@0.3.0' 'place'}
    			FUNCTION PLACE
    			VAR_INPUT
    				at_shown : COLOR;
    				at_lit : BOOL;
    			END_VAR
    			END_FUNCTION
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    ");
}

/// The cases of an enum, `count` of them: `c0`, `c1`, ...
fn wide(count: usize) -> String {
    let cases: Vec<String> = (0..count).map(|at| format!("c{at}")).collect();
    format!(
        "package wasi:example@0.3.0;
        interface wide {{
            enum many {{ {} }}
            pick: func(what: many) -> many;
        }}
        world example {{
            import wide;
        }}",
        cases.join(", ")
    )
}

/// The lines of the generated enum that show its base and its last case,
/// and the function's.
fn base_and_last(wit: &str, last: &str) -> String {
    let bindings = generated(wit);
    bindings.files[0]
        .text
        .lines()
        .map(str::trim)
        .filter(|line| {
            line.starts_with("TYPE") || line.starts_with("FUNCTION") || line.starts_with(last)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// 256 cases fit a byte, the 257th takes 16 bits: USINT, then UINT.
#[rstest]
fn the_base_widens_past_256_cases(mut with_db: RootDatabase) {
    assert_snapshot!(base_and_last(&wide(256), "C255"), @r"
    TYPE MANY : USINT (
    C255 := 255
    FUNCTION PICK : MANY
    ");
    assert_snapshot!(base_and_last(&wide(257), "C256"), @r"
    TYPE MANY : UINT (
    C256 := 256
    FUNCTION PICK : MANY
    ");
    let bindings = generated(&wide(257));
    assert_eq!(reported(&mut with_db, &bindings.files, &[]), "");
}
