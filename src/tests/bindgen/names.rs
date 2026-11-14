// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! How WIT names are written in ST: a namespace in words that start with a
//! capital, a POU, a type or an enum's value in capitals joined by `_`, a
//! variable in lowercase; a keyword of ST takes a `_` after it, wherever it
//! stands. Two WIT names written alike in one namespace stop the
//! generator; two variables of one POU leave the function out.

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use super::checked;
use crate::tests::utils::with_db;

/// Words joined by `-` in each place a name stands: a package, an interface,
/// a function, a parameter, a type, an enum's value.
#[rstest]
fn words_in_each_place(mut with_db: RootDatabase) {
    assert_snapshot!(checked(&mut with_db, r#"
        package wasi:many-words@0.3.0;

        interface monotonic-clock-reader {
            enum read-mode { as-is, rounded-down }
            type tick-count = u64;
            read-the-clock: func(read-mode: read-mode, round-to-nearest: u32) -> tick-count;
        }

        world example {
            import monotonic-clock-reader;
        }
    "#), @r"
    // wasi-many-words-monotonic-clock-reader.st
    NAMESPACE INTERNAL Wasi
    	NAMESPACE ManyWords
    		NAMESPACE MonotonicClockReader
    			TYPE READ_MODE : USINT (
    				AS_IS := 0,
    				ROUNDED_DOWN := 1
    			); END_TYPE

    			TYPE TICK_COUNT : ULINT; END_TYPE

    			{extern 'wasi:many-words/monotonic-clock-reader@0.3.0' 'read-the-clock'}
    			FUNCTION READ_THE_CLOCK : TICK_COUNT
    			VAR_INPUT
    				read_mode : READ_MODE;
    				round_to_nearest : UDINT;
    			END_VAR
    			END_FUNCTION
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    ");
}

/// A keyword of ST, in any place: the interface `exit`, the function
/// `exit`, a parameter `by`, an enum's values `true` and `do`, a type
/// `time`, a resource `task`, outputs `step` and `of`.
#[rstest]
fn a_keyword_takes_an_underscore_wherever_it_stands(mut with_db: RootDatabase) {
    assert_snapshot!(checked(&mut with_db, r#"
        package wasi:example@0.3.0;

        interface exit {
            enum answer { true, do, maybe }
            type time = u64;
            resource task {
                step: func() -> time;
            }
            record point { step: u32, of: u32 }
            exit: func(by: answer, at: time) -> point;
        }

        world example {
            import exit;
        }
    "#), @r"
    // wasi-example-exit.st
    NAMESPACE INTERNAL Wasi
    	NAMESPACE Example
    		NAMESPACE Exit_
    			TYPE ANSWER : USINT (
    				TRUE_ := 0,
    				DO_ := 1,
    				MAYBE := 2
    			); END_TYPE

    			TYPE TIME_ : ULINT; END_TYPE

    			/* Lets go of `handle`, a handle of `task` the host gave. */
    			{extern 'wasi:example/exit@0.3.0' '[resource-drop]task'}
    			FUNCTION TASK_DROP
    			VAR_INPUT
    				handle : UDINT;
    			END_VAR
    			END_FUNCTION

    			{extern 'wasi:example/exit@0.3.0' '[method]task.step'}
    			FUNCTION TASK_STEP : TIME_
    			VAR_INPUT
    				self : UDINT;
    			END_VAR
    			END_FUNCTION

    			{extern 'wasi:example/exit@0.3.0' 'exit'}
    			FUNCTION EXIT_
    			VAR_INPUT
    				by_ : ANSWER;
    				at_ : TIME_;
    			END_VAR
    			VAR_OUTPUT
    				step_ : UDINT;
    				of_ : UDINT;
    			END_VAR
    			END_FUNCTION
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    ");
}

/// Two WIT names that ST writes alike in one namespace: the function
/// `x-new` and the constructor of `x`. The generator stops, saying which.
#[test]
fn two_names_written_alike_stop_the_generator() {
    let (resolve, world) = rk_bindgen::resolve(
        r#"
        package wasi:example@0.3.0;

        interface clash {
            resource x {
                constructor();
            }
            x-new: func() -> u32;
        }

        world example {
            import clash;
        }
    "#,
    )
    .expect("the WIT resolves");
    assert_snapshot!(rk_bindgen::generate(&resolve, world).unwrap_err(), @"`[constructor]x` and `x-new` would both be `X_NEW` in `wasi:example/clash@0.3.0`");
}

/// Two variables of one POU that ST writes alike: a parameter `value` and
/// the option's `value` that comes back, a record parameter whose field
/// meets another parameter, an async block's `DONE` and a field `done`.
/// The function is left out, saying why.
#[rstest]
fn two_variables_written_alike_leave_the_function_out(mut with_db: RootDatabase) {
    assert_snapshot!(checked(&mut with_db, r#"
        package wasi:example@0.3.0;

        interface clash {
            record p { x: u32 }
            record finished { done: bool }
            lookup: func(value: u32) -> option<u32>;
            place: func(p: p, p-x: u32);
            run: async func() -> finished;
            fine: func(value: u32) -> u32;
        }

        world example {
            import clash;
        }
    "#), @r"
    // wasi-example-clash.st
    NAMESPACE INTERNAL Wasi
    	NAMESPACE Example
    		NAMESPACE Clash
    			{extern 'wasi:example/clash@0.3.0' 'fine'}
    			FUNCTION FINE : UDINT
    			VAR_INPUT
    				value : UDINT;
    			END_VAR
    			END_FUNCTION
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    /* Not generated:
       - `lookup`: two of its variables would be `value`, which ST takes for
         one.
       - `place`: two of its variables would be `p_x`, which ST takes for one.
       - `run`: two of its variables would be `done`, which ST takes for one. */
    ");
}

/// Every keyword of ST's grammar that a WIT name could become is one the
/// generated names keep away from.
#[test]
fn the_keywords_are_the_grammar_s() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/crates/tree-sitter/src/node-types.json"
    );
    let nodes: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).expect("the grammar's node types"))
            .expect("JSON");
    let mut words = std::collections::BTreeSet::new();
    fn walk(value: &serde_json::Value, words: &mut std::collections::BTreeSet<String>) {
        match value {
            serde_json::Value::Object(object) => {
                if let (
                    Some(serde_json::Value::String(word)),
                    Some(serde_json::Value::Bool(false)),
                ) = (object.get("type"), object.get("named"))
                {
                    let wit_like = word.starts_with(|c: char| c.is_ascii_alphabetic())
                        && word.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
                    if wit_like {
                        words.insert(word.clone());
                    }
                }
                object.values().for_each(|value| walk(value, words));
            }
            serde_json::Value::Array(values) => values.iter().for_each(|value| walk(value, words)),
            _ => {}
        }
    }
    walk(&nodes, &mut words);
    let missing: Vec<&String> = words
        .iter()
        .filter(|word| {
            !rk_bindgen::names::KEYWORDS
                .iter()
                .any(|keyword| keyword.eq_ignore_ascii_case(word))
        })
        .collect();
    assert!(
        missing.is_empty(),
        "keywords the generator does not keep away from: {missing:?}"
    );
}
