// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! A string that comes back: the host writes it into the STRING the call
//! passes, a `VAR_IN_OUT` of any length, which is empty when no string
//! came. One longer than its STRING raises. One string comes back so, from
//! a call that waits for its result: the one the result holds outside its
//! error and outside a variant's cases.

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;
use wasmtime::component::{Linker, Val};

use super::{checked, hosted};
use crate::tests::cli::component_host::Host;
use crate::tests::utils::with_db;

/// A string by itself, in an option, in a record and as a result's value;
/// then what is left out: two strings, an error that is one, a string
/// beside an error or a case that holds one or a list, a string from an
/// async call, and a list of them.
const TEXTS: &str = r#"
    package wasi:example@0.3.0;

    interface texts {
        record labelled { id: u32, name: string }
        enum color { red, green, blue }
        variant fault { busy, other(option<string>) }
        variant chunk { empty, bytes(list<u8>) }
        text: func(which: u32) -> string;
        maybe: func(which: u32) -> option<string>;
        described: func(which: u32) -> labelled;
        settled: func(which: u32) -> result<string, color>;
        failing: func() -> result<u32, fault>;
        both: func() -> tuple<string, string>;
        why: func() -> result<u32, string>;
        mixed: func() -> result<string, fault>;
        kinded: func() -> tuple<fault, string>;
        beside: func() -> tuple<chunk, string>;
        later: async func() -> string;
        names: func() -> list<string>;
    }

    world example {
        import texts;
    }
"#;

