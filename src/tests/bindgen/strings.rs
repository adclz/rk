// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! A string that comes back: the host writes it into the STRING the call
//! passes, a `VAR_IN_OUT` of any length, or a record's into its member, a
//! STRING of 256 bytes, which is empty when no string came. One longer than
//! its STRING raises. One string comes back so, from
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
/// a text, in an error's case or as the error itself; then what is left
/// out: two strings, a string beside an error or a case that holds one or a
/// list, a string from an async call, and a list of them; and what comes
/// back without its text: two texts, on either side, and one beside a
/// char.
const TEXTS: &str = r#"
    package wasi:example@0.3.0;

    interface texts {
        record labelled { id: u32, name: string }
        enum color { red, green, blue }
        variant fault { busy, other(option<string>) }
        variant chunk { empty, bytes(list<u8>) }
        variant lettered { none, named(tuple<string, char>) }
        text: func(which: u32) -> string;
        maybe: func(which: u32) -> option<string>;
        described: func(which: u32) -> labelled;
        settled: func(which: u32) -> result<string, color>;
        failing: func(which: u32) -> result<u32, fault>;
        both: func() -> tuple<string, string>;
        why: func(which: u32) -> result<u32, string>;
        mixed: func() -> result<string, fault>;
        kinded: func() -> tuple<fault, string>;
        beside: func() -> tuple<chunk, string>;
        later: async func() -> string;
        names: func() -> list<string>;
        twice: func() -> result<fault, fault>;
        letter: func() -> lettered;
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

    			TYPE LABELLED : STRUCT
    				id : UDINT;
    				name : STRING[256];
    			END_STRUCT END_TYPE

    			TYPE COLOR : USINT (
    				RED := 0,
    				GREEN := 1,
    				BLUE := 2
    			); END_TYPE

    			/* The cases of `fault`. */
    			TYPE FAULT_KIND : USINT (
    				BUSY := 0,
    				OTHER := 1
    			); END_TYPE

    			TYPE FAULT : STRUCT
    				kind : FAULT_KIND;
    				other_is_some : BOOL;
    				other : STRING[256];
    			END_STRUCT END_TYPE

    			/* The cases of `chunk`. */
    			TYPE CHUNK_KIND : USINT (
    				EMPTY := 0,
    				BYTES := 1
    			); END_TYPE

    			TYPE CHUNK : STRUCT
    				kind : CHUNK_KIND;
    			END_STRUCT END_TYPE

    			/* The cases of `lettered`. */
    			TYPE LETTERED_KIND : USINT (
    				NONE := 0,
    				NAMED := 1
    			); END_TYPE

    			TYPE LETTERED : STRUCT
    				kind : LETTERED_KIND;
    			END_STRUCT END_TYPE

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

    			FUNCTION DESCRIBED : LABELLED
    			VAR_INPUT
    				which : UDINT;
    			END_VAR
    			VAR
    				w0 : LWORD;
    				w1 : LWORD;
    			END_VAR
    				DESCRIBED_RAW(which := which, name := DESCRIBED.name, w0 => w0, w1 => w1);
    				DESCRIBED.id := DWORD_TO_UDINT(w0.%D0);
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
    			   canonical ABI lays it out, which `FAILING` reads, and its text in
    			   `other`, where the host writes it, cut to fit. */
    			{extern 'wasi:example/texts@0.3.0' 'failing'}
    			FUNCTION PRIVATE FAILING_RAW
    			VAR_INPUT
    				which : UDINT;
    			END_VAR
    			VAR_OUTPUT
    				w0 : LWORD;
    				w1 : LWORD;
    				w2 : LWORD;
    				other : STRING[256];
    			END_VAR
    			END_FUNCTION

    			FUNCTION FAILING
    			VAR_INPUT
    				which : UDINT;
    			END_VAR
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
    				FAILING_RAW(which := which, other => error.other, w0 => w0, w1 => w1, w2 => w2);
    				value := 0;
    				error.kind := 0;
    				error.other_is_some := FALSE;
    				failed := w0.%X0;
    				IF failed THEN
    					error.kind := BYTE_TO_USINT(w0.%B4);
    					CASE error.kind OF
    						FAULT_KIND#OTHER:
    							error.other_is_some := w1.%X0;
    					END_CASE;
    				ELSE
    					value := DWORD_TO_UDINT(w0.%D1);
    				END_IF;
    			END_FUNCTION

    			/* `why` as the host gives it: its result in words of 8 bytes, as the
    			   canonical ABI lays it out, which `WHY` reads, and its text in `error`,
    			   where the host writes it, cut to fit. */
    			{extern 'wasi:example/texts@0.3.0' 'why'}
    			FUNCTION PRIVATE WHY_RAW
    			VAR_INPUT
    				which : UDINT;
    			END_VAR
    			VAR_OUTPUT
    				w0 : LWORD;
    				w1 : LWORD;
    				error : STRING[256];
    			END_VAR
    			END_FUNCTION

    			FUNCTION WHY
    			VAR_INPUT
    				which : UDINT;
    			END_VAR
    			VAR_OUTPUT
    				failed : BOOL;
    				value : UDINT;
    				error : STRING[256];
    			END_VAR
    			VAR
    				w0 : LWORD;
    				w1 : LWORD;
    			END_VAR
    				WHY_RAW(which := which, error => error, w0 => w0, w1 => w1);
    				value := 0;
    				failed := w0.%X0;
    				IF NOT failed THEN
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

    			/* `twice` as the host gives it: its result in words of 8 bytes, as the
    			   canonical ABI lays it out, which `TWICE` reads. */
    			{extern 'wasi:example/texts@0.3.0' 'twice'}
    			FUNCTION PRIVATE TWICE_RAW
    			VAR_OUTPUT
    				w0 : LWORD;
    				w1 : LWORD;
    				w2 : LWORD;
    			END_VAR
    			END_FUNCTION

    			FUNCTION TWICE
    			VAR_OUTPUT
    				failed : BOOL;
    				value : FAULT;
    				error : FAULT;
    			END_VAR
    			VAR
    				w0 : LWORD;
    				w1 : LWORD;
    				w2 : LWORD;
    			END_VAR
    				TWICE_RAW(w0 => w0, w1 => w1, w2 => w2);
    				value.kind := 0;
    				error.kind := 0;
    				failed := w0.%X0;
    				IF failed THEN
    					error.kind := BYTE_TO_USINT(w0.%B4);
    				ELSE
    					value.kind := BYTE_TO_USINT(w0.%B4);
    				END_IF;
    			END_FUNCTION

    			/* `letter` as the host gives it: its result in words of 8 bytes, as the
    			   canonical ABI lays it out, which `LETTER` reads. */
    			{extern 'wasi:example/texts@0.3.0' 'letter'}
    			FUNCTION PRIVATE LETTER_RAW
    			VAR_OUTPUT
    				w0 : LWORD;
    				w1 : LWORD;
    			END_VAR
    			END_FUNCTION

    			FUNCTION LETTER : LETTERED
    			VAR
    				w0 : LWORD;
    				w1 : LWORD;
    			END_VAR
    				LETTER_RAW(w0 => w0, w1 => w1);
    				LETTER.kind := BYTE_TO_USINT(w0.%B0);
    			END_FUNCTION
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    /* Not generated:
       - what `other` of `fault` holds: a string, which an async call cannot
         give back yet.
       - what `bytes` of `chunk` holds: a list, which an import cannot give
         back yet.
       - what `named` of `lettered` holds: a char, which an import cannot give
         back yet.
       - `both`: its result holds two strings or a string and a list, which an
         import cannot give back yet.
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
/// whose case holds a text: a short one, one of 300 bytes of digits, one
/// whose 256th byte is inside a character, none, then another case and a
/// value; `why` an error that is a text, short or of 300 bytes, then a
/// value.
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
        .func_new("failing", move |_, _, params, results| {
            let other = |text: Option<String>| {
                let text = Val::Option(text.map(|text| Box::new(Val::String(text))));
                Val::Variant("other".to_string(), Some(Box::new(text)))
            };
            let fault = match which(params) {
                0 => other(Some("disk on fire".to_string())),
                1 => other(Some("0123456789".repeat(30))),
                // `é` takes the 256th and the 257th bytes.
                2 => other(Some(format!("{}éy", "x".repeat(255)))),
                3 => other(None),
                4 => Val::Variant("busy".to_string(), None),
                _ => {
                    results[0] = Val::Result(Ok(Some(Box::new(Val::U32(42)))));
                    return Ok(());
                }
            };
            results[0] = Val::Result(Err(Some(Box::new(fault))));
            Ok(())
        })
        .expect("a function of the host");
    texts
        .func_new("why", move |_, _, params, results| {
            results[0] = Val::Result(match which(params) {
                0 => Err(Some(Box::new(string("not ready")))),
                1 => Err(Some(Box::new(Val::String("y".repeat(300))))),
                _ => Ok(Some(Box::new(Val::U32(7)))),
            });
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
        reason : STRING[256];
        first : STRING[10];
        raised, known, failed : BOOL;
        value : UDINT;
        labelled : Wasi.Example.Texts.LABELLED;
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

        // A record's string, in its STRING member.
        labelled := Wasi.Example.Texts.DESCRIBED(which := 6);
        IF labelled.id <> 7 OR labelled.name <> 'motor' THEN check := -11; RETURN; END_IF;

        Wasi.Example.Texts.SETTLED(which := 0, value := s, failed => failed, error => error);
        IF failed OR s <> 'fine' OR error <> 0 THEN check := -12; RETURN; END_IF;
        Wasi.Example.Texts.SETTLED(which := 1, value := s, failed => failed, error => error);
        IF NOT failed OR s <> '' OR error <> 1 THEN check := -13; RETURN; END_IF;

        // An error's text, in its member and in no other STRING.
        s := 'kept';
        exact := 'kept!';
        long := 'kept';
        Wasi.Example.Texts.FAILING(which := 0, failed => failed, value => value, error => fault);
        IF NOT failed OR fault.kind <> Wasi.Example.Texts.FAULT_KIND#OTHER
            OR NOT fault.other_is_some OR fault.other <> 'disk on fire'
        THEN
            check := -14; RETURN;
        END_IF;
        IF s <> 'kept' OR exact <> 'kept!' OR long <> 'kept' THEN check := -15; RETURN; END_IF;
        // One too long is cut to its first 256 bytes, and raises nothing.
        Wasi.Example.Texts.FAILING(which := 1, failed => failed, value => value, error => fault);
        first := fault.other;
        IF NOT fault.other_is_some OR bytes(fault.other) <> 256 OR first <> '0123456789' THEN
            check := -16; RETURN;
        END_IF;
        // Before the character the 256th byte is in: 255 bytes, not 256.
        Wasi.Example.Texts.FAILING(which := 2, failed => failed, value => value, error => fault);
        IF bytes(fault.other) <> 255 THEN check := -17; RETURN; END_IF;
        // No text, another case, a value: the member is empty.
        Wasi.Example.Texts.FAILING(which := 3, failed => failed, value => value, error => fault);
        IF NOT failed OR fault.kind <> Wasi.Example.Texts.FAULT_KIND#OTHER
            OR fault.other_is_some OR fault.other <> ''
        THEN
            check := -18; RETURN;
        END_IF;
        Wasi.Example.Texts.FAILING(which := 4, failed => failed, value => value, error => fault);
        IF NOT failed OR fault.kind <> Wasi.Example.Texts.FAULT_KIND#BUSY OR fault.other <> '' THEN
            check := -19; RETURN;
        END_IF;
        Wasi.Example.Texts.FAILING(which := 5, failed => failed, value => value, error => fault);
        IF failed OR value <> 42 OR fault.other <> '' THEN check := -20; RETURN; END_IF;
        // An error that is a text, in the output the FUNCTION gives it.
        Wasi.Example.Texts.WHY(which := 0, failed => failed, value => value, error => reason);
        IF NOT failed OR reason <> 'not ready' THEN check := -21; RETURN; END_IF;
        Wasi.Example.Texts.WHY(which := 1, failed => failed, value => value, error => reason);
        IF NOT failed OR bytes(reason) <> 256 THEN check := -22; RETURN; END_IF;
        Wasi.Example.Texts.WHY(which := 2, failed => failed, value => value, error => reason);
        IF failed OR value <> 7 OR reason <> '' THEN check := -23; RETURN; END_IF;
        // Bound to a shorter STRING, the text is cut to that one.
        Wasi.Example.Texts.WHY(which := 0, failed => failed, value => value, error => tiny);
        IF tiny <> 'not ' THEN check := -24; RETURN; END_IF;
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
        IF s <> 'first' THEN by_hand := -25; RETURN; END_IF;
        by_hand := 1;
    END_FUNCTION

    // What it retains goes at the start of the scratch, before the place
    // where a text too long for its STRING lands.
    PROGRAM P
    VAR_EXTERNAL output : DINT; END_VAR
    VAR RETAIN runs : DINT; END_VAR
        runs := runs + 1;
        output := check();
        IF output = 1 THEN
            output := by_hand();
        END_IF;
    END_PROGRAM
"#;

/// The host writes each string into the STRING the call passed: one that
/// fits, one of the STRING's very length, an empty one, one of several
/// bytes a character and one of 300 bytes; an option's, a result's, and a
/// record's, in the STRING member of the STRUCT the call returns. No
/// string leaves the STRING empty, and one too long raises. An error's
/// text, in its case or as the error itself, goes into its member or its
/// output and no other STRING, empty when none came, and one too long is
/// cut to fit, before the character its last byte would split. Of two
/// strings, which a declaration written by hand may let come, the first is
/// the STRING's.
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
        // A text too long for its STRING lands where the one before it did,
        // and is cut from there.
        FOR i := 1 TO 20000 DO
            Wasi.Example.Texts.FAILING(which := 1, failed => failed, value => value, error => fault);
        END_FOR;
        IF NOT failed OR fault.kind <> Wasi.Example.Texts.FAULT_KIND#OTHER THEN
            check := -2; RETURN;
        END_IF;
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
/// STRING longer than a page raises; texts too long for their STRING land
/// on one another before they are cut, so that 20,000 of them in one scan
/// take the room of one; and a string longer than the room traps, whatever
/// its STRING.
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
