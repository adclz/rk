// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! A list that comes back: the host writes its elements into the arrays the
//! call passes, `ARRAY[*]` `VAR_IN_OUT`s of any length, one per field of an
//! element, and its strings into their elements of a STRING array; `count`
//! says how many came. A list longer than an array, or a string longer than
//! its STRING, raises. One list comes back so, from a call that waits for
//! its result, outside its error and outside a variant's cases.

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;
use wasmtime::component::{Linker, Val};

use super::{checked, hosted};
use crate::tests::cli::component_host::Host;
use crate::tests::utils::with_db;

/// A list of bytes, of numbers, of an enum, of strings, of pairs of
/// strings, of a number and a string, of records; in an option and as a
/// result's value; then what is left out: a list of lists, of options, a
/// list beside a string, two lists, a list in an error, and one from an
/// async call.
const LISTS: &str = r#"
    package wasi:example@0.3.0;

    interface lists {
        enum color { red, green, blue }
        record reading { id: u32, name: string, ok: bool }
        bytes: func(size: u32) -> list<u8>;
        numbers: func(size: u32) -> list<u32>;
        colors: func() -> list<color>;
        names: func(size: u32) -> list<string>;
        pairs: func(size: u32) -> list<tuple<string, string>>;
        entries: func() -> list<tuple<u32, string>>;
        readings: func() -> list<reading>;
        maybe: func(some: bool) -> option<list<u16>>;
        fetched: func(fail: bool) -> result<list<u8>, color>;
        label: func() -> string;
        nested: func() -> list<list<u8>>;
        optional: func() -> list<option<u32>>;
        beside: func() -> tuple<list<u8>, string>;
        two: func() -> tuple<list<u8>, list<u8>>;
        refused: func() -> result<u32, list<u8>>;
        later: async func() -> list<u8>;
    }

    world example {
        import lists;
    }
"#;

