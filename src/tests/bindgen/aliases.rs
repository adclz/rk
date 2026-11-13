// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! Aliases: `type ticks = u64` is a TYPE of its own, named where a function
//! takes or gives it; a `use` names the type where it is defined; an alias
//! of what is passed field by field, or of a string, writes no TYPE.

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use super::checked;
use crate::tests::utils::with_db;

/// An alias of a scalar is a TYPE, and an alias of an alias names the
/// first.
#[rstest]
fn an_alias_of_a_scalar_is_a_type(mut with_db: RootDatabase) {
    assert_snapshot!(checked(&mut with_db, r#"
        package wasi:example@0.3.0;

        interface clock {
            /// A count of ticks.
            type ticks = u64;
            type span = ticks;
            now: func() -> ticks;
            wait: func(how-long: span) -> bool;
        }

        world example {
            import clock;
        }
    "#), @r"
    // wasi-example-clock.st
    NAMESPACE INTERNAL Wasi
    	NAMESPACE Example
    		NAMESPACE Clock
    			/* A count of ticks. */
    			TYPE TICKS : ULINT; END_TYPE

    			TYPE SPAN : TICKS; END_TYPE

    			{extern 'wasi:example/clock@0.3.0' 'now'}
    			FUNCTION NOW : TICKS
    			END_FUNCTION

    			{extern 'wasi:example/clock@0.3.0' 'wait'}
    			FUNCTION WAIT : BOOL
    			VAR_INPUT
    				how_long : SPAN;
    			END_VAR
    			END_FUNCTION
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    ");
}

/// A type another interface `use`s is named where it is defined, through
/// `Wasi`, its package and its interface, a renamed one too.
#[rstest]
fn a_used_type_is_named_where_it_is_defined(mut with_db: RootDatabase) {
    assert_snapshot!(checked(&mut with_db, r#"
        package wasi:example@0.3.0;

        interface types {
            type duration = u64;
            enum unit { second, minute }
        }

        interface timer {
            use types.{duration, unit, duration as span};
            start: func(how-long: duration, by: unit) -> span;
        }

        world example {
            import timer;
        }
    "#), @r"
    // wasi-example-types.st
    NAMESPACE INTERNAL Wasi
    	NAMESPACE Example
    		NAMESPACE Types
    			TYPE DURATION : ULINT; END_TYPE

    			TYPE UNIT : USINT (
    				SECOND := 0,
    				MINUTE := 1
    			); END_TYPE
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE

    // wasi-example-timer.st
    NAMESPACE INTERNAL Wasi
    	NAMESPACE Example
    		NAMESPACE Timer
    			{extern 'wasi:example/timer@0.3.0' 'start'}
    			FUNCTION START : Wasi.Example.Types.DURATION
    			VAR_INPUT
    				how_long : Wasi.Example.Types.DURATION;
    				by_ : Wasi.Example.Types.UNIT;
    			END_VAR
    			END_FUNCTION
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    ");
}

/// An alias of a record, a tuple, an option, a result or a string writes
/// no TYPE: the functions that take or give one pass it as they would pass
/// what it names. An alias of a list is left out, with the functions.
#[rstest]
fn an_alias_of_what_is_passed_otherwise_writes_no_type(mut with_db: RootDatabase) {
    assert_snapshot!(checked(&mut with_db, r#"
        package wasi:example@0.3.0;

        interface shapes {
            record point { x: f64, y: f64 }
            type spot = point;
            type pair = tuple<u32, u32>;
            type maybe = option<u32>;
            type outcome = result<u32>;
            type path = string;
            type names = list<string>;
            move-to: func(where: spot) -> pair;
            find: func(at: path) -> maybe;
            check: func() -> outcome;
            list-all: func() -> names;
            here: func() -> path;
        }

        world example {
            import shapes;
        }
    "#), @r"
    // wasi-example-shapes.st
    NAMESPACE INTERNAL Wasi
    	NAMESPACE Example
    		NAMESPACE Shapes
    			{extern 'wasi:example/shapes@0.3.0' 'move-to'}
    			FUNCTION MOVE_TO
    			VAR_INPUT
    				where_x : LREAL;
    				where_y : LREAL;
    			END_VAR
    			VAR_OUTPUT
    				item0 : UDINT;
    				item1 : UDINT;
    			END_VAR
    			END_FUNCTION

    			{extern 'wasi:example/shapes@0.3.0' 'find'}
    			FUNCTION FIND
    			VAR_INPUT
    				at_ : STRING[4096];
    			END_VAR
    			VAR_OUTPUT
    				is_some : BOOL;
    				value : UDINT;
    			END_VAR
    			END_FUNCTION

    			{extern 'wasi:example/shapes@0.3.0' 'check'}
    			FUNCTION CHECK
    			VAR_OUTPUT
    				failed : BOOL;
    				value : UDINT;
    			END_VAR
    			END_FUNCTION
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    /* Not generated:
       - `names`: it is a list, which ST has no type for yet.
       - `list-all`: its result holds a list, which an import cannot give back
         yet.
       - `here`: its result holds a string, which an import cannot give back
         yet. */
    ");
}
