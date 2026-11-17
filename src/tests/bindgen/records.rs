// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! Records and tuples: a record, and a tuple WIT names, is a STRUCT of its
//! fields, a tuple's `item0`, `item1`. One passed in is one input, which
//! the FUNCTION passes to its import field by field, as the canonical ABI
//! flattens it, `when_seconds`; a tuple no name holds is one input per
//! item, `pair_0`. One that comes back is the FUNCTION's own value: the
//! import gives a record of scalars as its outputs, which rk lays out as
//! the canonical ABI lays out the record, and the FUNCTION puts each in its
//! member; a record that holds a record comes back in words, each field
//! read out of them into its member. What holds something that cannot
//! pass is left out.

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use super::checked;
use crate::tests::utils::with_db;

/// A record passed in is one input of its STRUCT, which the FUNCTION
/// passes to its import field by field, a string field as a STRING, an
/// enum and a handle as themselves; a tuple no name holds is one input per
/// item.
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

    			TYPE DOOR : UDINT; END_TYPE

    			/* Lets go of `handle`, a handle of `door` the host gave. */
    			{extern 'wasi:example/places@0.3.0' '[resource-drop]door'}
    			FUNCTION DOOR_DROP
    			VAR_INPUT
    				handle : DOOR;
    			END_VAR
    			END_FUNCTION

    			TYPE INSTANT : STRUCT
    				seconds : LINT;
    				nanoseconds : UDINT;
    			END_STRUCT END_TYPE

    			TYPE LABEL : STRUCT
    				text : STRING[256];
    				size : USINT;
    			END_STRUCT END_TYPE

    			TYPE MIXED : STRUCT
    				way : SIDE;
    				entry : DOOR;
    				ratio : REAL;
    			END_STRUCT END_TYPE

    			/* `offset` as the host takes it: its parameters as the canonical ABI
    			   passes them, which `OFFSET` passes field by field. */
    			{extern 'wasi:example/places@0.3.0' 'offset'}
    			FUNCTION PRIVATE OFFSET_RAW : LINT
    			VAR_INPUT
    				when_seconds : LINT;
    				when_nanoseconds : UDINT;
    			END_VAR
    			END_FUNCTION

    			FUNCTION OFFSET : LINT
    			VAR_INPUT
    				when : INSTANT;
    			END_VAR
    				OFFSET := OFFSET_RAW(when_seconds := when.seconds, when_nanoseconds := when.nanoseconds);
    			END_FUNCTION

    			{extern 'wasi:example/places@0.3.0' 'split'}
    			FUNCTION SPLIT
    			VAR_INPUT
    				pair_0 : USINT;
    				pair_1 : UINT;
    				pair_2 : ULINT;
    			END_VAR
    			END_FUNCTION

    			/* `print` as the host takes it: its parameters as the canonical ABI passes
    			   them, which `PRINT` passes field by field. */
    			{extern 'wasi:example/places@0.3.0' 'print'}
    			FUNCTION PRIVATE PRINT_RAW
    			VAR_INPUT
    				what_text : STRING[4096];
    				what_size : USINT;
    				copies : UDINT;
    			END_VAR
    			END_FUNCTION

    			FUNCTION PRINT
    			VAR_INPUT
    				what : LABEL;
    				copies : UDINT;
    			END_VAR
    				PRINT_RAW(what_text := what.text, what_size := what.size, copies := copies);
    			END_FUNCTION

    			/* `go` as the host takes it: its parameters as the canonical ABI passes
    			   them, which `GO` passes field by field. */
    			{extern 'wasi:example/places@0.3.0' 'go'}
    			FUNCTION PRIVATE GO_RAW
    			VAR_INPUT
    				how_way : SIDE;
    				how_entry : DOOR;
    				how_ratio : REAL;
    			END_VAR
    			END_FUNCTION

    			FUNCTION GO
    			VAR_INPUT
    				how : MIXED;
    			END_VAR
    				GO_RAW(how_way := how.way, how_entry := how.entry, how_ratio := how.ratio);
    			END_FUNCTION
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    ");
}