#[rstest]
fn a_list_comes_back_in_the_arrays_the_call_passes(mut with_db: RootDatabase) {
    assert_snapshot!(checked(&mut with_db, LISTS), @r"
    // wasi-example-lists.st
    NAMESPACE INTERNAL Wasi
    	NAMESPACE Example
    		NAMESPACE Lists
    			USING Std.Convert;

    			TYPE COLOR : USINT (
    				RED := 0,
    				GREEN := 1,
    				BLUE := 2
    			); END_TYPE

    			TYPE READING : STRUCT
    				id : UDINT;
    				name : STRING[256];
    				ok : BOOL;
    			END_STRUCT END_TYPE

    			/* `bytes` as the host gives it: its result in words of 8 bytes, as the
    			   canonical ABI lays it out, which `BYTES` reads, and its list in `value`,
    			   where the host writes it. */
    			{extern 'wasi:example/lists@0.3.0' 'bytes'}
    			FUNCTION PRIVATE BYTES_RAW
    			VAR_INPUT
    				size : UDINT;
    			END_VAR
    			VAR_IN_OUT
    				value : ARRAY[*] OF BYTE;
    			END_VAR
    			VAR_OUTPUT
    				w0 : LWORD;
    				w1 : LWORD;
    			END_VAR
    			END_FUNCTION

    			FUNCTION BYTES
    			VAR_INPUT
    				size : UDINT;
    			END_VAR
    			VAR_IN_OUT
    				value : ARRAY[*] OF BYTE;
    			END_VAR
    			VAR_OUTPUT
    				count : UDINT;
    			END_VAR
    			VAR
    				w0 : LWORD;
    				w1 : LWORD;
    			END_VAR
    				BYTES_RAW(size := size, value := value, w0 => w0, w1 => w1);
    				count := DWORD_TO_UDINT(w0.%D1);
    			END_FUNCTION

    			/* `numbers` as the host gives it: its result in words of 8 bytes, as the
    			   canonical ABI lays it out, which `NUMBERS` reads, and its list in
    			   `value`, where the host writes it. */
    			{extern 'wasi:example/lists@0.3.0' 'numbers'}
    			FUNCTION PRIVATE NUMBERS_RAW
    			VAR_INPUT
    				size : UDINT;
    			END_VAR
    			VAR_IN_OUT
    				value : ARRAY[*] OF UDINT;
    			END_VAR
    			VAR_OUTPUT
    				w0 : LWORD;
    				w1 : LWORD;
    			END_VAR
    			END_FUNCTION

    			FUNCTION NUMBERS
    			VAR_INPUT
    				size : UDINT;
    			END_VAR
    			VAR_IN_OUT
    				value : ARRAY[*] OF UDINT;
    			END_VAR
    			VAR_OUTPUT
    				count : UDINT;
    			END_VAR
    			VAR
    				w0 : LWORD;
    				w1 : LWORD;
    			END_VAR
    				NUMBERS_RAW(size := size, value := value, w0 => w0, w1 => w1);
    				count := DWORD_TO_UDINT(w0.%D1);
    			END_FUNCTION

    			/* `colors` as the host gives it: its result in words of 8 bytes, as the
    			   canonical ABI lays it out, which `COLORS` reads, and its list in
    			   `value`, where the host writes it. */
    			{extern 'wasi:example/lists@0.3.0' 'colors'}
    			FUNCTION PRIVATE COLORS_RAW
    			VAR_IN_OUT
    				value : ARRAY[*] OF COLOR;
    			END_VAR
    			VAR_OUTPUT
    				w0 : LWORD;
    				w1 : LWORD;
    			END_VAR
    			END_FUNCTION

    			FUNCTION COLORS
    			VAR_IN_OUT
    				value : ARRAY[*] OF COLOR;
    			END_VAR
    			VAR_OUTPUT
    				count : UDINT;
    			END_VAR
    			VAR
    				w0 : LWORD;
    				w1 : LWORD;
    			END_VAR
    				COLORS_RAW(value := value, w0 => w0, w1 => w1);
    				count := DWORD_TO_UDINT(w0.%D1);
    			END_FUNCTION

    			/* `names` as the host gives it: its result in words of 8 bytes, as the
    			   canonical ABI lays it out, which `NAMES` reads, and its list in `value`,
    			   where the host writes it. */
    			{extern 'wasi:example/lists@0.3.0' 'names'}
    			FUNCTION PRIVATE NAMES_RAW
    			VAR_INPUT
    				size : UDINT;
    			END_VAR
    			VAR_IN_OUT
    				value : ARRAY[*] OF STRING;
    			END_VAR
    			VAR_OUTPUT
    				w0 : LWORD;
    				w1 : LWORD;
    			END_VAR
    			END_FUNCTION

    			FUNCTION NAMES
    			VAR_INPUT
    				size : UDINT;
    			END_VAR
    			VAR_IN_OUT
    				value : ARRAY[*] OF STRING;
    			END_VAR
    			VAR_OUTPUT
    				count : UDINT;
    			END_VAR
    			VAR
    				w0 : LWORD;
    				w1 : LWORD;
    			END_VAR
    				NAMES_RAW(size := size, value := value, w0 => w0, w1 => w1);
    				count := DWORD_TO_UDINT(w0.%D1);
    			END_FUNCTION

    			/* `pairs` as the host gives it: its result in words of 8 bytes, as the
    			   canonical ABI lays it out, which `PAIRS` reads, and its list in `item0`,
    			   `item1`, where the host writes it. */
    			{extern 'wasi:example/lists@0.3.0' 'pairs'}
    			FUNCTION PRIVATE PAIRS_RAW
    			VAR_INPUT
    				size : UDINT;
    			END_VAR
    			VAR_IN_OUT
    				item0 : ARRAY[*] OF STRING;
    				item1 : ARRAY[*] OF STRING;
    			END_VAR
    			VAR_OUTPUT
    				w0 : LWORD;
    				w1 : LWORD;
    			END_VAR
    			END_FUNCTION

    			FUNCTION PAIRS
    			VAR_INPUT
    				size : UDINT;
    			END_VAR
    			VAR_IN_OUT
    				item0 : ARRAY[*] OF STRING;
    				item1 : ARRAY[*] OF STRING;
    			END_VAR
    			VAR_OUTPUT
    				count : UDINT;
    			END_VAR
    			VAR
    				w0 : LWORD;
    				w1 : LWORD;
    			END_VAR
    				PAIRS_RAW(size := size, item0 := item0, item1 := item1, w0 => w0, w1 => w1);
    				count := DWORD_TO_UDINT(w0.%D1);
    			END_FUNCTION

    			/* `entries` as the host gives it: its result in words of 8 bytes, as the
    			   canonical ABI lays it out, which `ENTRIES` reads, and its list in
    			   `item0`, `item1`, where the host writes it. */
    			{extern 'wasi:example/lists@0.3.0' 'entries'}
    			FUNCTION PRIVATE ENTRIES_RAW
    			VAR_IN_OUT
    				item0 : ARRAY[*] OF UDINT;
    				item1 : ARRAY[*] OF STRING;
    			END_VAR
    			VAR_OUTPUT
    				w0 : LWORD;
    				w1 : LWORD;
    			END_VAR
    			END_FUNCTION

    			FUNCTION ENTRIES
    			VAR_IN_OUT
    				item0 : ARRAY[*] OF UDINT;
    				item1 : ARRAY[*] OF STRING;
    			END_VAR
    			VAR_OUTPUT
    				count : UDINT;
    			END_VAR
    			VAR
    				w0 : LWORD;
    				w1 : LWORD;
    			END_VAR
    				ENTRIES_RAW(item0 := item0, item1 := item1, w0 => w0, w1 => w1);
    				count := DWORD_TO_UDINT(w0.%D1);
    			END_FUNCTION

    			/* `readings` as the host gives it: its result in words of 8 bytes, as the
    			   canonical ABI lays it out, which `READINGS` reads, and its list in `id`,
    			   `name`, `ok`, where the host writes it. */
    			{extern 'wasi:example/lists@0.3.0' 'readings'}
    			FUNCTION PRIVATE READINGS_RAW
    			VAR_IN_OUT
    				id : ARRAY[*] OF UDINT;
    				name : ARRAY[*] OF STRING;
    				ok : ARRAY[*] OF BOOL;
    			END_VAR
    			VAR_OUTPUT
    				w0 : LWORD;
    				w1 : LWORD;
    			END_VAR
    			END_FUNCTION

    			FUNCTION READINGS
    			VAR_IN_OUT
    				id : ARRAY[*] OF UDINT;
    				name : ARRAY[*] OF STRING;
    				ok : ARRAY[*] OF BOOL;
    			END_VAR
    			VAR_OUTPUT
    				count : UDINT;
    			END_VAR
    			VAR
    				w0 : LWORD;
    				w1 : LWORD;
    			END_VAR
    				READINGS_RAW(id := id, name := name, ok := ok, w0 => w0, w1 => w1);
    				count := DWORD_TO_UDINT(w0.%D1);
    			END_FUNCTION

    			/* `maybe` as the host gives it: its result in words of 8 bytes, as the
    			   canonical ABI lays it out, which `MAYBE` reads, and its list in `value`,
    			   where the host writes it. */
    			{extern 'wasi:example/lists@0.3.0' 'maybe'}
    			FUNCTION PRIVATE MAYBE_RAW
    			VAR_INPUT
    				some : BOOL;
    			END_VAR
    			VAR_IN_OUT
    				value : ARRAY[*] OF UINT;
    			END_VAR
    			VAR_OUTPUT
    				w0 : LWORD;
    				w1 : LWORD;
    			END_VAR
    			END_FUNCTION

    			FUNCTION MAYBE
    			VAR_INPUT
    				some : BOOL;
    			END_VAR
    			VAR_IN_OUT
    				value : ARRAY[*] OF UINT;
    			END_VAR
    			VAR_OUTPUT
    				is_some : BOOL;
    				value_count : UDINT;
    			END_VAR
    			VAR
    				w0 : LWORD;
    				w1 : LWORD;
    			END_VAR
    				MAYBE_RAW(some := some, value := value, w0 => w0, w1 => w1);
    				value_count := 0;
    				is_some := w0.%X0;
    				IF is_some THEN
    					value_count := DWORD_TO_UDINT(w1.%D0);
    				END_IF;
    			END_FUNCTION

    			/* `fetched` as the host gives it: its result in words of 8 bytes, as the
    			   canonical ABI lays it out, which `FETCHED` reads, and its list in
    			   `value`, where the host writes it. */
    			{extern 'wasi:example/lists@0.3.0' 'fetched'}
    			FUNCTION PRIVATE FETCHED_RAW
    			VAR_INPUT
    				fail : BOOL;
    			END_VAR
    			VAR_IN_OUT
    				value : ARRAY[*] OF BYTE;
    			END_VAR
    			VAR_OUTPUT
    				w0 : LWORD;
    				w1 : LWORD;
    			END_VAR
    			END_FUNCTION

    			FUNCTION FETCHED
    			VAR_INPUT
    				fail : BOOL;
    			END_VAR
    			VAR_IN_OUT
    				value : ARRAY[*] OF BYTE;
    			END_VAR
    			VAR_OUTPUT
    				failed : BOOL;
    				value_count : UDINT;
    				error : COLOR;
    			END_VAR
    			VAR
    				w0 : LWORD;
    				w1 : LWORD;
    			END_VAR
    				FETCHED_RAW(fail := fail, value := value, w0 => w0, w1 => w1);
    				value_count := 0;
    				error := 0;
    				failed := w0.%X0;
    				IF failed THEN
    					error := BYTE_TO_USINT(w0.%B4);
    				ELSE
    					value_count := DWORD_TO_UDINT(w1.%D0);
    				END_IF;
    			END_FUNCTION

    			/* `label` as the host gives it: its result in words of 8 bytes, as the
    			   canonical ABI lays it out, which `LABEL` reads, and its string in
    			   `value`, where the host writes it. */
    			{extern 'wasi:example/lists@0.3.0' 'label'}
    			FUNCTION PRIVATE LABEL_RAW
    			VAR_IN_OUT
    				value : STRING;
    			END_VAR
    			VAR_OUTPUT
    				w0 : LWORD;
    				w1 : LWORD;
    			END_VAR
    			END_FUNCTION

    			FUNCTION LABEL
    			VAR_IN_OUT
    				value : STRING;
    			END_VAR
    			VAR
    				w0 : LWORD;
    				w1 : LWORD;
    			END_VAR
    				LABEL_RAW(value := value, w0 => w0, w1 => w1);
    			END_FUNCTION
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    /* Not generated:
       - `nested`: its result holds a list, which an import cannot give back
         yet.
       - `optional`: its result holds a list, which an import cannot give back
         yet.
       - `beside`: its result holds two strings or a string and a list, which
         an import cannot give back yet.
       - `two`: its result holds two strings or a string and a list, which an
         import cannot give back yet.
       - `refused`: its result holds a list, which an import cannot give back
         yet.
       - `later`: its result holds a list, which an import cannot give back
         yet. */
    ");
}

/// What the host of [`LISTS`] gives a function, from what it was passed.
type Gives = Box<dyn Fn(&[Val]) -> Val + Send + Sync>;

/// The host's side of [`LISTS`]: each list as the call asks for it, the
/// first `size` items of a fixed one where it takes a size.
fn lists_host(linker: &mut Linker<Host>) {
    let size = |params: &[Val]| match params {
        [Val::U32(size)] => *size as usize,
        other => panic!("`size`, a u32: {other:?}"),
    };
    let flag = |params: &[Val]| match params {
        [Val::Bool(flag)] => *flag,
        other => panic!("a bool: {other:?}"),
    };
    let string = |text: &str| Val::String(text.to_string());
    let mut lists = linker
        .instance("wasi:example/lists@0.3.0")
        .expect("the interface");
    let mut define = |name: &str, give: Gives| {
        lists
            .func_new(name, move |_, _, params, results| {
                results[0] = give(params);
                Ok(())
            })
            .expect("a function of the host");
    };
    define(
        "bytes",
        Box::new(move |params| Val::List((0..size(params)).map(|at| Val::U8(at as u8)).collect())),
    );
    define(
        "numbers",
        Box::new(move |params| {
            Val::List(
                (0..size(params))
                    .map(|at| Val::U32(1000 + at as u32))
                    .collect(),
            )
        }),
    );
    define(
        "colors",
        Box::new(|_| {
            Val::List(
                ["blue", "red", "green"]
                    .iter()
                    .map(|color| Val::Enum(color.to_string()))
                    .collect(),
            )
        }),
    );
    define(
        "names",
        Box::new(move |params| {
            let names = ["a", "", "ccc", "héllo", "too-long!"];
            Val::List(
                names[..size(params)]
                    .iter()
                    .map(|name| string(name))
                    .collect(),
            )
        }),
    );
    define(
        "pairs",
        Box::new(move |params| {
            let pairs = [("HOME", "/root"), ("LANG", "C"), ("TERM", "dumb")];
            Val::List(
                pairs[..size(params)]
                    .iter()
                    .map(|(key, value)| Val::Tuple(vec![string(key), string(value)]))
                    .collect(),
            )
        }),
    );
    define(
        "entries",
        Box::new(move |_| {
            Val::List(vec![
                Val::Tuple(vec![Val::U32(7), string("seven")]),
                Val::Tuple(vec![Val::U32(11), string("eleven")]),
            ])
        }),
    );
    define(
        "readings",
        Box::new(move |_| {
            let reading = |id: u32, name: &str, ok: bool| {
                Val::Record(vec![
                    ("id".to_string(), Val::U32(id)),
                    ("name".to_string(), string(name)),
                    ("ok".to_string(), Val::Bool(ok)),
                ])
            };
            Val::List(vec![
                reading(1, "temp", true),
                reading(2, "flow", false),
                reading(3, "", true),
            ])
        }),
    );
    define(
        "maybe",
        Box::new(move |params| {
            Val::Option(
                flag(params).then(|| Box::new(Val::List(vec![Val::U16(1), Val::U16(65535)]))),
            )
        }),
    );
    define(
        "fetched",
        Box::new(move |params| {
            Val::Result(match flag(params) {
                false => Ok(Some(Box::new(Val::List(vec![
                    Val::U8(9),
                    Val::U8(8),
                    Val::U8(7),
                ])))),
                true => Err(Some(Box::new(Val::Enum("green".to_string())))),
            })
        }),
    );
    define("label", Box::new(move |_| string("after")));
}

/// What the host of [`LISTS`] gives, read: 1 when each list is in its
/// arrays and nowhere else, and otherwise the number of the check that
/// failed, negative.
const READ: &str = r#"
    /* An array, and right after it what a write past its end would reach. */
    TYPE GUARDED_BYTES : STRUCT
        data : ARRAY[0..7] OF BYTE;
        after : BYTE := 16#AA;
    END_STRUCT END_TYPE

    TYPE GUARDED_NAMES : STRUCT
        data : ARRAY[0..1] OF STRING[16];
        after : STRING[16] := 'guard';
    END_STRUCT END_TYPE

    FUNCTION check : DINT
    VAR
        raw : GUARDED_BYTES;
        numbers : ARRAY[1..4] OF UDINT;
        colors : ARRAY[0..3] OF Wasi.Example.Lists.COLOR;
        names : ARRAY[0..4] OF STRING[8];
        keys, values : ARRAY[0..3] OF STRING[16];
        few : GUARDED_NAMES;
        ids : ARRAY[0..3] OF UDINT;
        labels : ARRAY[0..3] OF STRING[8];
        oks : ARRAY[0..3] OF BOOL;
        halves : ARRAY[0..2] OF UINT;
        count : UDINT;
        known, failed, raised : BOOL;
        error : Wasi.Example.Lists.COLOR;
        why, s : STRING;
    END_VAR
        check := 0;
        // Bytes the host writes into the array itself; past them, nothing.
        raw.data[5] := 99;
        Wasi.Example.Lists.BYTES(size := 5, value := raw.data, count => count);
        IF count <> 5 OR raw.data[0] <> 0 OR raw.data[1] <> 1 OR raw.data[4] <> 4
            OR raw.data[5] <> 99
        THEN
            check := -1; RETURN;
        END_IF;
        Wasi.Example.Lists.BYTES(size := 8, value := raw.data, count => count);
        IF count <> 8 OR raw.data[7] <> 7 THEN check := -2; RETURN; END_IF;
        __TRY
            Wasi.Example.Lists.BYTES(size := 9, value := raw.data, count => count);
        __CATCH(why)
            raised := TRUE;
        __END_TRY;
        IF NOT raised OR why <> 'a list the host gave is longer than its array' THEN
            check := -3; RETURN;
        END_IF;
        // Nothing past the array was written.
        IF raw.after <> 16#AA THEN check := -18; RETURN; END_IF;

        // An array whose first element is 1.
        numbers[4] := 7;
        Wasi.Example.Lists.NUMBERS(size := 3, value := numbers, count => count);
        IF count <> 3 OR numbers[1] <> 1000 OR numbers[3] <> 1002 OR numbers[4] <> 7 THEN
            check := -4; RETURN;
        END_IF;
        Wasi.Example.Lists.COLORS(value := colors, count => count);
        IF count <> 3 OR colors[0] <> 2 OR colors[1] <> 0 OR colors[2] <> 1 THEN
            check := -5; RETURN;
        END_IF;

        // Each string into its element, an empty one too; one a byte
        // longer than its STRING raises.
        Wasi.Example.Lists.NAMES(size := 4, value := names, count => count);
        IF count <> 4 OR names[0] <> 'a' OR names[1] <> '' OR names[2] <> 'ccc'
            OR names[3] <> 'héllo'
        THEN
            check := -6; RETURN;
        END_IF;
        raised := FALSE;
        __TRY
            Wasi.Example.Lists.NAMES(size := 5, value := names, count => count);
        __CATCH(why)
            raised := TRUE;
        __END_TRY;
        IF NOT raised OR why <> 'a string the host gave is longer than its STRING' THEN
            check := -7; RETURN;
        END_IF;
        // The call after it is one like any other.
        Wasi.Example.Lists.NAMES(size := 2, value := names, count => count);
        IF count <> 2 OR names[0] <> 'a' OR names[1] <> '' THEN check := -8; RETURN; END_IF;

        // Two strings an element, each into its own array.
        Wasi.Example.Lists.PAIRS(size := 2, item0 := keys, item1 := values, count => count);
        IF count <> 2 OR keys[0] <> 'HOME' OR values[0] <> '/root' OR keys[1] <> 'LANG'
            OR values[1] <> 'C'
        THEN
            check := -9; RETURN;
        END_IF;
        // The shorter of two arrays bounds the list.
        raised := FALSE;
        __TRY
            Wasi.Example.Lists.PAIRS(size := 3, item0 := keys, item1 := few.data, count => count);
        __CATCH(why)
            raised := TRUE;
        __END_TRY;
        IF NOT raised OR why <> 'a list the host gave is longer than its array' THEN
            check := -10; RETURN;
        END_IF;
        IF few.after <> 'guard' THEN check := -19; RETURN; END_IF;

        // A number and a string an element: the number copied into its array.
        Wasi.Example.Lists.ENTRIES(item0 := ids, item1 := labels, count => count);
        IF count <> 2 OR ids[0] <> 7 OR labels[0] <> 'seven' OR ids[1] <> 11
            OR labels[1] <> 'eleven'
        THEN
            check := -11; RETURN;
        END_IF;
        Wasi.Example.Lists.READINGS(id := ids, name := labels, ok := oks, count => count);
        IF count <> 3 OR ids[0] <> 1 OR labels[0] <> 'temp' OR NOT oks[0] OR ids[1] <> 2
            OR labels[1] <> 'flow' OR oks[1] OR ids[2] <> 3 OR labels[2] <> '' OR NOT oks[2]
        THEN
            check := -12; RETURN;
        END_IF;

        Wasi.Example.Lists.MAYBE(some := FALSE, value := halves, is_some => known,
            value_count => count);
        IF known OR count <> 0 THEN check := -13; RETURN; END_IF;
        Wasi.Example.Lists.MAYBE(some := TRUE, value := halves, is_some => known,
            value_count => count);
        IF NOT known OR count <> 2 OR halves[0] <> 1 OR halves[1] <> 65535 THEN
            check := -14; RETURN;
        END_IF;

        Wasi.Example.Lists.FETCHED(fail := FALSE, value := raw.data, failed => failed,
            value_count => count, error => error);
        IF failed OR count <> 3 OR raw.data[0] <> 9 OR raw.data[2] <> 7 THEN
            check := -15; RETURN;
        END_IF;
        Wasi.Example.Lists.FETCHED(fail := TRUE, value := raw.data, failed => failed,
            value_count => count, error => error);
        IF NOT failed OR count <> 0 OR error <> 1 THEN check := -16; RETURN; END_IF;

        // A string after them, in the same module.
        Wasi.Example.Lists.LABEL(value := s);
        IF s <> 'after' THEN check := -17; RETURN; END_IF;
        check := 1;
    END_FUNCTION

    PROGRAM P
    VAR_EXTERNAL output : DINT; END_VAR
        output := check();
    END_PROGRAM
"#;

/// The host writes each list into the arrays the call passed: bytes and
/// numbers into the array itself, an enum, strings each into its element,
/// two strings an element into two arrays, a number and a string or a
/// record whose scalars are copied into theirs; in an option and as a
/// result's value. A list longer than an array, or a string longer than
/// its STRING, raises, and the call after it is one like any other.
#[rstest]
fn the_host_writes_a_list_into_the_arrays_passed(mut with_db: RootDatabase) {
    let mut plc = hosted(&mut with_db, LISTS, READ, lists_host);
    assert_eq!(
        plc.scan(0),
        1,
        "the number of the check that failed, negative"
    );
}

/// What the scan of [`each_call_starts_the_scratch_over`] checks: 1, or the
/// number of the check that failed, negative. Its second scan asks for a
/// list longer than any place the module has.
const AGAIN: &str = r#"
    FUNCTION check : DINT
    VAR
        names : ARRAY[0..4] OF STRING[8];
        count : UDINT;
        i : DINT;
    END_VAR
        check := 0;
        // Each call's elements land from the start of the scratch.
        FOR i := 1 TO 5000 DO
            Wasi.Example.Lists.NAMES(size := 4, value := names, count => count);
        END_FOR;
        IF count <> 4 OR names[0] <> 'a' OR names[3] <> 'héllo' THEN check := -1; RETURN; END_IF;
        check := 1;
    END_FUNCTION

    PROGRAM P
    VAR_EXTERNAL output : DINT; END_VAR
    VAR scans : DINT; raw : ARRAY[0..7] OF BYTE; count : UDINT; END_VAR
        scans := scans + 1;
        IF scans = 1 THEN
            output := check();
        ELSE
            Wasi.Example.Lists.BYTES(size := 300000, value := raw, count => count);
            output := -100;
        END_IF;
    END_PROGRAM
"#;

/// The elements of a list of strings land in the scratch until the call
/// has copied what it keeps, and each call of an import starts it over:
/// 5,000 calls in one scan take the room of one, the memory being as large
/// as the module was built. A list longer than the room the scratch keeps
/// for what is too long traps.
#[rstest]
fn each_call_starts_the_scratch_over(mut with_db: RootDatabase) {
    let mut plc = hosted(&mut with_db, LISTS, AGAIN, lists_host);
    assert_eq!(
        plc.scan(0),
        1,
        "the number of the check that failed, negative"
    );
    let trap = plc.try_scan(0).expect_err("a list past the room traps");
    assert!(trap.contains("unreachable"), "{trap}");
}
