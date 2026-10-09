/*
Byte order and BCD.

rk keeps every value in little-endian order, the order of WebAssembly: its
least significant byte at the lowest address. TO_BIG_ENDIAN and
FROM_BIG_ENDIAN reverse the bytes of a value, and TO_LITTLE_ENDIAN and
FROM_LITTLE_ENDIAN return it as it is.

Each takes an integer, a bit string, a REAL, an LREAL, a TIME or an LTIME,
and returns the type it was given. A value of one byte has no order, and is
returned as it is.

BCD stores a number one decimal digit in each 4 bits: WORD#16#0369 is 369.
Its conversions go between a bit string and the unsigned integer of the same
width, from BYTE and USINT to LWORD and ULINT.
*/
NAMESPACE Std.Bytes

	/* Returns `IN` with its bytes in big-endian order, the most significant
	   first: the order a peer reading big-endian data expects.

	   # Examples

	   ```iecst
	   w := TO_BIG_ENDIAN(WORD#16#1234);       // WORD#16#3412
	   d := TO_BIG_ENDIAN(DWORD#16#12345678);  // DWORD#16#78563412
	   b := TO_BIG_ENDIAN(BYTE#16#AB);         // BYTE#16#AB
	   ```
	*/
	FUNCTION TO_BIG_ENDIAN: BYTE
		VAR_INPUT
			IN: BYTE;
		END_VAR
		TO_BIG_ENDIAN := IN;
	END_FUNCTION

	FUNCTION TO_BIG_ENDIAN: SINT
		VAR_INPUT
			IN: SINT;
		END_VAR
		TO_BIG_ENDIAN := IN;
	END_FUNCTION

	FUNCTION TO_BIG_ENDIAN: USINT
		VAR_INPUT
			IN: USINT;
		END_VAR
		TO_BIG_ENDIAN := IN;
	END_FUNCTION

	FUNCTION TO_BIG_ENDIAN: WORD
		VAR_INPUT
			IN: WORD;
		END_VAR
		{wasm IN 'bswap' (params IN) (result TO_BIG_ENDIAN)}
	END_FUNCTION

	FUNCTION TO_BIG_ENDIAN: INT
		VAR_INPUT
			IN: INT;
		END_VAR
		VAR
			swapped: WORD;
		END_VAR
		{wasm IN 'bswap' (params IN) (result swapped)}
		{wasm 'nop' (params swapped) (result TO_BIG_ENDIAN)}
	END_FUNCTION

	FUNCTION TO_BIG_ENDIAN: UINT
		VAR_INPUT
			IN: UINT;
		END_VAR
		{wasm IN 'bswap' (params IN) (result TO_BIG_ENDIAN)}
	END_FUNCTION

	FUNCTION TO_BIG_ENDIAN: DWORD
		VAR_INPUT
			IN: DWORD;
		END_VAR
		{wasm IN 'bswap' (params IN) (result TO_BIG_ENDIAN)}
	END_FUNCTION

	FUNCTION TO_BIG_ENDIAN: DINT
		VAR_INPUT
			IN: DINT;
		END_VAR
		{wasm IN 'bswap' (params IN) (result TO_BIG_ENDIAN)}
	END_FUNCTION

	FUNCTION TO_BIG_ENDIAN: UDINT
		VAR_INPUT
			IN: UDINT;
		END_VAR
		{wasm IN 'bswap' (params IN) (result TO_BIG_ENDIAN)}
	END_FUNCTION

	FUNCTION TO_BIG_ENDIAN: REAL
		VAR_INPUT
			IN: REAL;
		END_VAR
		VAR
			bits: DWORD;
		END_VAR
		{wasm 'i32.reinterpret_f32' (params IN) (result bits)}
		{wasm bits 'bswap' (params bits) (result bits)}
		{wasm 'f32.reinterpret_i32' (params bits) (result TO_BIG_ENDIAN)}
	END_FUNCTION

	FUNCTION TO_BIG_ENDIAN: TIME
		VAR_INPUT
			IN: TIME;
		END_VAR
		{wasm IN 'bswap' (params IN) (result TO_BIG_ENDIAN)}
	END_FUNCTION

	FUNCTION TO_BIG_ENDIAN: LWORD
		VAR_INPUT
			IN: LWORD;
		END_VAR
		{wasm IN 'bswap' (params IN) (result TO_BIG_ENDIAN)}
	END_FUNCTION

	FUNCTION TO_BIG_ENDIAN: LINT
		VAR_INPUT
			IN: LINT;
		END_VAR
		{wasm IN 'bswap' (params IN) (result TO_BIG_ENDIAN)}
	END_FUNCTION

	FUNCTION TO_BIG_ENDIAN: ULINT
		VAR_INPUT
			IN: ULINT;
		END_VAR
		{wasm IN 'bswap' (params IN) (result TO_BIG_ENDIAN)}
	END_FUNCTION

	FUNCTION TO_BIG_ENDIAN: LREAL
		VAR_INPUT
			IN: LREAL;
		END_VAR
		VAR
			bits: LWORD;
		END_VAR
		{wasm 'i64.reinterpret_f64' (params IN) (result bits)}
		{wasm bits 'bswap' (params bits) (result bits)}
		{wasm 'f64.reinterpret_i64' (params bits) (result TO_BIG_ENDIAN)}
	END_FUNCTION

	FUNCTION TO_BIG_ENDIAN: LTIME
		VAR_INPUT
			IN: LTIME;
		END_VAR
		{wasm IN 'bswap' (params IN) (result TO_BIG_ENDIAN)}
	END_FUNCTION

	/* Returns `IN`, whose bytes are in big-endian order, in the order rk
	   computes with.

	   # Examples

	   ```iecst
	   w := FROM_BIG_ENDIAN(WORD#16#3412);     // WORD#16#1234
	   ```
	*/
	FUNCTION FROM_BIG_ENDIAN: BYTE
		VAR_INPUT
			IN: BYTE;
		END_VAR
		FROM_BIG_ENDIAN := IN;
	END_FUNCTION

	FUNCTION FROM_BIG_ENDIAN: SINT
		VAR_INPUT
			IN: SINT;
		END_VAR
		FROM_BIG_ENDIAN := IN;
	END_FUNCTION

	FUNCTION FROM_BIG_ENDIAN: USINT
		VAR_INPUT
			IN: USINT;
		END_VAR
		FROM_BIG_ENDIAN := IN;
	END_FUNCTION

	FUNCTION FROM_BIG_ENDIAN: WORD
		VAR_INPUT
			IN: WORD;
		END_VAR
		{wasm IN 'bswap' (params IN) (result FROM_BIG_ENDIAN)}
	END_FUNCTION

	FUNCTION FROM_BIG_ENDIAN: INT
		VAR_INPUT
			IN: INT;
		END_VAR
		VAR
			swapped: WORD;
		END_VAR
		{wasm IN 'bswap' (params IN) (result swapped)}
		{wasm 'nop' (params swapped) (result FROM_BIG_ENDIAN)}
	END_FUNCTION

	FUNCTION FROM_BIG_ENDIAN: UINT
		VAR_INPUT
			IN: UINT;
		END_VAR
		{wasm IN 'bswap' (params IN) (result FROM_BIG_ENDIAN)}
	END_FUNCTION

	FUNCTION FROM_BIG_ENDIAN: DWORD
		VAR_INPUT
			IN: DWORD;
		END_VAR
		{wasm IN 'bswap' (params IN) (result FROM_BIG_ENDIAN)}
	END_FUNCTION

	FUNCTION FROM_BIG_ENDIAN: DINT
		VAR_INPUT
			IN: DINT;
		END_VAR
		{wasm IN 'bswap' (params IN) (result FROM_BIG_ENDIAN)}
	END_FUNCTION

	FUNCTION FROM_BIG_ENDIAN: UDINT
		VAR_INPUT
			IN: UDINT;
		END_VAR
		{wasm IN 'bswap' (params IN) (result FROM_BIG_ENDIAN)}
	END_FUNCTION

	FUNCTION FROM_BIG_ENDIAN: REAL
		VAR_INPUT
			IN: REAL;
		END_VAR
		VAR
			bits: DWORD;
		END_VAR
		{wasm 'i32.reinterpret_f32' (params IN) (result bits)}
		{wasm bits 'bswap' (params bits) (result bits)}
		{wasm 'f32.reinterpret_i32' (params bits) (result FROM_BIG_ENDIAN)}
	END_FUNCTION

	FUNCTION FROM_BIG_ENDIAN: TIME
		VAR_INPUT
			IN: TIME;
		END_VAR
		{wasm IN 'bswap' (params IN) (result FROM_BIG_ENDIAN)}
	END_FUNCTION

	FUNCTION FROM_BIG_ENDIAN: LWORD
		VAR_INPUT
			IN: LWORD;
		END_VAR
		{wasm IN 'bswap' (params IN) (result FROM_BIG_ENDIAN)}
	END_FUNCTION

	FUNCTION FROM_BIG_ENDIAN: LINT
		VAR_INPUT
			IN: LINT;
		END_VAR
		{wasm IN 'bswap' (params IN) (result FROM_BIG_ENDIAN)}
	END_FUNCTION

	FUNCTION FROM_BIG_ENDIAN: ULINT
		VAR_INPUT
			IN: ULINT;
		END_VAR
		{wasm IN 'bswap' (params IN) (result FROM_BIG_ENDIAN)}
	END_FUNCTION

	FUNCTION FROM_BIG_ENDIAN: LREAL
		VAR_INPUT
			IN: LREAL;
		END_VAR
		VAR
			bits: LWORD;
		END_VAR
		{wasm 'i64.reinterpret_f64' (params IN) (result bits)}
		{wasm bits 'bswap' (params bits) (result bits)}
		{wasm 'f64.reinterpret_i64' (params bits) (result FROM_BIG_ENDIAN)}
	END_FUNCTION

	FUNCTION FROM_BIG_ENDIAN: LTIME
		VAR_INPUT
			IN: LTIME;
		END_VAR
		{wasm IN 'bswap' (params IN) (result FROM_BIG_ENDIAN)}
	END_FUNCTION

	/* Returns `IN` with its bytes in little-endian order, the least
	   significant first. rk keeps every value in that order, so `IN` is
	   returned as it is.
	*/
	FUNCTION TO_LITTLE_ENDIAN: BYTE
		VAR_INPUT
			IN: BYTE;
		END_VAR
		TO_LITTLE_ENDIAN := IN;
	END_FUNCTION

	FUNCTION TO_LITTLE_ENDIAN: SINT
		VAR_INPUT
			IN: SINT;
		END_VAR
		TO_LITTLE_ENDIAN := IN;
	END_FUNCTION

	FUNCTION TO_LITTLE_ENDIAN: USINT
		VAR_INPUT
			IN: USINT;
		END_VAR
		TO_LITTLE_ENDIAN := IN;
	END_FUNCTION

	FUNCTION TO_LITTLE_ENDIAN: WORD
		VAR_INPUT
			IN: WORD;
		END_VAR
		TO_LITTLE_ENDIAN := IN;
	END_FUNCTION

	FUNCTION TO_LITTLE_ENDIAN: INT
		VAR_INPUT
			IN: INT;
		END_VAR
		TO_LITTLE_ENDIAN := IN;
	END_FUNCTION

	FUNCTION TO_LITTLE_ENDIAN: UINT
		VAR_INPUT
			IN: UINT;
		END_VAR
		TO_LITTLE_ENDIAN := IN;
	END_FUNCTION

	FUNCTION TO_LITTLE_ENDIAN: DWORD
		VAR_INPUT
			IN: DWORD;
		END_VAR
		TO_LITTLE_ENDIAN := IN;
	END_FUNCTION

	FUNCTION TO_LITTLE_ENDIAN: DINT
		VAR_INPUT
			IN: DINT;
		END_VAR
		TO_LITTLE_ENDIAN := IN;
	END_FUNCTION

	FUNCTION TO_LITTLE_ENDIAN: UDINT
		VAR_INPUT
			IN: UDINT;
		END_VAR
		TO_LITTLE_ENDIAN := IN;
	END_FUNCTION

	FUNCTION TO_LITTLE_ENDIAN: REAL
		VAR_INPUT
			IN: REAL;
		END_VAR
		TO_LITTLE_ENDIAN := IN;
	END_FUNCTION

	FUNCTION TO_LITTLE_ENDIAN: TIME
		VAR_INPUT
			IN: TIME;
		END_VAR
		TO_LITTLE_ENDIAN := IN;
	END_FUNCTION

	FUNCTION TO_LITTLE_ENDIAN: LWORD
		VAR_INPUT
			IN: LWORD;
		END_VAR
		TO_LITTLE_ENDIAN := IN;
	END_FUNCTION

	FUNCTION TO_LITTLE_ENDIAN: LINT
		VAR_INPUT
			IN: LINT;
		END_VAR
		TO_LITTLE_ENDIAN := IN;
	END_FUNCTION

	FUNCTION TO_LITTLE_ENDIAN: ULINT
		VAR_INPUT
			IN: ULINT;
		END_VAR
		TO_LITTLE_ENDIAN := IN;
	END_FUNCTION

	FUNCTION TO_LITTLE_ENDIAN: LREAL
		VAR_INPUT
			IN: LREAL;
		END_VAR
		TO_LITTLE_ENDIAN := IN;
	END_FUNCTION

	FUNCTION TO_LITTLE_ENDIAN: LTIME
		VAR_INPUT
			IN: LTIME;
		END_VAR
		TO_LITTLE_ENDIAN := IN;
	END_FUNCTION

	/* Returns `IN`, whose bytes are in little-endian order, in the order rk
	   computes with. rk keeps every value in that order, so `IN` is
	   returned as it is.
	*/
	FUNCTION FROM_LITTLE_ENDIAN: BYTE
		VAR_INPUT
			IN: BYTE;
		END_VAR
		FROM_LITTLE_ENDIAN := IN;
	END_FUNCTION

	FUNCTION FROM_LITTLE_ENDIAN: SINT
		VAR_INPUT
			IN: SINT;
		END_VAR
		FROM_LITTLE_ENDIAN := IN;
	END_FUNCTION

	FUNCTION FROM_LITTLE_ENDIAN: USINT
		VAR_INPUT
			IN: USINT;
		END_VAR
		FROM_LITTLE_ENDIAN := IN;
	END_FUNCTION

	FUNCTION FROM_LITTLE_ENDIAN: WORD
		VAR_INPUT
			IN: WORD;
		END_VAR
		FROM_LITTLE_ENDIAN := IN;
	END_FUNCTION

	FUNCTION FROM_LITTLE_ENDIAN: INT
		VAR_INPUT
			IN: INT;
		END_VAR
		FROM_LITTLE_ENDIAN := IN;
	END_FUNCTION

	FUNCTION FROM_LITTLE_ENDIAN: UINT
		VAR_INPUT
			IN: UINT;
		END_VAR
		FROM_LITTLE_ENDIAN := IN;
	END_FUNCTION

	FUNCTION FROM_LITTLE_ENDIAN: DWORD
		VAR_INPUT
			IN: DWORD;
		END_VAR
		FROM_LITTLE_ENDIAN := IN;
	END_FUNCTION

	FUNCTION FROM_LITTLE_ENDIAN: DINT
		VAR_INPUT
			IN: DINT;
		END_VAR
		FROM_LITTLE_ENDIAN := IN;
	END_FUNCTION

	FUNCTION FROM_LITTLE_ENDIAN: UDINT
		VAR_INPUT
			IN: UDINT;
		END_VAR
		FROM_LITTLE_ENDIAN := IN;
	END_FUNCTION

	FUNCTION FROM_LITTLE_ENDIAN: REAL
		VAR_INPUT
			IN: REAL;
		END_VAR
		FROM_LITTLE_ENDIAN := IN;
	END_FUNCTION

	FUNCTION FROM_LITTLE_ENDIAN: TIME
		VAR_INPUT
			IN: TIME;
		END_VAR
		FROM_LITTLE_ENDIAN := IN;
	END_FUNCTION

	FUNCTION FROM_LITTLE_ENDIAN: LWORD
		VAR_INPUT
			IN: LWORD;
		END_VAR
		FROM_LITTLE_ENDIAN := IN;
	END_FUNCTION

	FUNCTION FROM_LITTLE_ENDIAN: LINT
		VAR_INPUT
			IN: LINT;
		END_VAR
		FROM_LITTLE_ENDIAN := IN;
	END_FUNCTION

	FUNCTION FROM_LITTLE_ENDIAN: ULINT
		VAR_INPUT
			IN: ULINT;
		END_VAR
		FROM_LITTLE_ENDIAN := IN;
	END_FUNCTION

	FUNCTION FROM_LITTLE_ENDIAN: LREAL
		VAR_INPUT
			IN: LREAL;
		END_VAR
		FROM_LITTLE_ENDIAN := IN;
	END_FUNCTION

	FUNCTION FROM_LITTLE_ENDIAN: LTIME
		VAR_INPUT
			IN: LTIME;
		END_VAR
		FROM_LITTLE_ENDIAN := IN;
	END_FUNCTION

	/* Returns the number `IN` holds in BCD, one decimal digit in each 4 bits,
	   the least significant in the lowest. A digit above 9 raises.

	   # Examples

	   ```iecst
	   n := WORD_BCD_TO_UINT(WORD#16#0369);  // UINT#369
	   n := BYTE_BCD_TO_USINT(BYTE#16#25);   // USINT#25
	   ```
	*/
	FUNCTION BYTE_BCD_TO_USINT: USINT
		VAR_INPUT
			IN: BYTE;
		END_VAR
		VAR
			bcd: USINT;
			digit: USINT;
			place: USINT := 1;
		END_VAR
		{wasm 'nop' (params IN) (result bcd)}
		WHILE bcd > 0 DO
			digit := bcd MOD 16;
			IF digit > 9 THEN
				__RAISE('invalid BCD digit');
			END_IF;
			BYTE_BCD_TO_USINT := BYTE_BCD_TO_USINT + digit * place;
			place := place * 10;
			bcd := bcd / 16;
		END_WHILE;
	END_FUNCTION

	FUNCTION WORD_BCD_TO_UINT: UINT
		VAR_INPUT
			IN: WORD;
		END_VAR
		VAR
			bcd: UINT;
			digit: UINT;
			place: UINT := 1;
		END_VAR
		{wasm 'nop' (params IN) (result bcd)}
		WHILE bcd > 0 DO
			digit := bcd MOD 16;
			IF digit > 9 THEN
				__RAISE('invalid BCD digit');
			END_IF;
			WORD_BCD_TO_UINT := WORD_BCD_TO_UINT + digit * place;
			place := place * 10;
			bcd := bcd / 16;
		END_WHILE;
	END_FUNCTION

	FUNCTION DWORD_BCD_TO_UDINT: UDINT
		VAR_INPUT
			IN: DWORD;
		END_VAR
		VAR
			bcd: UDINT;
			digit: UDINT;
			place: UDINT := 1;
		END_VAR
		{wasm 'nop' (params IN) (result bcd)}
		WHILE bcd > 0 DO
			digit := bcd MOD 16;
			IF digit > 9 THEN
				__RAISE('invalid BCD digit');
			END_IF;
			DWORD_BCD_TO_UDINT := DWORD_BCD_TO_UDINT + digit * place;
			place := place * 10;
			bcd := bcd / 16;
		END_WHILE;
	END_FUNCTION

	FUNCTION LWORD_BCD_TO_ULINT: ULINT
		VAR_INPUT
			IN: LWORD;
		END_VAR
		VAR
			bcd: ULINT;
			digit: ULINT;
			place: ULINT := 1;
		END_VAR
		{wasm 'nop' (params IN) (result bcd)}
		WHILE bcd > 0 DO
			digit := bcd MOD 16;
			IF digit > 9 THEN
				__RAISE('invalid BCD digit');
			END_IF;
			LWORD_BCD_TO_ULINT := LWORD_BCD_TO_ULINT + digit * place;
			place := place * 10;
			bcd := bcd / 16;
		END_WHILE;
	END_FUNCTION

	/* Returns `IN` in BCD, one decimal digit in each 4 bits, the least
	   significant in the lowest. A number with more digits than the bit
	   string holds raises: more than 99 for a BYTE, 9999 for a WORD.

	   # Examples

	   ```iecst
	   w := UINT_TO_BCD_WORD(UINT#369);    // WORD#16#0369
	   b := USINT_TO_BCD_BYTE(USINT#25);   // BYTE#16#25
	   ```
	*/
	FUNCTION USINT_TO_BCD_BYTE: BYTE
		VAR_INPUT
			IN: USINT;
		END_VAR
		VAR
			value: USINT;
			bcd: USINT;
			place: USINT := 1;
		END_VAR
		IF IN > 99 THEN
			__RAISE('value too large for BCD');
		END_IF;
		value := IN;
		WHILE value > 0 DO
			bcd := bcd + (value MOD 10) * place;
			place := place * 16;
			value := value / 10;
		END_WHILE;
		{wasm 'nop' (params bcd) (result USINT_TO_BCD_BYTE)}
	END_FUNCTION

	FUNCTION UINT_TO_BCD_WORD: WORD
		VAR_INPUT
			IN: UINT;
		END_VAR
		VAR
			value: UINT;
			bcd: UINT;
			place: UINT := 1;
		END_VAR
		IF IN > 9999 THEN
			__RAISE('value too large for BCD');
		END_IF;
		value := IN;
		WHILE value > 0 DO
			bcd := bcd + (value MOD 10) * place;
			place := place * 16;
			value := value / 10;
		END_WHILE;
		{wasm 'nop' (params bcd) (result UINT_TO_BCD_WORD)}
	END_FUNCTION

	FUNCTION UDINT_TO_BCD_DWORD: DWORD
		VAR_INPUT
			IN: UDINT;
		END_VAR
		VAR
			value: UDINT;
			bcd: UDINT;
			place: UDINT := 1;
		END_VAR
		IF IN > 99999999 THEN
			__RAISE('value too large for BCD');
		END_IF;
		value := IN;
		WHILE value > 0 DO
			bcd := bcd + (value MOD 10) * place;
			place := place * 16;
			value := value / 10;
		END_WHILE;
		{wasm 'nop' (params bcd) (result UDINT_TO_BCD_DWORD)}
	END_FUNCTION

	FUNCTION ULINT_TO_BCD_LWORD: LWORD
		VAR_INPUT
			IN: ULINT;
		END_VAR
		VAR
			value: ULINT;
			bcd: ULINT;
			place: ULINT := 1;
		END_VAR
		IF IN > 9999999999999999 THEN
			__RAISE('value too large for BCD');
		END_IF;
		value := IN;
		WHILE value > 0 DO
			bcd := bcd + (value MOD 10) * place;
			place := place * 16;
			value := value / 10;
		END_WHILE;
		{wasm 'nop' (params bcd) (result ULINT_TO_BCD_LWORD)}
	END_FUNCTION

	/* The number `IN` holds in BCD, as `WORD_BCD_TO_UINT` and its siblings
	   give it, named by the result only.
	*/
	FUNCTION BCD_TO_USINT: USINT
		VAR_INPUT
			IN: BYTE;
		END_VAR
		BCD_TO_USINT := BYTE_BCD_TO_USINT(IN);
	END_FUNCTION

	FUNCTION BCD_TO_UINT: UINT
		VAR_INPUT
			IN: WORD;
		END_VAR
		BCD_TO_UINT := WORD_BCD_TO_UINT(IN);
	END_FUNCTION

	FUNCTION BCD_TO_UDINT: UDINT
		VAR_INPUT
			IN: DWORD;
		END_VAR
		BCD_TO_UDINT := DWORD_BCD_TO_UDINT(IN);
	END_FUNCTION

	FUNCTION BCD_TO_ULINT: ULINT
		VAR_INPUT
			IN: LWORD;
		END_VAR
		BCD_TO_ULINT := LWORD_BCD_TO_ULINT(IN);
	END_FUNCTION

	/* `IN` in BCD, as `UINT_TO_BCD_WORD` and its siblings give it, named by
	   the result only.
	*/
	FUNCTION TO_BCD_BYTE: BYTE
		VAR_INPUT
			IN: USINT;
		END_VAR
		TO_BCD_BYTE := USINT_TO_BCD_BYTE(IN);
	END_FUNCTION

	FUNCTION TO_BCD_WORD: WORD
		VAR_INPUT
			IN: UINT;
		END_VAR
		TO_BCD_WORD := UINT_TO_BCD_WORD(IN);
	END_FUNCTION

	FUNCTION TO_BCD_DWORD: DWORD
		VAR_INPUT
			IN: UDINT;
		END_VAR
		TO_BCD_DWORD := UDINT_TO_BCD_DWORD(IN);
	END_FUNCTION

	FUNCTION TO_BCD_LWORD: LWORD
		VAR_INPUT
			IN: ULINT;
		END_VAR
		TO_BCD_LWORD := ULINT_TO_BCD_LWORD(IN);
	END_FUNCTION

	/* Whether every 4 bits of `IN` hold a decimal digit, 0 to 9: whether
	   `IN` converts from BCD without raising.

	   # Examples

	   ```iecst
	   ok := IS_VALID_BCD(WORD#16#0369);  // TRUE
	   ok := IS_VALID_BCD(WORD#16#03A9);  // FALSE
	   ```
	*/
	FUNCTION IS_VALID_BCD: BOOL
		VAR_INPUT
			IN: BYTE;
		END_VAR
		VAR
			bcd: USINT;
		END_VAR
		{wasm 'nop' (params IN) (result bcd)}
		IS_VALID_BCD := TRUE;
		WHILE bcd > 0 DO
			IF bcd MOD 16 > 9 THEN
				IS_VALID_BCD := FALSE;
				RETURN;
			END_IF;
			bcd := bcd / 16;
		END_WHILE;
	END_FUNCTION

	FUNCTION IS_VALID_BCD: BOOL
		VAR_INPUT
			IN: WORD;
		END_VAR
		VAR
			bcd: UINT;
		END_VAR
		{wasm 'nop' (params IN) (result bcd)}
		IS_VALID_BCD := TRUE;
		WHILE bcd > 0 DO
			IF bcd MOD 16 > 9 THEN
				IS_VALID_BCD := FALSE;
				RETURN;
			END_IF;
			bcd := bcd / 16;
		END_WHILE;
	END_FUNCTION

	FUNCTION IS_VALID_BCD: BOOL
		VAR_INPUT
			IN: DWORD;
		END_VAR
		VAR
			bcd: UDINT;
		END_VAR
		{wasm 'nop' (params IN) (result bcd)}
		IS_VALID_BCD := TRUE;
		WHILE bcd > 0 DO
			IF bcd MOD 16 > 9 THEN
				IS_VALID_BCD := FALSE;
				RETURN;
			END_IF;
			bcd := bcd / 16;
		END_WHILE;
	END_FUNCTION

	FUNCTION IS_VALID_BCD: BOOL
		VAR_INPUT
			IN: LWORD;
		END_VAR
		VAR
			bcd: ULINT;
		END_VAR
		{wasm 'nop' (params IN) (result bcd)}
		IS_VALID_BCD := TRUE;
		WHILE bcd > 0 DO
			IF bcd MOD 16 > 9 THEN
				IS_VALID_BCD := FALSE;
				RETURN;
			END_IF;
			bcd := bcd / 16;
		END_WHILE;
	END_FUNCTION

	NAMESPACE Test
		USING Std.Unit
		USING Std.Convert

		{test}
		FUNCTION test_word ASSERT_EQ(TO_BIG_ENDIAN(WORD#16#1234), WORD#16#3412);
		END_FUNCTION

		{test}
		FUNCTION test_uint ASSERT_EQ(TO_BIG_ENDIAN(UINT#16#FF00), UINT#16#00FF);
		END_FUNCTION

		{test}
		FUNCTION test_int_keeps_its_sign ASSERT_EQ(TO_BIG_ENDIAN(INT#-2), INT#-257);
		END_FUNCTION

		{test}
		FUNCTION test_int_takes_a_sign ASSERT_EQ(TO_BIG_ENDIAN(INT#384), INT#-32767);
		END_FUNCTION

		{test}
		FUNCTION test_dword ASSERT_EQ(TO_BIG_ENDIAN(DWORD#16#12345678), DWORD#16#78563412);
		END_FUNCTION

		{test}
		FUNCTION test_dint ASSERT_EQ(TO_BIG_ENDIAN(DINT#-2), DINT#-16777217);
		END_FUNCTION

		{test}
		FUNCTION test_udint ASSERT_EQ(TO_BIG_ENDIAN(UDINT#16#11223344), UDINT#16#44332211);
		END_FUNCTION

		{test}
		FUNCTION test_lword ASSERT_EQ(TO_BIG_ENDIAN(LWORD#16#0102030405060708), LWORD#16#0807060504030201);
		END_FUNCTION

		{test}
		FUNCTION test_lint ASSERT_EQ(TO_BIG_ENDIAN(LINT#1), LINT#16#0100000000000000);
		END_FUNCTION

		{test}
		FUNCTION test_ulint ASSERT_EQ(TO_BIG_ENDIAN(ULINT#16#FF), ULINT#16#FF00000000000000);
		END_FUNCTION

		{test}
		FUNCTION test_real ASSERT_EQ(TO_BIG_ENDIAN(REAL#1.0), DWORD_TO_REAL(DWORD#16#0000803F));
		END_FUNCTION

		{test}
		FUNCTION test_lreal ASSERT_EQ(TO_BIG_ENDIAN(LREAL#1.0), LWORD_TO_LREAL(LWORD#16#000000000000F03F));
		END_FUNCTION

		{test}
		FUNCTION test_time ASSERT_EQ(TO_BIG_ENDIAN(T#1ms), T#16777216ms);
		END_FUNCTION

		{test}
		FUNCTION test_ltime ASSERT_EQ(TO_BIG_ENDIAN(LT#1ns), LT#72057594037927936ns);
		END_FUNCTION

		{test}
		FUNCTION test_one_byte ASSERT_EQ(TO_BIG_ENDIAN(BYTE#16#AB), BYTE#16#AB);
		END_FUNCTION

		{test}
		FUNCTION test_signed_byte ASSERT_EQ(FROM_BIG_ENDIAN(SINT#-3), SINT#-3);
		END_FUNCTION

		{test}
		FUNCTION test_round_trip_real ASSERT_EQ(FROM_BIG_ENDIAN(TO_BIG_ENDIAN(REAL#1.5)), REAL#1.5);
		END_FUNCTION

		{test}
		FUNCTION test_round_trip_dint ASSERT_EQ(FROM_BIG_ENDIAN(TO_BIG_ENDIAN(DINT#-123456)), DINT#-123456);
		END_FUNCTION

		{test}
		FUNCTION test_little_endian ASSERT_EQ(TO_LITTLE_ENDIAN(DWORD#16#12345678), DWORD#16#12345678);
		END_FUNCTION

		{test}
		FUNCTION test_from_little_endian ASSERT_EQ(FROM_LITTLE_ENDIAN(LINT#-5), LINT#-5);
		END_FUNCTION

		{test}
		FUNCTION test_bcd_byte ASSERT_EQ(USINT_TO_BCD_BYTE(USINT#25), BYTE#2#0010_0101);
		END_FUNCTION

		{test}
		FUNCTION test_bcd_word_decodes ASSERT_EQ(WORD_BCD_TO_UINT(WORD#2#0011_0110_1001), UINT#369);
		END_FUNCTION

		{test}
		FUNCTION test_bcd_dword ASSERT_EQ(UDINT_TO_BCD_DWORD(UDINT#12345678), DWORD#16#12345678);
		END_FUNCTION

		{test}
		FUNCTION test_bcd_lword_largest ASSERT_EQ(ULINT_TO_BCD_LWORD(ULINT#9999999999999999), LWORD#16#9999999999999999);
		END_FUNCTION

		{test}
		FUNCTION test_bcd_lword_decodes ASSERT_EQ(LWORD_BCD_TO_ULINT(LWORD#16#1234567890123456), ULINT#1234567890123456);
		END_FUNCTION

		{test}
		FUNCTION test_bcd_zero ASSERT_EQ(UINT_TO_BCD_WORD(UINT#0), WORD#0);
		END_FUNCTION

		{test}
		FUNCTION test_bcd_round_trip ASSERT_EQ(DWORD_BCD_TO_UDINT(UDINT_TO_BCD_DWORD(UDINT#99999999)), UDINT#99999999);
		END_FUNCTION

		{test}
		FUNCTION test_bcd_to_by_result ASSERT_EQ(BCD_TO_UINT(WORD#16#0042), UINT#42);
		END_FUNCTION

		{test}
		FUNCTION test_to_bcd_by_result ASSERT_EQ(TO_BCD_WORD(UINT#42), WORD#16#42);
		END_FUNCTION

		{test}
		FUNCTION test_valid_bcd ASSERT_EQ(IS_VALID_BCD(WORD#16#1234), TRUE);
		END_FUNCTION

		{test}
		FUNCTION test_invalid_bcd ASSERT_EQ(IS_VALID_BCD(WORD#16#12A4), FALSE);
		END_FUNCTION

		{test}
		FUNCTION test_invalid_bcd_top_digit ASSERT_EQ(IS_VALID_BCD(LWORD#16#F000000000000000), FALSE);
		END_FUNCTION

		{test}
		FUNCTION test_valid_bcd_byte ASSERT_EQ(IS_VALID_BCD(BYTE#16#99), TRUE);
		END_FUNCTION
	END_NAMESPACE
END_NAMESPACE