/// A record that comes back is the FUNCTION's own value: its import gives
/// the fields as outputs, under their names as written, and the FUNCTION
/// puts each in its member; one of a single field is the one value the
/// import returns, however deep, its output named after its place
/// (`inner_count`). A tuple no name holds is an output per item, `item0`,
/// `item1`.
#[rstest]
fn a_record_that_comes_back_is_the_function_s_value(mut with_db: RootDatabase) {
    assert_snapshot!(checked(&mut with_db, r#"
        package wasi:example@0.3.0;

        interface readings {
            enum unit { kelvin, celsius }
            record reading { value: f64, unit: unit, valid: bool }
            record single { count: u32 }
            record boxed { inner: single }
            now: func() -> reading;
            count: func() -> single;
            unbox: func() -> boxed;
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

    			TYPE READING : STRUCT
    				value : LREAL;
    				unit : UNIT;
    				valid : BOOL;
    			END_STRUCT END_TYPE

    			TYPE SINGLE_ : STRUCT
    				count : UDINT;
    			END_STRUCT END_TYPE

    			TYPE BOXED : STRUCT
    				inner : SINGLE_;
    			END_STRUCT END_TYPE

    			/* `now` as the host gives it: the values of its result, which `NOW` puts
    			   in its members. */
    			{extern 'wasi:example/readings@0.3.0' 'now'}
    			FUNCTION PRIVATE NOW_RAW
    			VAR_OUTPUT
    				value : LREAL;
    				unit : UNIT;
    				valid : BOOL;
    			END_VAR
    			END_FUNCTION

    			FUNCTION NOW : READING
    				NOW_RAW(value => NOW.value, unit => NOW.unit, valid => NOW.valid);
    			END_FUNCTION

    			/* `count` as the host gives it: the one value of its result, which `COUNT`
    			   puts in its member. */
    			{extern 'wasi:example/readings@0.3.0' 'count'}
    			FUNCTION PRIVATE COUNT_RAW
    			VAR_OUTPUT
    				count : UDINT;
    			END_VAR
    			END_FUNCTION

    			FUNCTION COUNT : SINGLE_
    				COUNT_RAW(count => COUNT.count);
    			END_FUNCTION

    			/* `unbox` as the host gives it: the one value of its result, which `UNBOX`
    			   puts in its member. */
    			{extern 'wasi:example/readings@0.3.0' 'unbox'}
    			FUNCTION PRIVATE UNBOX_RAW
    			VAR_OUTPUT
    				inner_count : UDINT;
    			END_VAR
    			END_FUNCTION

    			FUNCTION UNBOX : BOXED
    				UNBOX_RAW(inner_count => UNBOX.inner.count);
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

/// A field that is a keyword of ST takes a `_`, as a member and as an
/// output of the import; an input of the import is named after its
/// parameter first, which keeps it clear.
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
    			TYPE RANGE : STRUCT
    				step_ : UDINT;
    				to_ : UDINT;
    				by_ : UDINT;
    			END_STRUCT END_TYPE

    			/* `make` as the host gives it: the values of its result, which `MAKE` puts
    			   in its members. */
    			{extern 'wasi:example/span@0.3.0' 'make'}
    			FUNCTION PRIVATE MAKE_RAW
    			VAR_OUTPUT
    				step_ : UDINT;
    				to_ : UDINT;
    				by_ : UDINT;
    			END_VAR
    			END_FUNCTION

    			FUNCTION MAKE : RANGE
    				MAKE_RAW(step_ => MAKE.step_, to_ => MAKE.to_, by_ => MAKE.by_);
    			END_FUNCTION

    			/* `walk` as the host takes it: its parameters as the canonical ABI passes
    			   them, which `WALK` passes field by field. */
    			{extern 'wasi:example/span@0.3.0' 'walk'}
    			FUNCTION PRIVATE WALK_RAW
    			VAR_INPUT
    				r_step : UDINT;
    				r_to : UDINT;
    				r_by : UDINT;
    			END_VAR
    			END_FUNCTION

    			FUNCTION WALK
    			VAR_INPUT
    				r_ : RANGE;
    			END_VAR
    				WALK_RAW(r_step := r_.step_, r_to := r_.to_, r_by := r_.by_);
    			END_FUNCTION
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    ");
}

/// A record that holds records passes field by field, each input of the
/// import named after both, as the canonical ABI flattens it. It comes
/// back in words, as no record of outputs is laid out: each record inside
/// is aligned on its own. Each field is read where the canonical ABI puts
/// it, into its member.
#[rstest]
fn a_record_of_records_passes_by_field_and_comes_back_in_words(mut with_db: RootDatabase) {
    assert_snapshot!(checked(&mut with_db, r#"
        package wasi:example@0.3.0;

        interface nested {
            record point { x: f64, y: f64 }
            record segment { start: point, end: point, width: u8 }
            draw: func(s: segment);
            longest: func() -> segment;
        }

        world example {
            import nested;
        }
    "#), @r"
    // wasi-example-nested.st
    NAMESPACE INTERNAL Wasi
    	NAMESPACE Example
    		NAMESPACE Nested
    			USING Std.Convert;

    			TYPE POINT : STRUCT
    				x : LREAL;
    				y : LREAL;
    			END_STRUCT END_TYPE

    			TYPE SEGMENT : STRUCT
    				start : POINT;
    				end : POINT;
    				width : USINT;
    			END_STRUCT END_TYPE

    			/* `draw` as the host takes it: its parameters as the canonical ABI passes
    			   them, which `DRAW` passes field by field. */
    			{extern 'wasi:example/nested@0.3.0' 'draw'}
    			FUNCTION PRIVATE DRAW_RAW
    			VAR_INPUT
    				s_start_x : LREAL;
    				s_start_y : LREAL;
    				s_end_x : LREAL;
    				s_end_y : LREAL;
    				s_width : USINT;
    			END_VAR
    			END_FUNCTION

    			FUNCTION DRAW
    			VAR_INPUT
    				s_ : SEGMENT;
    			END_VAR
    				DRAW_RAW(s_start_x := s_.start.x, s_start_y := s_.start.y, s_end_x := s_.end.x, s_end_y := s_.end.y, s_width := s_.width);
    			END_FUNCTION

    			/* `longest` as the host gives it: its result in words of 8 bytes, as the
    			   canonical ABI lays it out, which `LONGEST` reads. */
    			{extern 'wasi:example/nested@0.3.0' 'longest'}
    			FUNCTION PRIVATE LONGEST_RAW
    			VAR_OUTPUT
    				w0 : LWORD;
    				w1 : LWORD;
    				w2 : LWORD;
    				w3 : LWORD;
    				w4 : LWORD;
    			END_VAR
    			END_FUNCTION

    			FUNCTION LONGEST : SEGMENT
    			VAR
    				w0 : LWORD;
    				w1 : LWORD;
    				w2 : LWORD;
    				w3 : LWORD;
    				w4 : LWORD;
    			END_VAR
    				LONGEST_RAW(w0 => w0, w1 => w1, w2 => w2, w3 => w3, w4 => w4);
    				LONGEST.start.x := LWORD_TO_LREAL(w0);
    				LONGEST.start.y := LWORD_TO_LREAL(w1);
    				LONGEST.end.x := LWORD_TO_LREAL(w2);
    				LONGEST.end.y := LWORD_TO_LREAL(w3);
    				LONGEST.width := BYTE_TO_USINT(w4.%B0);
    			END_FUNCTION
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    ");
}

/// What a record holds must pass too: a string in a record that comes
/// back is written into its member, a STRING of 256 bytes, which the call
/// passes; a record that holds a list has no STRUCT, and a function that
/// passes one is left out.
#[rstest]
fn a_record_s_string_comes_back_and_its_list_is_left_out(mut with_db: RootDatabase) {
    assert_snapshot!(checked(&mut with_db, r#"
        package wasi:example@0.3.0;

        interface nested {
            record named { name: string, id: u32 }
            record bag { items: list<u8> }
            who: func() -> named;
            fill: func(b: bag);
        }

        world example {
            import nested;
        }
    "#), @r"
    // wasi-example-nested.st
    NAMESPACE INTERNAL Wasi
    	NAMESPACE Example
    		NAMESPACE Nested
    			USING Std.Convert;

    			TYPE NAMED : STRUCT
    				name : STRING[256];
    				id : UDINT;
    			END_STRUCT END_TYPE

    			/* `who` as the host gives it: its result in words of 8 bytes, as the
    			   canonical ABI lays it out, which `WHO` reads, and its string in `name`,
    			   where the host writes it. */
    			{extern 'wasi:example/nested@0.3.0' 'who'}
    			FUNCTION PRIVATE WHO_RAW
    			VAR_IN_OUT
    				name : STRING;
    			END_VAR
    			VAR_OUTPUT
    				w0 : LWORD;
    				w1 : LWORD;
    			END_VAR
    			END_FUNCTION

    			FUNCTION WHO : NAMED
    			VAR
    				w0 : LWORD;
    				w1 : LWORD;
    			END_VAR
    				WHO_RAW(name := WHO.name, w0 => w0, w1 => w1);
    				WHO.id := DWORD_TO_UDINT(w1.%D0);
    			END_FUNCTION
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    /* Not generated:
       - `bag`: it holds a list, which ST has no type for yet.
       - `fill`: its parameter `b` holds a list, which an import cannot take
         yet. */
    ");
}
