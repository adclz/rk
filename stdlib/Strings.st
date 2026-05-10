/*
Standard String Functions
*/
NAMESPACE Std.Strings

	/* ## LEN — length in bytes

	   Constant-time: STRINGs are stored as `(ptr, len)`, this just returns
	   the second word. Always succeeds. */
	FUNCTION LEN: UDINT
		VAR_INPUT
			IN: STRING;
		END_VAR
		{wasm 'str.byte_len' (params IN) (result LEN)}
	END_FUNCTION

	/* ## IS_UTF8 — UTF-8 validation

	   Returns TRUE iff the buffer is well-formed UTF-8. Useful when
	   accepting strings from external sources (host imports, MODBUS, etc.)
	   before passing them to slicing operations that may otherwise split a
	   multi-byte sequence. */
	FUNCTION IS_UTF8: BOOL
		VAR_INPUT
			IN: STRING;
		END_VAR
		{wasm 'str.is_utf8' (params IN) (result IS_UTF8)}
	END_FUNCTION

	/* ## FIND — 1-indexed byte position of NEEDLE in HAYSTACK

	   Returns 0 when not found. An empty NEEDLE returns 1 (consistent
	   with IEC 61131-3 FIND semantics). To search for a single CHAR,
	   wrap it with `Std.Convert.CHAR_TO_STRING(c)`. */
	FUNCTION FIND: UDINT
		VAR_INPUT
			HAYSTACK: STRING;
			NEEDLE: STRING;
		END_VAR
		{wasm 'str.byte_find' (params HAYSTACK NEEDLE) (result FIND)}
	END_FUNCTION

	/* ## Producers — return a fresh STRING

	   These functions take their inputs as `VAR_INPUT STRING` (flattened
	   to `(ptr, len)` at the WASM ABI) and return STRING (flattened to
	   `(ptr, len)`). Each producer writes into its own static return
	   slot in linear memory; the codegen snapshots the result into a
	   per-call-site scratch slot when the value is consumed as a
	   `ByValue` arg of another call, so nested invocations don't clobber
	   each other. Truncation is silent on overflow. Slicing operates on
	   raw bytes — splitting a multi-byte UTF-8 sequence yields invalid
	   UTF-8.
	*/

	/* ## CONCAT — append B after A, truncate to result capacity.
	   To concatenate a CHAR, wrap it with `Std.Convert.CHAR_TO_STRING(c)`. */
	FUNCTION CONCAT: STRING
		VAR_INPUT
			A: STRING;
			B: STRING;
		END_VAR
		{wasm 'str.concat' (params A B) (result CONCAT)}
	END_FUNCTION

	/* ## LEFT — first N bytes of S */
	FUNCTION LEFT: STRING
		VAR_INPUT
			S: STRING;
			N: UDINT;
		END_VAR
		{wasm 'str.byte_left' (params S N) (result LEFT)}
	END_FUNCTION

	/* ## RIGHT — last N bytes of S */
	FUNCTION RIGHT: STRING
		VAR_INPUT
			S: STRING;
			N: UDINT;
		END_VAR
		{wasm 'str.byte_right' (params S N) (result RIGHT)}
	END_FUNCTION

	/* ## MID — N bytes starting at 1-indexed byte position P */
	FUNCTION MID: STRING
		VAR_INPUT
			S: STRING;
			N: UDINT;
			P: UDINT;
		END_VAR
		{wasm 'str.byte_mid' (params S N P) (result MID)}
	END_FUNCTION

	/* ## INSERT — insert INS into S after byte position P (0 = front).
	   To insert a single CHAR, wrap it with `Std.Convert.CHAR_TO_STRING(c)`. */
	FUNCTION INSERT: STRING
		VAR_INPUT
			S: STRING;
			INS: STRING;
			P: UDINT;
		END_VAR
		{wasm 'str.byte_insert' (params S INS P) (result INSERT)}
	END_FUNCTION

	/* ## DELETE — remove N bytes starting at 1-indexed position P */
	FUNCTION DELETE: STRING
		VAR_INPUT
			S: STRING;
			N: UDINT;
			P: UDINT;
		END_VAR
		{wasm 'str.byte_delete' (params S N P) (result DELETE)}
	END_FUNCTION

	/* ## REPLACE — replace N bytes at position P with INS.
	   To replace with a single CHAR, wrap it with `Std.Convert.CHAR_TO_STRING(c)`. */
	FUNCTION REPLACE: STRING
		VAR_INPUT
			S: STRING;
			INS: STRING;
			N: UDINT;
			P: UDINT;
		END_VAR
		{wasm 'str.byte_replace' (params S INS N P) (result REPLACE)}
	END_FUNCTION

	/* ## Tests */

	NAMESPACE Test
		USING Std.Unit
		USING Std.Convert

		{test}
		FUNCTION test_len_ascii
			ASSERT(LEN('hello') = 5);
		END_FUNCTION

		{test}
		FUNCTION test_len_empty
			ASSERT(LEN('') = 0);
		END_FUNCTION

		{test}
		FUNCTION test_is_utf8_ascii
			ASSERT(IS_UTF8('hello'));
		END_FUNCTION

		{test}
		FUNCTION test_is_utf8_empty
			ASSERT(IS_UTF8(''));
		END_FUNCTION

		{test}
		FUNCTION test_find_present
			ASSERT(FIND('hello world', 'world') = 7);
		END_FUNCTION

		{test}
		FUNCTION test_find_at_start
			ASSERT(FIND('hello world', 'hello') = 1);
		END_FUNCTION

		{test}
		FUNCTION test_find_missing
			ASSERT(FIND('hello world', 'xyz') = 0);
		END_FUNCTION

		{test}
		FUNCTION test_find_empty_needle
			ASSERT(FIND('hello', '') = 1);
		END_FUNCTION

		{test}
		FUNCTION test_find_needle_longer_than_haystack
			ASSERT(FIND('hi', 'hello') = 0);
		END_FUNCTION

		{test}
		FUNCTION test_concat_basic
			VAR result: STRING; END_VAR
			result := CONCAT('hello ', 'world');
			ASSERT(LEN(result) = 11);
			ASSERT(FIND(result, 'hello world') = 1);
		END_FUNCTION

		{test}
		FUNCTION test_concat_empty_left
			VAR result: STRING; END_VAR
			result := CONCAT('', 'world');
			ASSERT(LEN(result) = 5);
		END_FUNCTION

		{test}
		FUNCTION test_concat_empty_right
			VAR result: STRING; END_VAR
			result := CONCAT('hello', '');
			ASSERT(LEN(result) = 5);
		END_FUNCTION

		{test}
		FUNCTION test_left_basic
			VAR result: STRING; END_VAR
			result := LEFT('hello world', 5);
			ASSERT(LEN(result) = 5);
			ASSERT(FIND(result, 'hello') = 1);
		END_FUNCTION

		{test}
		FUNCTION test_left_n_exceeds_length
			VAR result: STRING; END_VAR
			result := LEFT('hi', 10);
			ASSERT(LEN(result) = 2);
		END_FUNCTION

		{test}
		FUNCTION test_right_basic
			VAR result: STRING; END_VAR
			result := RIGHT('hello world', 5);
			ASSERT(LEN(result) = 5);
			ASSERT(FIND(result, 'world') = 1);
		END_FUNCTION

		{test}
		FUNCTION test_mid_basic
			VAR result: STRING; END_VAR
			// 'world' starts at byte 7, 5 chars
			result := MID('hello world', 5, 7);
			ASSERT(LEN(result) = 5);
			ASSERT(FIND(result, 'world') = 1);
		END_FUNCTION

		{test}
		FUNCTION test_mid_p_zero
			VAR result: STRING; END_VAR
			result := MID('hello', 3, 0);
			ASSERT(LEN(result) = 0);
		END_FUNCTION

		{test}
		FUNCTION test_insert_middle
			VAR result: STRING; END_VAR
			// Insert 'XX' after byte 5 of 'helloworld' → 'helloXXworld'
			result := INSERT('helloworld', 'XX', 5);
			ASSERT(LEN(result) = 12);
			ASSERT(FIND(result, 'helloXXworld') = 1);
		END_FUNCTION

		{test}
		FUNCTION test_insert_front
			VAR result: STRING; END_VAR
			result := INSERT('world', 'hello ', 0);
			ASSERT(LEN(result) = 11);
			ASSERT(FIND(result, 'hello world') = 1);
		END_FUNCTION

		{test}
		FUNCTION test_delete_basic
			VAR result: STRING; END_VAR
			// Delete 6 bytes starting at byte 6 of 'hello world' → 'hello'
			result := DELETE('hello world', 6, 6);
			ASSERT(LEN(result) = 5);
			ASSERT(FIND(result, 'hello') = 1);
		END_FUNCTION

		{test}
		FUNCTION test_replace_basic
			VAR result: STRING; END_VAR
			// Replace 5 bytes at byte 7 of 'hello world' with 'there'
			result := REPLACE('hello world', 'there', 5, 7);
			ASSERT(LEN(result) = 11);
			ASSERT(FIND(result, 'hello there') = 1);
		END_FUNCTION

		{test}
		FUNCTION test_concat_nested_binary
			// Independent inner CONCATs feeding an outer CONCAT — each
			// inner result must survive long enough for the outer to read
			// it. With per-call-site scratch slots: 'xxyyzzww'.
			VAR result: STRING; END_VAR
			result := CONCAT(CONCAT('xx', 'yy'), CONCAT('zz', 'ww'));
			ASSERT(LEN(result) = 8);
			ASSERT(FIND(result, 'xxyyzzww') = 1);
		END_FUNCTION

		{test}
		FUNCTION test_concat_nested_left_arg
			// Inner result used as outer's first arg.
			VAR result: STRING; END_VAR
			result := CONCAT(CONCAT('ab', 'cd'), 'ef');
			ASSERT(LEN(result) = 6);
			ASSERT(FIND(result, 'abcdef') = 1);
		END_FUNCTION

		{test}
		FUNCTION test_left_inside_concat
			VAR result: STRING; END_VAR
			result := CONCAT(LEFT('hello world', 5), '!');
			ASSERT(LEN(result) = 6);
			ASSERT(FIND(result, 'hello!') = 1);
		END_FUNCTION

		/* CHAR-as-string usage — explicit conversion via
		   `Std.Convert.CHAR_TO_STRING`. CHAR is an i32 code point at
		   the WASM ABI; CONCAT/INSERT/REPLACE/FIND all take STRING args,
		   so users wrap CHAR values explicitly. */

		{test}
		FUNCTION test_concat_char_literal_right
			VAR result: STRING; END_VAR
			result := CONCAT('hello', CHAR_TO_STRING(CHAR#'!'));
			ASSERT(LEN(result) = 6);
			ASSERT(FIND(result, 'hello!') = 1);
		END_FUNCTION

		{test}
		FUNCTION test_concat_char_literal_left
			VAR result: STRING; END_VAR
			result := CONCAT(CHAR_TO_STRING(CHAR#'>'), ' message');
			ASSERT(LEN(result) = 9);
			ASSERT(FIND(result, '> message') = 1);
		END_FUNCTION

		{test}
		FUNCTION test_find_char_literal_needle
			ASSERT(FIND('hello', CHAR_TO_STRING(CHAR#'l')) = 3);
		END_FUNCTION

		{test}
		FUNCTION test_insert_char_literal
			VAR result: STRING; END_VAR
			result := INSERT('helloworld', CHAR_TO_STRING(CHAR#' '), 5);
			ASSERT(LEN(result) = 11);
			ASSERT(FIND(result, 'hello world') = 1);
		END_FUNCTION

		{test}
		FUNCTION test_replace_char_literal
			VAR result: STRING; END_VAR
			// Replace 5 bytes at byte 7 of 'hello world' with the single CHAR '!'.
			result := REPLACE('hello world', CHAR_TO_STRING(CHAR#'!'), 5, 7);
			ASSERT(LEN(result) = 7);
			ASSERT(FIND(result, 'hello !') = 1);
		END_FUNCTION

		{test}
		FUNCTION test_concat_char_var
			VAR
				bang: CHAR := CHAR#'!';
				result: STRING;
			END_VAR
			result := CONCAT('hello', CHAR_TO_STRING(bang));
			ASSERT(LEN(result) = 6);
			ASSERT(FIND(result, 'hello!') = 1);
		END_FUNCTION

		{test}
		FUNCTION test_find_char_var
			VAR
				needle: CHAR := CHAR#'l';
			END_VAR
			ASSERT(FIND('hello', CHAR_TO_STRING(needle)) = 3);
		END_FUNCTION

		{test}
		FUNCTION test_concat_char_var_left
			VAR
				prefix: CHAR := CHAR#'>';
				result: STRING;
			END_VAR
			result := CONCAT(CHAR_TO_STRING(prefix), ' message');
			ASSERT(LEN(result) = 9);
			ASSERT(FIND(result, '> message') = 1);
		END_FUNCTION

		{test}
		FUNCTION test_concat_two_char_vars
			// Two nested CHAR_TO_STRING calls — each must get its own
			// per-call-site scratch slot, the second mustn't clobber the
			// first before the outer CONCAT reads them.
			VAR
				a: CHAR := CHAR#'a';
				b: CHAR := CHAR#'b';
				result: STRING;
			END_VAR
			result := CONCAT(CHAR_TO_STRING(a), CHAR_TO_STRING(b));
			ASSERT(LEN(result) = 2);
			ASSERT(FIND(result, 'ab') = 1);
		END_FUNCTION

	END_NAMESPACE

END_NAMESPACE