#[rstest]
fn a_string_comes_back_in_the_string_the_call_passes(mut with_db: RootDatabase) {
    assert_snapshot!(checked(&mut with_db, TEXTS), @r"
    // wasi-example-texts.st
    NAMESPACE INTERNAL Wasi
    	NAMESPACE Example
    		NAMESPACE Texts
    			USING Std.Convert;

    			TYPE COLOR : USINT (
    				RED := 0,
    				GREEN := 1,
    				BLUE := 2
    			); END_TYPE

    			TYPE FAULT : USINT (
    				BUSY := 0,
    				OTHER := 1
    			); END_TYPE

    			TYPE CHUNK : USINT (
    				EMPTY := 0,
    				BYTES := 1
    			); END_TYPE

    			/* `text` as the host gives it: its result in words of 8 bytes, as the
    			   canonical ABI lays it out, which `TEXT` reads, and its string in
    			   `value`, where the host writes it. */
    			{extern 'wasi:example/texts@0.3.0' 'text'}
    			FUNCTION PRIVATE TEXT_RAW
    			VAR_INPUT
    				which : UDINT;
    			END_VAR
    			VAR_IN_OUT
    				value : STRING;
    			END_VAR
    			VAR_OUTPUT
    				w0 : LWORD;
    				w1 : LWORD;
    			END_VAR
    			END_FUNCTION

    			FUNCTION TEXT
    			VAR_INPUT
    				which : UDINT;
    			END_VAR
    			VAR_IN_OUT
    				value : STRING;
    			END_VAR
    			VAR
    				w0 : LWORD;
    				w1 : LWORD;
    			END_VAR
    				TEXT_RAW(which := which, value := value, w0 => w0, w1 => w1);
    			END_FUNCTION

    			/* `maybe` as the host gives it: its result in words of 8 bytes, as the
    			   canonical ABI lays it out, which `MAYBE` reads, and its string in
    			   `value`, where the host writes it. */
    			{extern 'wasi:example/texts@0.3.0' 'maybe'}
    			FUNCTION PRIVATE MAYBE_RAW
    			VAR_INPUT
    				which : UDINT;
    			END_VAR
    			VAR_IN_OUT
    				value : STRING;
    			END_VAR
    			VAR_OUTPUT
    				w0 : LWORD;
    				w1 : LWORD;
    			END_VAR
    			END_FUNCTION

    			FUNCTION MAYBE
    			VAR_INPUT
    				which : UDINT;
    			END_VAR
    			VAR_IN_OUT
    				value : STRING;
    			END_VAR
    			VAR_OUTPUT
    				is_some : BOOL;
    			END_VAR
    			VAR
    				w0 : LWORD;
    				w1 : LWORD;
    			END_VAR
    				MAYBE_RAW(which := which, value := value, w0 => w0, w1 => w1);
    				is_some := w0.%X0;
    			END_FUNCTION

    			/* `described` as the host gives it: its result in words of 8 bytes, as the
    			   canonical ABI lays it out, which `DESCRIBED` reads, and its string in
    			   `name`, where the host writes it. */
    			{extern 'wasi:example/texts@0.3.0' 'described'}
    			FUNCTION PRIVATE DESCRIBED_RAW
    			VAR_INPUT
    				which : UDINT;
    			END_VAR
    			VAR_IN_OUT
    				name : STRING;
    			END_VAR
    			VAR_OUTPUT
    				w0 : LWORD;
    				w1 : LWORD;
    			END_VAR
    			END_FUNCTION

    			FUNCTION DESCRIBED
    			VAR_INPUT
    				which : UDINT;
    			END_VAR
    			VAR_IN_OUT
    				name : STRING;
    			END_VAR
    			VAR_OUTPUT
    				id : UDINT;
    			END_VAR
    			VAR
    				w0 : LWORD;
    				w1 : LWORD;
    			END_VAR
    				DESCRIBED_RAW(which := which, name := name, w0 => w0, w1 => w1);
    				id := DWORD_TO_UDINT(w0.%D0);
    			END_FUNCTION

    			/* `settled` as the host gives it: its result in words of 8 bytes, as the
    			   canonical ABI lays it out, which `SETTLED` reads, and its string in
    			   `value`, where the host writes it. */
    			{extern 'wasi:example/texts@0.3.0' 'settled'}
    			FUNCTION PRIVATE SETTLED_RAW
    			VAR_INPUT
    				which : UDINT;
    			END_VAR
    			VAR_IN_OUT
    				value : STRING;
    			END_VAR
    			VAR_OUTPUT
    				w0 : LWORD;
    				w1 : LWORD;
    			END_VAR
    			END_FUNCTION

    			FUNCTION SETTLED
    			VAR_INPUT
    				which : UDINT;
    			END_VAR
    			VAR_IN_OUT
    				value : STRING;
    			END_VAR
    			VAR_OUTPUT
    				failed : BOOL;
    				error : COLOR;
    			END_VAR
    			VAR
    				w0 : LWORD;
    				w1 : LWORD;
    			END_VAR
    				SETTLED_RAW(which := which, value := value, w0 => w0, w1 => w1);
    				error := 0;
    				failed := w0.%X0;
    				IF failed THEN
    					error := BYTE_TO_USINT(w0.%B4);
    				END_IF;
    			END_FUNCTION

    			/* `failing` as the host gives it: its result in words of 8 bytes, as the
    			   canonical ABI lays it out, which `FAILING` reads. */
    			{extern 'wasi:example/texts@0.3.0' 'failing'}
    			FUNCTION PRIVATE FAILING_RAW
    			VAR_OUTPUT
    				w0 : LWORD;
    				w1 : LWORD;
    				w2 : LWORD;
    			END_VAR
    			END_FUNCTION

    			FUNCTION FAILING
    			VAR_OUTPUT
    				failed : BOOL;
    				value : UDINT;
    				error : FAULT;
    			END_VAR
    			VAR
    				w0 : LWORD;
    				w1 : LWORD;
    				w2 : LWORD;
    			END_VAR
    				FAILING_RAW(w0 => w0, w1 => w1, w2 => w2);
    				value := 0;
    				error := 0;
    				failed := w0.%X0;
    				IF failed THEN
    					error := BYTE_TO_USINT(w0.%B4);
    				ELSE
    					value := DWORD_TO_UDINT(w0.%D1);
    				END_IF;
    			END_FUNCTION

    			/* `names` as the host gives it: its result in words of 8 bytes, as the
    			   canonical ABI lays it out, which `NAMES` reads, and its list in `value`,
    			   where the host writes it. */
    			{extern 'wasi:example/texts@0.3.0' 'names'}
    			FUNCTION PRIVATE NAMES_RAW
    			VAR_IN_OUT
    				value : ARRAY[*] OF STRING;
    			END_VAR
    			VAR_OUTPUT
    				w0 : LWORD;
    				w1 : LWORD;
    			END_VAR
    			END_FUNCTION

    			FUNCTION NAMES
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
    				NAMES_RAW(value := value, w0 => w0, w1 => w1);
    				count := DWORD_TO_UDINT(w0.%D1);
    			END_FUNCTION
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    /* Not generated:
       - what `other` of `fault` holds: a string, which an import cannot give
         back yet.
       - what `bytes` of `chunk` holds: a list, which an import cannot give
         back yet.
       - `both`: its result holds two strings or a string and a list, which an
         import cannot give back yet.
       - `why`: its result holds a string, which an import cannot give back
         yet.
       - `mixed`: its result holds two strings or a string and a list, which an
         import cannot give back yet.
       - `kinded`: its result holds two strings or a string and a list, which
         an import cannot give back yet.
       - `beside`: its result holds two strings or a string and a list, which
         an import cannot give back yet.
       - `later`: its result holds a string, which an import cannot give back
         yet. */
    ");
}

/// The host's side of [`TEXTS`]: `text` gives a short string, an empty
/// one, one of several bytes a character, one of 300 bytes, and for 10 and
/// 11 ones of 100,010 and 300,000; `maybe` one or none; `described` a
/// number and a name; `settled` a string or an error; `failing` an error
/// whose case holds a text no output takes.
fn texts_host(linker: &mut Linker<Host>) {
    let which = |params: &[Val]| match params {
        [Val::U32(which)] => *which,
        other => panic!("`which`, a u32: {other:?}"),
    };
    let string = |text: &str| Val::String(text.to_string());
    let mut texts = linker
        .instance("wasi:example/texts@0.3.0")
        .expect("the interface");
    texts
        .func_new("text", move |_, _, params, results| {
            results[0] = match which(params) {
                0 => string("hello"),
                1 => string(""),
                2 => string("héllo, 漢字"),
                10 => Val::String("x".repeat(100_010)),
                11 => Val::String("x".repeat(300_000)),
                _ => Val::String("x".repeat(300)),
            };
            Ok(())
        })
        .expect("a function of the host");
    texts
        .func_new("maybe", move |_, _, params, results| {
            results[0] = Val::Option(match which(params) {
                0 => None,
                _ => Some(Box::new(string("found"))),
            });
            Ok(())
        })
        .expect("a function of the host");
    texts
        .func_new("described", move |_, _, params, results| {
            results[0] = Val::Record(vec![
                ("id".to_string(), Val::U32(which(params) + 1)),
                ("name".to_string(), string("motor")),
            ]);
            Ok(())
        })
        .expect("a function of the host");
    texts
        .func_new("settled", move |_, _, params, results| {
            results[0] = Val::Result(match which(params) {
                0 => Ok(Some(Box::new(string("fine")))),
                _ => Err(Some(Box::new(Val::Enum("green".to_string())))),
            });
            Ok(())
        })
        .expect("a function of the host");
    texts
        .func_new("both", move |_, _, _, results| {
            results[0] = Val::Tuple(vec![string("first"), string("second")]);
            Ok(())
        })
        .expect("a function of the host");
    texts
        .func_new("failing", move |_, _, _, results| {
            let text = Val::Option(Some(Box::new(string("a text that no output takes"))));
            results[0] = Val::Result(Err(Some(Box::new(Val::Variant(
                "other".to_string(),
                Some(Box::new(text)),
            )))));
            Ok(())
        })
        .expect("a function of the host");
}

/// What the host of [`TEXTS`] gives, read: 1 when each string is in its
/// STRING and nowhere else, and otherwise the number of the check that
/// failed, negative.
const READ: &str = r#"
    FUNCTION bytes : UDINT
    VAR_INPUT
        IN : STRING[300];
    END_VAR
        {wasm 'str.byte_len' (params IN) (result bytes)}
    END_FUNCTION

    FUNCTION check : DINT
    VAR
        s, why : STRING;
        exact : STRING[5];
        tiny : STRING[4];
        long : STRING[300];
        raised, known, failed : BOOL;
        id, value : UDINT;
        error : Wasi.Example.Texts.COLOR;
        fault : Wasi.Example.Texts.FAULT;
    END_VAR
        check := 0;
        Wasi.Example.Texts.TEXT(which := 0, value := s);
        IF s <> 'hello' THEN check := -1; RETURN; END_IF;
        // A STRING that holds the string and no more.
        Wasi.Example.Texts.TEXT(which := 0, value := exact);
        IF exact <> 'hello' THEN check := -2; RETURN; END_IF;
        // An empty string empties the STRING.
        Wasi.Example.Texts.TEXT(which := 1, value := s);
        IF s <> '' THEN check := -3; RETURN; END_IF;
        Wasi.Example.Texts.TEXT(which := 2, value := s);
        IF s <> 'héllo, 漢字' OR bytes(s) <> 14 THEN check := -4; RETURN; END_IF;
        Wasi.Example.Texts.TEXT(which := 3, value := long);
        IF bytes(long) <> 300 THEN check := -5; RETURN; END_IF;

        // A string longer than its STRING raises, and leaves it empty.
        tiny := 'old';
        __TRY
            Wasi.Example.Texts.TEXT(which := 0, value := tiny);
        __CATCH(why)
            raised := TRUE;
        __END_TRY;
        IF NOT raised OR why <> 'a string the host gave is longer than its STRING' THEN
            check := -6; RETURN;
        END_IF;
        IF tiny <> '' THEN check := -7; RETURN; END_IF;
        // The call after it is one like any other.
        Wasi.Example.Texts.TEXT(which := 0, value := s);
        IF s <> 'hello' THEN check := -8; RETURN; END_IF;

        Wasi.Example.Texts.MAYBE(which := 1, value := s, is_some => known);
        IF NOT known OR s <> 'found' THEN check := -9; RETURN; END_IF;
        // None came: the STRING is empty, not what the call before left.
        Wasi.Example.Texts.MAYBE(which := 0, value := s, is_some => known);
        IF known OR s <> '' THEN check := -10; RETURN; END_IF;

        Wasi.Example.Texts.DESCRIBED(which := 6, name := s, id => id);
        IF id <> 7 OR s <> 'motor' THEN check := -11; RETURN; END_IF;

        Wasi.Example.Texts.SETTLED(which := 0, value := s, failed => failed, error => error);
        IF failed OR s <> 'fine' OR error <> 0 THEN check := -12; RETURN; END_IF;
        Wasi.Example.Texts.SETTLED(which := 1, value := s, failed => failed, error => error);
        IF NOT failed OR s <> '' OR error <> 1 THEN check := -13; RETURN; END_IF;

        // A text the host gives that no STRING waits for lands in none.
        s := 'kept';
        exact := 'kept!';
        long := 'kept';
        Wasi.Example.Texts.FAILING(failed => failed, value => value, error => fault);
        IF NOT failed OR fault <> 1 THEN check := -14; RETURN; END_IF;
        IF s <> 'kept' OR exact <> 'kept!' OR long <> 'kept' THEN check := -15; RETURN; END_IF;
        check := 1;
    END_FUNCTION

    /* `both` declared by hand, which the generator leaves out: the host's
       first string goes into the STRING, its second where no STRING is. */
    {extern 'wasi:example/texts@0.3.0' 'both'}
    FUNCTION BOTH_BY_HAND
    VAR_IN_OUT
        first : STRING;
    END_VAR
    VAR_OUTPUT
        w0 : LWORD;
        w1 : LWORD;
    END_VAR
    END_FUNCTION

    FUNCTION by_hand : DINT
    VAR
        s : STRING;
    END_VAR
        BOTH_BY_HAND(first := s);
        IF s <> 'first' THEN by_hand := -16; RETURN; END_IF;
        by_hand := 1;
    END_FUNCTION

    PROGRAM P
    VAR_EXTERNAL output : DINT; END_VAR
        output := check();
        IF output = 1 THEN
            output := by_hand();
        END_IF;
    END_PROGRAM
"#;

/// The host writes each string into the STRING the call passed: one that
/// fits, one of the STRING's very length, an empty one, one of several
/// bytes a character and one of 300 bytes; an option's, a record's and a
/// result's. No string leaves the STRING empty, one too long raises, and a
/// text the host gives outside such a call is written into no STRING. Of
/// two strings, which a declaration written by hand may let come, the
/// first is the STRING's.
#[rstest]
fn the_host_writes_a_string_into_the_string_passed(mut with_db: RootDatabase) {
    let mut plc = hosted(&mut with_db, TEXTS, READ, texts_host);
    assert_eq!(
        plc.scan(0),
        1,
        "the number of the check that failed, negative"
    );
}

/// What the scan of [`room_for_what_is_too_long`] checks: 1, or the number
/// of the check that failed, negative. Its second scan asks for a string
/// longer than any place the module has.
const BOUNDS: &str = r#"
    FUNCTION check : DINT
    VAR
        big : STRING[100000];
        why : STRING;
        raised, failed : BOOL;
        value : UDINT;
        fault : Wasi.Example.Texts.FAULT;
        i : DINT;
    END_VAR
        check := 0;
        // Ten bytes too long for a STRING longer than a page: it raises.
        __TRY
            Wasi.Example.Texts.TEXT(which := 10, value := big);
        __CATCH(why)
            raised := TRUE;
        __END_TRY;
        IF NOT raised OR why <> 'a string the host gave is longer than its STRING' THEN
            check := -1; RETURN;
        END_IF;
        // A text no STRING waits for lands where the one before it did.
        FOR i := 1 TO 20000 DO
            Wasi.Example.Texts.FAILING(failed => failed, value => value, error => fault);
        END_FOR;
        IF NOT failed OR fault <> 1 THEN check := -2; RETURN; END_IF;
        check := 1;
    END_FUNCTION

    PROGRAM P
    VAR_EXTERNAL output : DINT; END_VAR
    VAR scans : DINT; s : STRING; END_VAR
        scans := scans + 1;
        IF scans = 1 THEN
            output := check();
        ELSE
            Wasi.Example.Texts.TEXT(which := 11, value := s);
            output := -100;
        END_IF;
    END_PROGRAM
"#;

/// The memory is as large as the module was built: what is too long for
/// its STRING lands in the scratch, which has room for the largest STRING
/// the module declares and a page more. A string ten bytes too long for a
/// STRING longer than a page raises; texts no STRING waits for land on one
/// another, so that 20,000 of them in one scan take the room of one; and a
/// string longer than the room traps, whatever its STRING.
#[rstest]
fn room_for_what_is_too_long(mut with_db: RootDatabase) {
    let mut plc = hosted(&mut with_db, TEXTS, BOUNDS, texts_host);
    assert_eq!(
        plc.scan(0),
        1,
        "the number of the check that failed, negative"
    );
    let trap = plc.try_scan(0).expect_err("a string past the room traps");
    assert!(trap.contains("unreachable"), "{trap}");
}
