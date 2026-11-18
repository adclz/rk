// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! Async functions: an `{extern async}` FUNCTION_BLOCK, whose calls take the
//! call a step at a time; `DONE`, then what it returns as outputs, a scalar
//! as `value`. Past 4 values the call passes its parameters in memory, as
//! a tuple: scalars and strings as they are, a record in words its fields
//! are packed into, since the tuple holds the record whole.

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use super::checked;
use crate::tests::utils::with_db;

/// Every shape an async call gives back.
#[rstest]
fn what_an_async_call_gives_back(mut with_db: RootDatabase) {
    assert_snapshot!(checked(&mut with_db, r#"
        package acme:example@0.3.0;

        interface timers {
            enum error-code { late }
            record stamp { seconds: s64, nanoseconds: u32 }
            wait-for: async func(how-long: u64);
            measure: async func(samples: u32) -> f32;
            when: async func() -> stamp;
            pair: async func() -> tuple<s32, s32>;
            maybe: async func() -> option<u64>;
            try-it: async func() -> result<_, error-code>;
        }

        world example {
            import timers;
        }
    "#), @r"
    // acme-example-timers.st
    NAMESPACE INTERNAL Acme
    	NAMESPACE Example
    		NAMESPACE Timers
    			TYPE ERROR_CODE : USINT (
    				LATE := 0
    			); END_TYPE

    			TYPE STAMP : STRUCT
    				seconds : LINT;
    				nanoseconds : UDINT;
    			END_STRUCT END_TYPE

    			{extern async 'acme:example/timers@0.3.0' 'wait-for'}
    			FUNCTION_BLOCK WAIT_FOR
    			VAR_INPUT
    				how_long : ULINT;
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    			END_VAR
    			END_FUNCTION_BLOCK

    			{extern async 'acme:example/timers@0.3.0' 'measure'}
    			FUNCTION_BLOCK MEASURE
    			VAR_INPUT
    				samples : UDINT;
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    				value : REAL;
    			END_VAR
    			END_FUNCTION_BLOCK

    			/* `when` as the host gives it: the values of its result, which `WHEN` puts
    			   in the members of `value`. */
    			{extern async 'acme:example/timers@0.3.0' 'when'}
    			FUNCTION_BLOCK WHEN_RAW
    			VAR_OUTPUT
    				DONE : BOOL;
    				seconds : LINT;
    				nanoseconds : UDINT;
    			END_VAR
    			END_FUNCTION_BLOCK

    			FUNCTION_BLOCK WHEN
    			VAR_OUTPUT
    				DONE : BOOL;
    				value : STAMP;
    			END_VAR
    			VAR
    				call : WHEN_RAW;
    			END_VAR
    				call();
    				DONE := call.DONE;
    				IF call.DONE THEN
    					value.seconds := call.seconds;
    					value.nanoseconds := call.nanoseconds;
    				END_IF;
    			END_FUNCTION_BLOCK

    			{extern async 'acme:example/timers@0.3.0' 'pair'}
    			FUNCTION_BLOCK PAIR
    			VAR_OUTPUT
    				DONE : BOOL;
    				item0 : DINT;
    				item1 : DINT;
    			END_VAR
    			END_FUNCTION_BLOCK

    			{extern async 'acme:example/timers@0.3.0' 'maybe'}
    			FUNCTION_BLOCK MAYBE
    			VAR_OUTPUT
    				DONE : BOOL;
    				is_some : BOOL;
    				value : ULINT;
    			END_VAR
    			END_FUNCTION_BLOCK

    			{extern async 'acme:example/timers@0.3.0' 'try-it'}
    			FUNCTION_BLOCK TRY_IT
    			VAR_OUTPUT
    				DONE : BOOL;
    				failed : BOOL;
    				error : ERROR_CODE;
    			END_VAR
    			END_FUNCTION_BLOCK
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    ");
}

/// Parameters up to 4 values go as values, past them as a tuple: 5 scalars
/// or strings go as they are, a record among 5 values is packed into the
/// words of the tuple it is laid out in, a record of 4 values goes field by
/// field.
#[rstest]
fn an_async_call_s_parameters(mut with_db: RootDatabase) {
    assert_snapshot!(checked(&mut with_db, r#"
        package acme:example@0.3.0;

        interface motion {
            record point { x: f64, y: f64 }
            record box { a: u8, b: u64, c: u8, d: u64 }
            four: async func(a: u32, b: u64, c: f32, d: bool);
            five: async func(a: u32, b: u64, c: f32, d: bool, e: s8) -> u32;
            texts: async func(a: string, b: string, c: u8);
            move-to: async func(target: point);
            fit: async func(b: box);
            aim: async func(target: point, speed: f64, turn: f64, lift: f64);
        }

        world example {
            import motion;
        }
    "#), @r"
    // acme-example-motion.st
    NAMESPACE INTERNAL Acme
    	NAMESPACE Example
    		NAMESPACE Motion
    			USING Std.Convert;

    			TYPE POINT : STRUCT
    				x : LREAL;
    				y : LREAL;
    			END_STRUCT END_TYPE

    			TYPE BOX : STRUCT
    				a : USINT;
    				b : ULINT;
    				c : USINT;
    				d_ : ULINT;
    			END_STRUCT END_TYPE

    			{extern async 'acme:example/motion@0.3.0' 'four'}
    			FUNCTION_BLOCK FOUR
    			VAR_INPUT
    				a : UDINT;
    				b : ULINT;
    				c : REAL;
    				d_ : BOOL;
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    			END_VAR
    			END_FUNCTION_BLOCK

    			{extern async 'acme:example/motion@0.3.0' 'five'}
    			FUNCTION_BLOCK FIVE
    			VAR_INPUT
    				a : UDINT;
    				b : ULINT;
    				c : REAL;
    				d_ : BOOL;
    				e : SINT;
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    				value : UDINT;
    			END_VAR
    			END_FUNCTION_BLOCK

    			{extern async 'acme:example/motion@0.3.0' 'texts'}
    			FUNCTION_BLOCK TEXTS
    			VAR_INPUT
    				a : STRING[4096];
    				b : STRING[4096];
    				c : USINT;
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    			END_VAR
    			END_FUNCTION_BLOCK

    			/* `move-to` as the host takes it: its parameters as the canonical ABI
    			   passes them, which `MOVE_TO` passes field by field. */
    			{extern async 'acme:example/motion@0.3.0' 'move-to'}
    			FUNCTION_BLOCK MOVE_TO_RAW
    			VAR_INPUT
    				target_x : LREAL;
    				target_y : LREAL;
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    			END_VAR
    			END_FUNCTION_BLOCK

    			FUNCTION_BLOCK MOVE_TO
    			VAR_INPUT
    				target : POINT;
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    			END_VAR
    			VAR
    				call : MOVE_TO_RAW;
    			END_VAR
    				call(target_x := target.x, target_y := target.y);
    				DONE := call.DONE;
    			END_FUNCTION_BLOCK

    			/* `fit` as the host takes it: its parameters as the canonical ABI passes
    			   them, which `FIT` passes field by field. */
    			{extern async 'acme:example/motion@0.3.0' 'fit'}
    			FUNCTION_BLOCK FIT_RAW
    			VAR_INPUT
    				b_a : USINT;
    				b_b : ULINT;
    				b_c : USINT;
    				b_d : ULINT;
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    			END_VAR
    			END_FUNCTION_BLOCK

    			FUNCTION_BLOCK FIT
    			VAR_INPUT
    				b : BOX;
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    			END_VAR
    			VAR
    				call : FIT_RAW;
    			END_VAR
    				call(b_a := b.a, b_b := b.b, b_c := b.c, b_d := b.d_);
    				DONE := call.DONE;
    			END_FUNCTION_BLOCK

    			/* `aim` as the host takes it: its parameters as the canonical ABI passes
    			   them, which `AIM` packs. */
    			{extern async 'acme:example/motion@0.3.0' 'aim'}
    			FUNCTION_BLOCK AIM_RAW
    			VAR_INPUT
    				a0 : LWORD;
    				a1 : LWORD;
    				speed : LREAL;
    				turn : LREAL;
    				lift : LREAL;
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    			END_VAR
    			END_FUNCTION_BLOCK

    			FUNCTION_BLOCK AIM
    			VAR_INPUT
    				target : POINT;
    				speed : LREAL;
    				turn : LREAL;
    				lift : LREAL;
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    			END_VAR
    			VAR
    				a0 : LWORD;
    				a1 : LWORD;
    				call : AIM_RAW;
    			END_VAR
    				a0 := 0;
    				a1 := 0;
    				a0 := LREAL_TO_LWORD(target.x);
    				a1 := LREAL_TO_LWORD(target.y);
    				call(a0 := a0, a1 := a1, speed := speed, turn := turn, lift := lift);
    				DONE := call.DONE;
    			END_FUNCTION_BLOCK
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    ");
}
