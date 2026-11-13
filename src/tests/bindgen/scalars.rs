// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! Numbers, BOOL, char and string: each WIT scalar is a variable of its ST
//! type, in and out; a char has none; a string goes in as a STRING and
//! does not come back yet.

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use super::checked;
use crate::tests::utils::with_db;

/// Each number and `bool` taken and returned, as one value each: `u8` is
/// USINT and `s64` LINT, `f32` REAL and `f64` LREAL.
#[rstest]
fn every_number_and_bool_goes_in_and_comes_back(mut with_db: RootDatabase) {
    assert_snapshot!(checked(&mut with_db, r#"
        package wasi:example@0.3.0;

        interface numbers {
            flag: func(x: bool) -> bool;
            byte: func(x: u8) -> u8;
            word: func(x: u16) -> u16;
            dword: func(x: u32) -> u32;
            lword: func(x: u64) -> u64;
            small: func(x: s8) -> s8;
            half: func(x: s16) -> s16;
            whole: func(x: s32) -> s32;
            long: func(x: s64) -> s64;
            single: func(x: f32) -> f32;
            double: func(x: f64) -> f64;
        }

        world example {
            import numbers;
        }
    "#), @r"
    // wasi-example-numbers.st
    NAMESPACE INTERNAL Wasi
    	NAMESPACE Example
    		NAMESPACE Numbers
    			{extern 'wasi:example/numbers@0.3.0' 'flag'}
    			FUNCTION FLAG : BOOL
    			VAR_INPUT
    				x : BOOL;
    			END_VAR
    			END_FUNCTION

    			{extern 'wasi:example/numbers@0.3.0' 'byte'}
    			FUNCTION BYTE_ : USINT
    			VAR_INPUT
    				x : USINT;
    			END_VAR
    			END_FUNCTION

    			{extern 'wasi:example/numbers@0.3.0' 'word'}
    			FUNCTION WORD_ : UINT
    			VAR_INPUT
    				x : UINT;
    			END_VAR
    			END_FUNCTION

    			{extern 'wasi:example/numbers@0.3.0' 'dword'}
    			FUNCTION DWORD_ : UDINT
    			VAR_INPUT
    				x : UDINT;
    			END_VAR
    			END_FUNCTION

    			{extern 'wasi:example/numbers@0.3.0' 'lword'}
    			FUNCTION LWORD_ : ULINT
    			VAR_INPUT
    				x : ULINT;
    			END_VAR
    			END_FUNCTION

    			{extern 'wasi:example/numbers@0.3.0' 'small'}
    			FUNCTION SMALL : SINT
    			VAR_INPUT
    				x : SINT;
    			END_VAR
    			END_FUNCTION

    			{extern 'wasi:example/numbers@0.3.0' 'half'}
    			FUNCTION HALF : INT
    			VAR_INPUT
    				x : INT;
    			END_VAR
    			END_FUNCTION

    			{extern 'wasi:example/numbers@0.3.0' 'whole'}
    			FUNCTION WHOLE : DINT
    			VAR_INPUT
    				x : DINT;
    			END_VAR
    			END_FUNCTION

    			{extern 'wasi:example/numbers@0.3.0' 'long'}
    			FUNCTION LONG : LINT
    			VAR_INPUT
    				x : LINT;
    			END_VAR
    			END_FUNCTION

    			{extern 'wasi:example/numbers@0.3.0' 'single'}
    			FUNCTION SINGLE_ : REAL
    			VAR_INPUT
    				x : REAL;
    			END_VAR
    			END_FUNCTION

    			{extern 'wasi:example/numbers@0.3.0' 'double'}
    			FUNCTION DOUBLE : LREAL
    			VAR_INPUT
    				x : LREAL;
    			END_VAR
    			END_FUNCTION
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    ");
}

/// A function that takes nothing and returns nothing is a FUNCTION with
/// neither, and several parameters keep their order.
#[rstest]
fn parameters_keep_their_order(mut with_db: RootDatabase) {
    assert_snapshot!(checked(&mut with_db, r#"
        package wasi:example@0.3.0;

        interface order {
            reset: func();
            mix: func(first: u8, second: f64, third: bool, fourth: s16) -> u32;
        }

        world example {
            import order;
        }
    "#), @r"
    // wasi-example-order.st
    NAMESPACE INTERNAL Wasi
    	NAMESPACE Example
    		NAMESPACE Order
    			{extern 'wasi:example/order@0.3.0' 'reset'}
    			FUNCTION RESET
    			END_FUNCTION

    			{extern 'wasi:example/order@0.3.0' 'mix'}
    			FUNCTION MIX : UDINT
    			VAR_INPUT
    				first : USINT;
    				second : LREAL;
    				third : BOOL;
    				fourth : INT;
    			END_VAR
    			END_FUNCTION
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    ");
}

/// A string passed in is a STRING of 4096 bytes, its address and length
/// two values of the call; one that comes back is left out.
#[rstest]
fn a_string_goes_in_but_does_not_come_back(mut with_db: RootDatabase) {
    assert_snapshot!(checked(&mut with_db, r#"
        package wasi:example@0.3.0;

        interface text {
            greet: func(who: string);
            pair: func(first: string, count: u32, second: string) -> bool;
            name: func() -> string;
        }

        world example {
            import text;
        }
    "#), @r"
    // wasi-example-text.st
    NAMESPACE INTERNAL Wasi
    	NAMESPACE Example
    		NAMESPACE Text
    			{extern 'wasi:example/text@0.3.0' 'greet'}
    			FUNCTION GREET
    			VAR_INPUT
    				who : STRING[4096];
    			END_VAR
    			END_FUNCTION

    			{extern 'wasi:example/text@0.3.0' 'pair'}
    			FUNCTION PAIR : BOOL
    			VAR_INPUT
    				first : STRING[4096];
    				count : UDINT;
    				second : STRING[4096];
    			END_VAR
    			END_FUNCTION
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    /* Not generated:
       - `name`: its result holds a string, which an import cannot give back
         yet. */
    ");
}

/// A char has no ST type: a function that takes or gives one is left out.
#[rstest]
fn a_char_is_left_out(mut with_db: RootDatabase) {
    assert_snapshot!(checked(&mut with_db, r#"
        package wasi:example@0.3.0;

        interface letters {
            letter: func() -> char;
            put: func(c: char);
            count: func() -> u32;
        }

        world example {
            import letters;
        }
    "#), @r"
    // wasi-example-letters.st
    NAMESPACE INTERNAL Wasi
    	NAMESPACE Example
    		NAMESPACE Letters
    			{extern 'wasi:example/letters@0.3.0' 'count'}
    			FUNCTION COUNT : UDINT
    			END_FUNCTION
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    /* Not generated:
       - `letter`: its result holds a char, which an import cannot give back
         yet.
       - `put`: its parameter `c` holds a char, which an import cannot take
         yet. */
    ");
}
