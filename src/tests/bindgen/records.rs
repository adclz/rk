// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! Records and tuples: one passed in is one input per field, as the
//! canonical ABI flattens it, `when_seconds`, `pair_0`; one that comes back
//! is one output per field, which rk lays out as the canonical ABI lays out
//! the record. What holds something that cannot pass is left out.

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use super::checked;
use crate::tests::utils::with_db;

/// A record and a tuple passed in, field by field, a string field as a
/// STRING, an enum and a handle as themselves.
#[rstest]
fn a_record_passed_in_is_its_fields(mut with_db: RootDatabase) {
    assert_snapshot!(checked(&mut with_db, r#"
        package wasi:example@0.3.0;

        interface places {
            enum side { left, right }
            resource door;
            record instant { seconds: s64, nanoseconds: u32 }
            record label { text: string, size: u8 }
            record mixed { way: side, entry: borrow<door>, ratio: f32 }
            offset: func(when: instant) -> s64;
            split: func(pair: tuple<u8, u16, u64>);
            print: func(what: label, copies: u32);
            go: func(how: mixed);
        }

        world example {
            import places;
        }
    "#), @r"
    // wasi-example-places.st
    NAMESPACE INTERNAL Wasi
    	NAMESPACE Example
    		NAMESPACE Places
    			TYPE SIDE : USINT (
    				LEFT := 0,
    				RIGHT := 1
    			); END_TYPE

    			/* Lets go of `handle`, a handle of `door` the host gave. */
    			{extern 'wasi:example/places@0.3.0' '[resource-drop]door'}
    			FUNCTION DOOR_DROP
    			VAR_INPUT
    				handle : UDINT;
    			END_VAR
    			END_FUNCTION

    			{extern 'wasi:example/places@0.3.0' 'offset'}
    			FUNCTION OFFSET : LINT
    			VAR_INPUT
    				when_seconds : LINT;
    				when_nanoseconds : UDINT;
    			END_VAR
    			END_FUNCTION

    			{extern 'wasi:example/places@0.3.0' 'split'}
    			FUNCTION SPLIT
    			VAR_INPUT
    				pair_0 : USINT;
    				pair_1 : UINT;
    				pair_2 : ULINT;
    			END_VAR
    			END_FUNCTION

    			{extern 'wasi:example/places@0.3.0' 'print'}
    			FUNCTION PRINT
    			VAR_INPUT
    				what_text : STRING[4096];
    				what_size : USINT;
    				copies : UDINT;
    			END_VAR
    			END_FUNCTION

    			{extern 'wasi:example/places@0.3.0' 'go'}
    			FUNCTION GO
    			VAR_INPUT
    				how_way : SIDE;
    				how_entry : UDINT;
    				how_ratio : REAL;
    			END_VAR
    			END_FUNCTION
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    ");
}

/// A record or a tuple that comes back is an output per field, its fields'
/// names as written, a tuple's `item0`, `item1`; one of a single field is
/// the one value the call returns.
#[rstest]
fn a_record_that_comes_back_is_outputs(mut with_db: RootDatabase) {
    assert_snapshot!(checked(&mut with_db, r#"
        package wasi:example@0.3.0;

        interface readings {
            enum unit { kelvin, celsius }
            record reading { value: f64, unit: unit, valid: bool }
            record single { count: u32 }
            now: func() -> reading;
            count: func() -> single;
            pair: func() -> tuple<u64, s8>;
            keyword: func() -> tuple<bool>;
        }

        world example {
            import readings;
        }
    "#), @r"
    // wasi-example-readings.st
    NAMESPACE INTERNAL Wasi
    	NAMESPACE Example
    		NAMESPACE Readings
    			TYPE UNIT : USINT (
    				KELVIN := 0,
    				CELSIUS := 1
    			); END_TYPE

    			{extern 'wasi:example/readings@0.3.0' 'now'}
    			FUNCTION NOW
    			VAR_OUTPUT
    				value : LREAL;
    				unit : UNIT;
    				valid : BOOL;
    			END_VAR
    			END_FUNCTION

    			{extern 'wasi:example/readings@0.3.0' 'count'}
    			FUNCTION COUNT
    			VAR_OUTPUT
    				count : UDINT;
    			END_VAR
    			END_FUNCTION

    			{extern 'wasi:example/readings@0.3.0' 'pair'}
    			FUNCTION PAIR
    			VAR_OUTPUT
    				item0 : ULINT;
    				item1 : SINT;
    			END_VAR
    			END_FUNCTION

    			{extern 'wasi:example/readings@0.3.0' 'keyword'}
    			FUNCTION KEYWORD
    			VAR_OUTPUT
    				item0 : BOOL;
    			END_VAR
    			END_FUNCTION
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    ");
}

/// A field that is a keyword of ST takes a `_` when it is an output; an
/// input is named after its parameter first, which keeps it clear.
#[rstest]
fn a_field_named_as_a_keyword(mut with_db: RootDatabase) {
    assert_snapshot!(checked(&mut with_db, r#"
        package wasi:example@0.3.0;

        interface span {
            record range { step: u32, to: u32, by: u32 }
            make: func() -> range;
            walk: func(r: range);
        }

        world example {
            import span;
        }
    "#), @r"
    // wasi-example-span.st
    NAMESPACE INTERNAL Wasi
    	NAMESPACE Example
    		NAMESPACE Span
    			{extern 'wasi:example/span@0.3.0' 'make'}
    			FUNCTION MAKE
    			VAR_OUTPUT
    				step_ : UDINT;
    				to_ : UDINT;
    				by_ : UDINT;
    			END_VAR
    			END_FUNCTION

    			{extern 'wasi:example/span@0.3.0' 'walk'}
    			FUNCTION WALK
    			VAR_INPUT
    				r_step : UDINT;
    				r_to : UDINT;
    				r_by : UDINT;
    			END_VAR
    			END_FUNCTION
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    ");
}

/// What a record holds must pass too: a record in a record, a string in a
/// record that comes back, a list in a record passed in.
#[rstest]
fn a_record_holding_what_cannot_pass_is_left_out(mut with_db: RootDatabase) {
    assert_snapshot!(checked(&mut with_db, r#"
        package wasi:example@0.3.0;

        interface nested {
            record point { x: f64, y: f64 }
            record segment { start: point, end: point }
            record named { name: string, id: u32 }
            record bag { items: list<u8> }
            draw: func(s: segment);
            longest: func() -> segment;
            who: func() -> named;
            fill: func(b: bag);
        }

        world example {
            import nested;
        }
    "#), @r"
    // wasi-example-nested.st
    /* Not generated:
       - `draw`: its parameter `s` holds a record, which an import cannot take
         yet.
       - `longest`: its result holds a record, which an import cannot give back
         yet.
       - `who`: its result holds a string, which an import cannot give back
         yet.
       - `fill`: its parameter `b` holds a list, which an import cannot take
         yet. */
    ");
}
