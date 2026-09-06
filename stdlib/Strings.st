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

	/* ## Character-aware functions

	   `LEN`, `FIND`, `LEFT`, `RIGHT`, `MID`, `INSERT`, `DELETE` and `REPLACE`
	   count bytes, and a byte position can fall inside a multi-byte
	   character. The `CHAR_` twins count characters: one well-formed UTF-8
	   sequence is a character, and a stray byte that is not part of one is a
	   character on its own, worth its Latin-1 code point like `CHAR#'$E9'`.
	   Positions are 1-based in characters. On ASCII text both families
	   agree; the byte family is the faster one. */

	/* ## CHAR_COUNT — number of characters in IN. `CHAR_COUNT('café')` is 4
	   where `LEN('café')` is 5. */
	FUNCTION CHAR_COUNT: UDINT
		VAR_INPUT
			IN: STRING;
		END_VAR
		{wasm 'str.char_count' (params IN) (result CHAR_COUNT)}
	END_FUNCTION

	/* ## CHAR_AT — the character at 1-indexed position P, as a CHAR.
	   `CHAR#'$00'` when P is 0 or past the end. */
	FUNCTION CHAR_AT: CHAR
		VAR_INPUT
			IN: STRING;
			P: UDINT;
		END_VAR
		{wasm 'str.char_at' (params IN P) (result CHAR_AT)}
	END_FUNCTION

	/* ## CHAR_FIND — 1-indexed character position of NEEDLE in HAYSTACK.
	   0 when not found, 1 for an empty NEEDLE, as FIND. */
	FUNCTION CHAR_FIND: UDINT
		VAR_INPUT
			HAYSTACK: STRING;
			NEEDLE: STRING;
		END_VAR
		{wasm 'str.char_find' (params HAYSTACK NEEDLE) (result CHAR_FIND)}
	END_FUNCTION

	/* ## CHAR_LEFT — first N characters of S */
	FUNCTION CHAR_LEFT: STRING
		VAR_INPUT
			S: STRING;
			N: UDINT;
		END_VAR
		{wasm 'str.char_left' (params S N) (result CHAR_LEFT)}
	END_FUNCTION

	/* ## CHAR_RIGHT — last N characters of S */
	FUNCTION CHAR_RIGHT: STRING
		VAR_INPUT
			S: STRING;
			N: UDINT;
		END_VAR
		{wasm 'str.char_right' (params S N) (result CHAR_RIGHT)}
	END_FUNCTION

	/* ## CHAR_MID — N characters starting at 1-indexed character position P.
	   Empty when P is 0 or past the end. */
	FUNCTION CHAR_MID: STRING
		VAR_INPUT
			S: STRING;
			N: UDINT;
			P: UDINT;
		END_VAR
		{wasm 'str.char_mid' (params S N P) (result CHAR_MID)}
	END_FUNCTION

	/* ## CHAR_INSERT — insert INS into S after character position P (0 = front) */
	FUNCTION CHAR_INSERT: STRING
		VAR_INPUT
			S: STRING;
			INS: STRING;
			P: UDINT;
		END_VAR
		{wasm 'str.char_insert' (params S INS P) (result CHAR_INSERT)}
	END_FUNCTION

	/* ## CHAR_DELETE — remove N characters starting at character position P.
	   S unchanged when P is 0 or past the end. */
	FUNCTION CHAR_DELETE: STRING
		VAR_INPUT
			S: STRING;
			N: UDINT;
			P: UDINT;
		END_VAR
		{wasm 'str.char_delete' (params S N P) (result CHAR_DELETE)}
	END_FUNCTION

	/* ## CHAR_REPLACE — replace N characters at character position P with INS.
	   S unchanged when P is 0 or past the end. */
	FUNCTION CHAR_REPLACE: STRING
		VAR_INPUT
			S: STRING;
			INS: STRING;
			N: UDINT;
			P: UDINT;
		END_VAR
		{wasm 'str.char_replace' (params S INS N P) (result CHAR_REPLACE)}
	END_FUNCTION

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
		FUNCTION test_len_counts_utf8_bytes
			ASSERT(LEN('café') = 5 AND LEN('€') = 3 AND LEN('中文') = 6);
			ASSERT(IS_UTF8('café') AND IS_UTF8('中文'));
			ASSERT(LEN('$E9') = 1 AND NOT IS_UTF8('$E9'));
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

		// --- return-slot aliasing: results copied into locals must survive
		// LATER producer calls (the dangerous scenario of a shared static
		// return slot), and self-argument accumulation must not corrupt ---

		{test}
		FUNCTION test_result_survives_later_producer_call
			VAR
				first, second: STRING;
			END_VAR
			first := CONCAT('aa', 'bb');
			second := CONCAT('cc', 'dd');
			ASSERT(FIND(first, 'aabb') = 1, 'earlier result must survive a later producer call');
			ASSERT(FIND(second, 'ccdd') = 1, 'later result is correct too');
		END_FUNCTION

		{test}
		FUNCTION test_concat_self_accumulation
			VAR
				s: STRING;
				i: INT;
			END_VAR
			s := 'x';
			FOR i := 1 TO 4 DO
				s := CONCAT(s, 'y');
			END_FOR;
			ASSERT(LEN(s) = 5, 'x + 4 accumulated y');
			ASSERT(FIND(s, 'xyyyy') = 1, 'self-argument CONCAT accumulates correctly');
		END_FUNCTION

		// --- slicer bounds: N/P beyond the string must clamp, not underflow ---

		{test}
		FUNCTION test_right_n_beyond_length
			VAR
				s: STRING;
			END_VAR
			// RIGHT computes len-N internally: N > len must clamp, not
			// underflow to a huge unsigned offset.
			s := RIGHT('hi', 10);
			ASSERT(LEN(s) = 2, 'RIGHT with N beyond length returns the whole string');
			ASSERT(FIND(s, 'hi') = 1, 'content intact');
		END_FUNCTION

		{test}
		FUNCTION test_mid_beyond_length
			VAR
				s: STRING;
			END_VAR
			s := MID('hello', 10, 3);
			ASSERT(LEN(s) = 3, 'MID clamps L to the remaining chars');
			s := MID('hello', 3, 10);
			ASSERT(LEN(s) = 0, 'MID with P beyond length yields empty');
		END_FUNCTION

		{test}
		FUNCTION test_delete_beyond_length
			VAR
				s: STRING;
			END_VAR
			// Signature is DELETE(S, N, P): delete N chars at position P.
			s := DELETE('hello', 3, 10);
			ASSERT(LEN(s) = 5, 'DELETE at P beyond the end removes nothing');
			s := DELETE('hello', 10, 3);
			ASSERT(LEN(s) = 2, 'DELETE clamps N to the remaining chars (keeps "he")');
			ASSERT(FIND(s, 'he') = 1, 'prefix before P survives');
		END_FUNCTION

		{test}
		FUNCTION test_find_no_match_returns_zero
			ASSERT(FIND('hello', 'xyz') = 0, 'FIND with no match returns 0');
			ASSERT(FIND('', 'a') = 0, 'FIND in empty string returns 0');
		END_FUNCTION

		{test}
		FUNCTION test_len_zero_slices
			VAR
				s: STRING;
			END_VAR
			s := LEFT('hello', 0);
			ASSERT(LEN(s) = 0, 'LEFT L=0 is empty');
			s := MID('hello', 0, 2);
			ASSERT(LEN(s) = 0, 'MID L=0 is empty');
		END_FUNCTION

		// --- STRING comparison operators: =, <>, and lexicographic ordering
		// (byte-wise with length tiebreak). These replace the LEN+FIND idiom
		// for content assertions. ---

		{test}
		FUNCTION test_string_equality_operators
			VAR
				s: STRING := 'START';
			END_VAR
			ASSERT(s = 'START', 'var = literal');
			ASSERT('START' = s, 'literal = var');
			ASSERT(s <> 'STOP', 'not-equal');
			ASSERT_EQ(CONCAT('STA', 'RT'), 'START', 'ASSERT_EQ on a producer result');
			ASSERT_NEQ(CONCAT('a', 'b'), 'ba', 'ASSERT_NEQ on unequal content');
		END_FUNCTION

		{test}
		FUNCTION test_string_ordering_operators
			ASSERT('abc' < 'abd', 'lexicographic less');
			ASSERT('ab' < 'abc', 'shared prefix: shorter is smaller');
			ASSERT('abd' > 'abc', 'greater');
			ASSERT('abc' <= 'abc', 'le on equal');
			ASSERT('abc' >= 'abc', 'ge on equal');
			ASSERT(NOT ('abc' < 'abc'), 'not strictly less than itself');
			ASSERT('' < 'a', 'empty sorts first');
		END_FUNCTION

		{test}
		FUNCTION test_string_equality_after_mutation
			VAR
				s: STRING := 'x';
				i: INT;
			END_VAR
			FOR i := 1 TO 3 DO
				s := CONCAT(s, 'y');
			END_FOR;
			ASSERT_EQ(s, 'xyyy', 'accumulated content compares whole');
		END_FUNCTION

		// Both operands are producer calls — each needs its own scratch
		// buffer. If they shared the one static return slot, the second call
		// would clobber the first and every such comparison would degenerate
		// to comparing a value with itself (silently always equal).
		{test}
		FUNCTION test_string_comparison_between_two_producers
			ASSERT(CONCAT('ab', 'cd') = CONCAT('ab', 'cd'), 'equal producer results compare equal');
			ASSERT(CONCAT('ab', 'cd') <> CONCAT('xy', 'zw'), 'different producer results must not be equal');
			ASSERT(CONCAT('a', 'a') < CONCAT('a', 'b'), 'ordering holds across two producers');
		END_FUNCTION

		// ----- Character-aware twins --------------------------------------
		{test}
		FUNCTION test_char_count_agrees_with_len_on_ascii
			ASSERT(CHAR_COUNT('hello') = 5 AND CHAR_COUNT('') = 0);
			ASSERT(CHAR_COUNT('hello') = LEN('hello'));
		END_FUNCTION

		{test}
		FUNCTION test_char_count_counts_characters_not_bytes
			ASSERT(CHAR_COUNT('café') = 4 AND LEN('café') = 5);
			ASSERT(CHAR_COUNT('日本語') = 3 AND LEN('日本語') = 9);
			ASSERT(CHAR_COUNT('$E9$E9') = 2, 'a stray byte is one character');
		END_FUNCTION

		{test}
		FUNCTION test_char_at_returns_the_character
			ASSERT(CHAR_AT('café', 4) = CHAR#'é' AND CHAR_AT('café', 1) = CHAR#'c');
			ASSERT(CHAR_AT('日本語', 2) = CHAR#'本');
			ASSERT(CHAR_AT('$E9', 1) = CHAR#'é', 'a stray byte reads as its Latin-1 character');
			ASSERT(CHAR_AT('abc', 0) = CHAR#'$00' AND CHAR_AT('abc', 4) = CHAR#'$00');
		END_FUNCTION

		{test}
		FUNCTION test_char_find_counts_characters
			ASSERT(CHAR_FIND('日本語', '語') = 3 AND FIND('日本語', '語') = 7);
			ASSERT(CHAR_FIND('café!', '!') = 5);
			ASSERT(CHAR_FIND('abc', 'x') = 0 AND CHAR_FIND('abc', '') = 1);
		END_FUNCTION

		{test}
		FUNCTION test_char_left_and_right_never_split_a_character
			ASSERT(CHAR_LEFT('café', 4) = 'café' AND CHAR_LEFT('café', 3) = 'caf');
			ASSERT(CHAR_RIGHT('café', 1) = 'é' AND CHAR_RIGHT('日本語', 2) = '本語');
			ASSERT(CHAR_LEFT('café', 10) = 'café' AND CHAR_RIGHT('café', 0) = '');
		END_FUNCTION

		{test}
		FUNCTION test_char_mid_counts_characters
			ASSERT(CHAR_MID('日本語', 1, 2) = '本' AND CHAR_MID('café', 2, 3) = 'fé');
			ASSERT(CHAR_MID('café', 10, 4) = 'é', 'N past the end is clamped');
			ASSERT(CHAR_MID('café', 1, 0) = '' AND CHAR_MID('café', 1, 5) = '');
		END_FUNCTION

		{test}
		FUNCTION test_char_insert_delete_replace_count_characters
			ASSERT(CHAR_INSERT('caf', 'é', 3) = 'café' AND CHAR_INSERT('日語', '本', 1) = '日本語');
			ASSERT(CHAR_INSERT('abc', 'X', 0) = 'Xabc');
			ASSERT(CHAR_DELETE('café', 1, 4) = 'caf' AND CHAR_DELETE('日本語', 1, 2) = '日語');
			ASSERT(CHAR_DELETE('café', 1, 0) = 'café' AND CHAR_DELETE('café', 1, 5) = 'café');
			ASSERT(CHAR_REPLACE('café', 'e', 1, 4) = 'cafe' AND CHAR_REPLACE('日本語', 'X', 2, 1) = 'X語');
			ASSERT(CHAR_REPLACE('café', 'X', 1, 0) = 'café');
		END_FUNCTION

		// A character position past the end maps to a byte position past the
		// end and the byte functions decide: just past and far past must agree.
		{test}
		FUNCTION test_char_positions_past_the_end_agree
			ASSERT(CHAR_MID('café', 2, 5) = CHAR_MID('café', 2, 99) AND CHAR_MID('café', 2, 99) = '');
			ASSERT(CHAR_DELETE('café', 2, 5) = CHAR_DELETE('café', 2, 99) AND CHAR_DELETE('café', 2, 99) = 'café');
			ASSERT(CHAR_REPLACE('café', 'X', 2, 5) = CHAR_REPLACE('café', 'X', 2, 99) AND CHAR_REPLACE('café', 'X', 2, 99) = 'café');
			ASSERT(CHAR_INSERT('café', 'X', 5) = CHAR_INSERT('café', 'X', 99) AND CHAR_INSERT('café', 'X', 99) = 'caféX');
		END_FUNCTION
	END_NAMESPACE

END_NAMESPACE
