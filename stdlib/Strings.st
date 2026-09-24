/*
String functions.

A `STRING` holds UTF-8 text, and its length is a number of bytes. The
functions here come in two families.

`LEN`, `FIND`, `LEFT`, `RIGHT`, `MID`, `INSERT`, `DELETE` and `REPLACE` count
bytes. They are the fast ones, and on ASCII text a byte is a character. On
other text a byte position can fall inside a character, and slicing there
yields a string that is not valid UTF-8.

The `CHAR_` functions count characters and never split one. A character is
one well-formed UTF-8 sequence. A byte that belongs to none counts as a
character on its own, its Latin-1 code point, like `CHAR#'$E9'`.

Positions start at 1 in both families. A function that returns a `STRING`
truncates it without an error when it does not fit where it is stored.
*/
NAMESPACE Std.Strings

	/* Returns the length of `IN`, in bytes.

	   This is not the number of characters: `LEN('café')` is 5, because `é`
	   takes two bytes. See [Std.Strings.CHAR_COUNT] for that.

	   The length is stored with the string, so this does not scan it.

	   # Examples

	   ```iecst
	   n := LEN('hello');  // 5
	   n := LEN('');       // 0
	   ```
	*/
	FUNCTION LEN: UDINT
		VAR_INPUT
			IN: STRING;
		END_VAR
		{wasm 'str.byte_len' (params IN) (result LEN)}
	END_FUNCTION

	/* Returns `TRUE` if `IN` is well-formed UTF-8.

	   Text that comes from outside the program, such as a host import or a
	   fieldbus frame, may not be. Check it before slicing it by bytes.

	   # Examples

	   ```iecst
	   ok := IS_UTF8('café');  // TRUE
	   ok := IS_UTF8('$E9');   // FALSE, a lone Latin-1 byte
	   ```
	*/
	FUNCTION IS_UTF8: BOOL
		VAR_INPUT
			IN: STRING;
		END_VAR
		{wasm 'str.is_utf8' (params IN) (result IS_UTF8)}
	END_FUNCTION

	/* Returns the byte position of the first occurrence of `NEEDLE` in
	   `HAYSTACK`, or 0 if there is none.

	   Positions start at 1. An empty `NEEDLE` is found at position 1.

	   To look for a single `CHAR`, convert it with [Std.Convert.CHAR_TO_STRING].

	   # Examples

	   ```iecst
	   p := FIND('hello world', 'world');  // 7
	   p := FIND('hello world', 'xyz');    // 0
	   ```
	*/
	FUNCTION FIND: UDINT
		VAR_INPUT
			HAYSTACK: STRING;
			NEEDLE: STRING;
		END_VAR
		{wasm 'str.byte_find' (params HAYSTACK NEEDLE) (result FIND)}
	END_FUNCTION

	/* Returns `A` followed by `B`.

	   To append a single `CHAR`, convert it with [Std.Convert.CHAR_TO_STRING].

	   # Examples

	   ```iecst
	   s := CONCAT('hello ', 'world');  // 'hello world'
	   ```
	*/
	FUNCTION CONCAT: STRING
		VAR_INPUT
			A: STRING;
			B: STRING;
		END_VAR
		{wasm 'str.concat' (params A B) (result CONCAT)}
	END_FUNCTION

	/* Returns the first `N` bytes of `S`.

	   Returns all of `S` when `N` is greater than its length.

	   # Examples

	   ```iecst
	   s := LEFT('hello world', 5);  // 'hello'
	   s := LEFT('hi', 10);          // 'hi'
	   ```
	*/
	FUNCTION LEFT: STRING
		VAR_INPUT
			S: STRING;
			N: UDINT;
		END_VAR
		{wasm 'str.byte_left' (params S N) (result LEFT)}
	END_FUNCTION

	/* Returns the last `N` bytes of `S`.

	   Returns all of `S` when `N` is greater than its length.

	   # Examples

	   ```iecst
	   s := RIGHT('hello world', 5);  // 'world'
	   s := RIGHT('hi', 10);          // 'hi'
	   ```
	*/
	FUNCTION RIGHT: STRING
		VAR_INPUT
			S: STRING;
			N: UDINT;
		END_VAR
		{wasm 'str.byte_right' (params S N) (result RIGHT)}
	END_FUNCTION

	/* Returns `N` bytes of `S`, starting at byte position `P`.

	   Returns fewer when `S` ends first, and an empty string when `P` is 0 or
	   past the end.

	   # Examples

	   ```iecst
	   s := MID('hello world', 5, 7);  // 'world'
	   s := MID('hello', 10, 3);       // 'llo'
	   s := MID('hello', 3, 10);       // ''
	   ```
	*/
	FUNCTION MID: STRING
		VAR_INPUT
			S: STRING;
			N: UDINT;
			P: UDINT;
		END_VAR
		{wasm 'str.byte_mid' (params S N P) (result MID)}
	END_FUNCTION

	/* Returns `S` with `INS` inserted after byte position `P`.

	   A `P` of 0 inserts at the front.

	   To insert a single `CHAR`, convert it with [Std.Convert.CHAR_TO_STRING].

	   # Examples

	   ```iecst
	   s := INSERT('helloworld', 'XX', 5);  // 'helloXXworld'
	   s := INSERT('world', 'hello ', 0);   // 'hello world'
	   ```
	*/
	FUNCTION INSERT: STRING
		VAR_INPUT
			S: STRING;
			INS: STRING;
			P: UDINT;
		END_VAR
		{wasm 'str.byte_insert' (params S INS P) (result INSERT)}
	END_FUNCTION

	/* Returns `S` without the `N` bytes that start at byte position `P`.

	   Removes fewer when `S` ends first, and nothing when `P` is past the end.

	   # Examples

	   ```iecst
	   s := DELETE('hello world', 6, 6);  // 'hello'
	   s := DELETE('hello', 10, 3);       // 'he'
	   s := DELETE('hello', 3, 10);       // 'hello'
	   ```
	*/
	FUNCTION DELETE: STRING
		VAR_INPUT
			S: STRING;
			N: UDINT;
			P: UDINT;
		END_VAR
		{wasm 'str.byte_delete' (params S N P) (result DELETE)}
	END_FUNCTION

	/* Returns `S` with the `N` bytes that start at byte position `P` replaced
	   by `INS`.

	   `INS` does not have to be `N` bytes long.

	   To replace with a single `CHAR`, convert it with [Std.Convert.CHAR_TO_STRING].

	   # Examples

	   ```iecst
	   s := REPLACE('hello world', 'there', 5, 7);  // 'hello there'
	   s := REPLACE('hello world', '!', 5, 7);      // 'hello !'
	   ```
	*/
	FUNCTION REPLACE: STRING
		VAR_INPUT
			S: STRING;
			INS: STRING;
			N: UDINT;
			P: UDINT;
		END_VAR
		{wasm 'str.byte_replace' (params S INS N P) (result REPLACE)}
	END_FUNCTION

	/* Returns the number of characters in `IN`.

	   # Examples

	   ```iecst
	   n := CHAR_COUNT('café');    // 4, where LEN returns 5
	   n := CHAR_COUNT('日本語');  // 3, where LEN returns 9
	   ```
	*/
	FUNCTION CHAR_COUNT: UDINT
		VAR_INPUT
			IN: STRING;
		END_VAR
		{wasm 'str.char_count' (params IN) (result CHAR_COUNT)}
	END_FUNCTION

	/* Returns the character at position `P` of `IN`.

	   Returns `CHAR#'$00'` when `P` is 0 or past the end.

	   # Examples

	   ```iecst
	   c := CHAR_AT('café', 4);  // CHAR#'é'
	   c := CHAR_AT('abc', 4);   // CHAR#'$00'
	   ```
	*/
	FUNCTION CHAR_AT: CHAR
		VAR_INPUT
			IN: STRING;
			P: UDINT;
		END_VAR
		{wasm 'str.char_at' (params IN P) (result CHAR_AT)}
	END_FUNCTION

	/* Returns the character position of the first occurrence of `NEEDLE` in
	   `HAYSTACK`, or 0 if there is none.

	   An empty `NEEDLE` is found at position 1, as with [Std.Strings.FIND].

	   # Examples

	   ```iecst
	   p := CHAR_FIND('日本語', '語');  // 3, where FIND returns 7
	   p := CHAR_FIND('abc', 'x');      // 0
	   ```
	*/
	FUNCTION CHAR_FIND: UDINT
		VAR_INPUT
			HAYSTACK: STRING;
			NEEDLE: STRING;
		END_VAR
		{wasm 'str.char_find' (params HAYSTACK NEEDLE) (result CHAR_FIND)}
	END_FUNCTION

	/* Returns the first `N` characters of `S`.

	   Returns all of `S` when `N` is greater than its character count.

	   # Examples

	   ```iecst
	   s := CHAR_LEFT('café', 3);   // 'caf'
	   s := CHAR_LEFT('café', 10);  // 'café'
	   ```
	*/
	FUNCTION CHAR_LEFT: STRING
		VAR_INPUT
			S: STRING;
			N: UDINT;
		END_VAR
		{wasm 'str.char_left' (params S N) (result CHAR_LEFT)}
	END_FUNCTION

	/* Returns the last `N` characters of `S`.

	   Returns all of `S` when `N` is greater than its character count.

	   # Examples

	   ```iecst
	   s := CHAR_RIGHT('日本語', 2);  // '本語'
	   s := CHAR_RIGHT('café', 1);    // 'é'
	   ```
	*/
	FUNCTION CHAR_RIGHT: STRING
		VAR_INPUT
			S: STRING;
			N: UDINT;
		END_VAR
		{wasm 'str.char_right' (params S N) (result CHAR_RIGHT)}
	END_FUNCTION

	/* Returns `N` characters of `S`, starting at character position `P`.

	   Returns fewer when `S` ends first, and an empty string when `P` is 0 or
	   past the end.

	   # Examples

	   ```iecst
	   s := CHAR_MID('café', 2, 3);    // 'fé'
	   s := CHAR_MID('日本語', 1, 2);  // '本'
	   ```
	*/
	FUNCTION CHAR_MID: STRING
		VAR_INPUT
			S: STRING;
			N: UDINT;
			P: UDINT;
		END_VAR
		{wasm 'str.char_mid' (params S N P) (result CHAR_MID)}
	END_FUNCTION

	/* Returns `S` with `INS` inserted after character position `P`.

	   A `P` of 0 inserts at the front, and a `P` past the end appends.

	   # Examples

	   ```iecst
	   s := CHAR_INSERT('caf', 'é', 3);    // 'café'
	   s := CHAR_INSERT('abc', 'X', 0);    // 'Xabc'
	   s := CHAR_INSERT('café', 'X', 99);  // 'caféX'
	   ```
	*/
	FUNCTION CHAR_INSERT: STRING
		VAR_INPUT
			S: STRING;
			INS: STRING;
			P: UDINT;
		END_VAR
		{wasm 'str.char_insert' (params S INS P) (result CHAR_INSERT)}
	END_FUNCTION

	/* Returns `S` without the `N` characters that start at character
	   position `P`.

	   Returns `S` unchanged when `P` is 0 or past the end.

	   # Examples

	   ```iecst
	   s := CHAR_DELETE('日本語', 1, 2);  // '日語'
	   s := CHAR_DELETE('café', 1, 5);    // 'café'
	   ```
	*/
	FUNCTION CHAR_DELETE: STRING
		VAR_INPUT
			S: STRING;
			N: UDINT;
			P: UDINT;
		END_VAR
		{wasm 'str.char_delete' (params S N P) (result CHAR_DELETE)}
	END_FUNCTION

	/* Returns `S` with the `N` characters that start at character position
	   `P` replaced by `INS`.

	   Returns `S` unchanged when `P` is 0 or past the end.

	   # Examples

	   ```iecst
	   s := CHAR_REPLACE('café', 'e', 1, 4);    // 'cafe'
	   s := CHAR_REPLACE('日本語', 'X', 2, 1);  // 'X語'
	   ```
	*/
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
			// Independent inner CONCATs feeding an outer CONCAT: each
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

		/* CHAR-as-string usage: explicit conversion via
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
			// Two nested CHAR_TO_STRING calls: each must get its own
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

		// Both operands are producer calls: each needs its own scratch
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
