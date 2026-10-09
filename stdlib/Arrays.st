/*
Array functions.

`LOWER_BOUND` and `UPPER_BOUND` read the bounds of an array. They take an
array of any type and any number of dimensions, so a FUNCTION with an
`ARRAY[*]` parameter can walk whatever array its caller passed.

`GET_DWORD_BE` and its siblings read a WORD, a DWORD, an LWORD, a REAL or an
LREAL out of an array of bytes, at a subscript of that array, in big-endian
(`_BE`) or little-endian (`_LE`) order. `PUT_DWORD_BE` and its siblings
write one. A position whose bytes are not all in the array faults, as any
subscript past its bounds does.
*/
NAMESPACE Std.Arrays

	/* Returns the lower bound of dimension `DIM` of `ARR`.

	   Dimensions are numbered from 1. A `DIM` outside them raises.

	   # Examples

	   ```iecst
	   FOR i := LOWER_BOUND(values, 1) TO UPPER_BOUND(values, 1) DO
	       total := total + values[i];
	   END_FOR;
	   ```
	*/
	FUNCTION LOWER_BOUND: DINT
		VAR_INPUT
			ARR: ARRAY[*];
			DIM: DINT;
		END_VAR
		VAR
			dimensions: DINT;
		END_VAR

		{wasm 'array.dimensions' (params ARR) (result dimensions)}
		IF DIM < 1 OR DIM > dimensions THEN
			__RAISE('array dimension out of range');
		END_IF
		{wasm 'array.lower_bound' (params ARR DIM) (result LOWER_BOUND)}
	END_FUNCTION

	/* Returns the upper bound of dimension `DIM` of `ARR`.

	   Dimensions are numbered from 1. A `DIM` outside them raises.

	   # Examples

	   ```iecst
	   // m : ARRAY[1..3, 0..9] OF REAL
	   rows := UPPER_BOUND(m, 1) - LOWER_BOUND(m, 1) + 1;     // 3
	   columns := UPPER_BOUND(m, 2) - LOWER_BOUND(m, 2) + 1;  // 10
	   ```
	*/
	FUNCTION UPPER_BOUND: DINT
		VAR_INPUT
			ARR: ARRAY[*];
			DIM: DINT;
		END_VAR
		VAR
			dimensions: DINT;
		END_VAR

		{wasm 'array.dimensions' (params ARR) (result dimensions)}
		IF DIM < 1 OR DIM > dimensions THEN
			__RAISE('array dimension out of range');
		END_IF
		{wasm 'array.upper_bound' (params ARR DIM) (result UPPER_BOUND)}
	END_FUNCTION

	/* Returns the value whose bytes are in `buf` from `pos` on, in big-endian
	   order: the most significant at `pos`.

	   # Examples

	   ```iecst
	   // frame holds 16#12, 16#34, 16#56, 16#78 from its subscript 0 on
	   d := GET_DWORD_BE(frame, 0);  // DWORD#16#12345678
	   w := GET_WORD_BE(frame, 2);   // WORD#16#5678
	   ```
	*/
	FUNCTION GET_WORD_BE: WORD
		VAR_IN_OUT
			buf: ARRAY[*] OF BYTE;
		END_VAR
		VAR_INPUT
			pos: DINT;
		END_VAR
		GET_WORD_BE.%B1 := buf[pos];
		GET_WORD_BE.%B0 := buf[pos + 1];
	END_FUNCTION

	FUNCTION GET_DWORD_BE: DWORD
		VAR_IN_OUT
			buf: ARRAY[*] OF BYTE;
		END_VAR
		VAR_INPUT
			pos: DINT;
		END_VAR
		GET_DWORD_BE.%B3 := buf[pos];
		GET_DWORD_BE.%B2 := buf[pos + 1];
		GET_DWORD_BE.%B1 := buf[pos + 2];
		GET_DWORD_BE.%B0 := buf[pos + 3];
	END_FUNCTION

	FUNCTION GET_LWORD_BE: LWORD
		VAR_IN_OUT
			buf: ARRAY[*] OF BYTE;
		END_VAR
		VAR_INPUT
			pos: DINT;
		END_VAR
		GET_LWORD_BE.%B7 := buf[pos];
		GET_LWORD_BE.%B6 := buf[pos + 1];
		GET_LWORD_BE.%B5 := buf[pos + 2];
		GET_LWORD_BE.%B4 := buf[pos + 3];
		GET_LWORD_BE.%B3 := buf[pos + 4];
		GET_LWORD_BE.%B2 := buf[pos + 5];
		GET_LWORD_BE.%B1 := buf[pos + 6];
		GET_LWORD_BE.%B0 := buf[pos + 7];
	END_FUNCTION

	FUNCTION GET_REAL_BE: REAL
		VAR_IN_OUT
			buf: ARRAY[*] OF BYTE;
		END_VAR
		VAR_INPUT
			pos: DINT;
		END_VAR
		VAR
			v: DWORD;
		END_VAR
		v := GET_DWORD_BE(buf, pos);
		{wasm 'f32.reinterpret_i32' (params v) (result GET_REAL_BE)}
	END_FUNCTION

	FUNCTION GET_LREAL_BE: LREAL
		VAR_IN_OUT
			buf: ARRAY[*] OF BYTE;
		END_VAR
		VAR_INPUT
			pos: DINT;
		END_VAR
		VAR
			v: LWORD;
		END_VAR
		v := GET_LWORD_BE(buf, pos);
		{wasm 'f64.reinterpret_i64' (params v) (result GET_LREAL_BE)}
	END_FUNCTION

	/* Returns the value whose bytes are in `buf` from `pos` on, in
	   little-endian order: the least significant at `pos`.

	   # Examples

	   ```iecst
	   // frame holds 16#12, 16#34, 16#56, 16#78 from its subscript 0 on
	   d := GET_DWORD_LE(frame, 0);  // DWORD#16#78563412
	   ```
	*/
	FUNCTION GET_WORD_LE: WORD
		VAR_IN_OUT
			buf: ARRAY[*] OF BYTE;
		END_VAR
		VAR_INPUT
			pos: DINT;
		END_VAR
		GET_WORD_LE.%B0 := buf[pos];
		GET_WORD_LE.%B1 := buf[pos + 1];
	END_FUNCTION

	FUNCTION GET_DWORD_LE: DWORD
		VAR_IN_OUT
			buf: ARRAY[*] OF BYTE;
		END_VAR
		VAR_INPUT
			pos: DINT;
		END_VAR
		GET_DWORD_LE.%B0 := buf[pos];
		GET_DWORD_LE.%B1 := buf[pos + 1];
		GET_DWORD_LE.%B2 := buf[pos + 2];
		GET_DWORD_LE.%B3 := buf[pos + 3];
	END_FUNCTION

	FUNCTION GET_LWORD_LE: LWORD
		VAR_IN_OUT
			buf: ARRAY[*] OF BYTE;
		END_VAR
		VAR_INPUT
			pos: DINT;
		END_VAR
		GET_LWORD_LE.%B0 := buf[pos];
		GET_LWORD_LE.%B1 := buf[pos + 1];
		GET_LWORD_LE.%B2 := buf[pos + 2];
		GET_LWORD_LE.%B3 := buf[pos + 3];
		GET_LWORD_LE.%B4 := buf[pos + 4];
		GET_LWORD_LE.%B5 := buf[pos + 5];
		GET_LWORD_LE.%B6 := buf[pos + 6];
		GET_LWORD_LE.%B7 := buf[pos + 7];
	END_FUNCTION

	FUNCTION GET_REAL_LE: REAL
		VAR_IN_OUT
			buf: ARRAY[*] OF BYTE;
		END_VAR
		VAR_INPUT
			pos: DINT;
		END_VAR
		VAR
			v: DWORD;
		END_VAR
		v := GET_DWORD_LE(buf, pos);
		{wasm 'f32.reinterpret_i32' (params v) (result GET_REAL_LE)}
	END_FUNCTION

	FUNCTION GET_LREAL_LE: LREAL
		VAR_IN_OUT
			buf: ARRAY[*] OF BYTE;
		END_VAR
		VAR_INPUT
			pos: DINT;
		END_VAR
		VAR
			v: LWORD;
		END_VAR
		v := GET_LWORD_LE(buf, pos);
		{wasm 'f64.reinterpret_i64' (params v) (result GET_LREAL_LE)}
	END_FUNCTION

	/* Writes the bytes of `value` into `buf` from `pos` on, in big-endian
	   order: the most significant at `pos`.

	   # Examples

	   ```iecst
	   PUT_DWORD_BE(frame, 0, DWORD#16#12345678);  // 16#12, 16#34, 16#56, 16#78
	   PUT_REAL_BE(frame, 4, REAL#1.0);            // 16#3F, 16#80, 16#00, 16#00
	   ```
	*/
	FUNCTION PUT_WORD_BE
		VAR_IN_OUT
			buf: ARRAY[*] OF BYTE;
		END_VAR
		VAR_INPUT
			pos: DINT;
			value: WORD;
		END_VAR
		buf[pos] := value.%B1;
		buf[pos + 1] := value.%B0;
	END_FUNCTION

	FUNCTION PUT_DWORD_BE
		VAR_IN_OUT
			buf: ARRAY[*] OF BYTE;
		END_VAR
		VAR_INPUT
			pos: DINT;
			value: DWORD;
		END_VAR
		buf[pos] := value.%B3;
		buf[pos + 1] := value.%B2;
		buf[pos + 2] := value.%B1;
		buf[pos + 3] := value.%B0;
	END_FUNCTION

	FUNCTION PUT_LWORD_BE
		VAR_IN_OUT
			buf: ARRAY[*] OF BYTE;
		END_VAR
		VAR_INPUT
			pos: DINT;
			value: LWORD;
		END_VAR
		buf[pos] := value.%B7;
		buf[pos + 1] := value.%B6;
		buf[pos + 2] := value.%B5;
		buf[pos + 3] := value.%B4;
		buf[pos + 4] := value.%B3;
		buf[pos + 5] := value.%B2;
		buf[pos + 6] := value.%B1;
		buf[pos + 7] := value.%B0;
	END_FUNCTION

	FUNCTION PUT_REAL_BE
		VAR_IN_OUT
			buf: ARRAY[*] OF BYTE;
		END_VAR
		VAR_INPUT
			pos: DINT;
			value: REAL;
		END_VAR
		VAR
			v: DWORD;
		END_VAR
		{wasm 'i32.reinterpret_f32' (params value) (result v)}
		PUT_DWORD_BE(buf, pos, v);
	END_FUNCTION

	FUNCTION PUT_LREAL_BE
		VAR_IN_OUT
			buf: ARRAY[*] OF BYTE;
		END_VAR
		VAR_INPUT
			pos: DINT;
			value: LREAL;
		END_VAR
		VAR
			v: LWORD;
		END_VAR
		{wasm 'i64.reinterpret_f64' (params value) (result v)}
		PUT_LWORD_BE(buf, pos, v);
	END_FUNCTION

	/* Writes the bytes of `value` into `buf` from `pos` on, in little-endian
	   order: the least significant at `pos`.

	   # Examples

	   ```iecst
	   PUT_DWORD_LE(frame, 0, DWORD#16#12345678);  // 16#78, 16#56, 16#34, 16#12
	   ```
	*/
	FUNCTION PUT_WORD_LE
		VAR_IN_OUT
			buf: ARRAY[*] OF BYTE;
		END_VAR
		VAR_INPUT
			pos: DINT;
			value: WORD;
		END_VAR
		buf[pos] := value.%B0;
		buf[pos + 1] := value.%B1;
	END_FUNCTION

	FUNCTION PUT_DWORD_LE
		VAR_IN_OUT
			buf: ARRAY[*] OF BYTE;
		END_VAR
		VAR_INPUT
			pos: DINT;
			value: DWORD;
		END_VAR
		buf[pos] := value.%B0;
		buf[pos + 1] := value.%B1;
		buf[pos + 2] := value.%B2;
		buf[pos + 3] := value.%B3;
	END_FUNCTION

	FUNCTION PUT_LWORD_LE
		VAR_IN_OUT
			buf: ARRAY[*] OF BYTE;
		END_VAR
		VAR_INPUT
			pos: DINT;
			value: LWORD;
		END_VAR
		buf[pos] := value.%B0;
		buf[pos + 1] := value.%B1;
		buf[pos + 2] := value.%B2;
		buf[pos + 3] := value.%B3;
		buf[pos + 4] := value.%B4;
		buf[pos + 5] := value.%B5;
		buf[pos + 6] := value.%B6;
		buf[pos + 7] := value.%B7;
	END_FUNCTION

	FUNCTION PUT_REAL_LE
		VAR_IN_OUT
			buf: ARRAY[*] OF BYTE;
		END_VAR
		VAR_INPUT
			pos: DINT;
			value: REAL;
		END_VAR
		VAR
			v: DWORD;
		END_VAR
		{wasm 'i32.reinterpret_f32' (params value) (result v)}
		PUT_DWORD_LE(buf, pos, v);
	END_FUNCTION

	FUNCTION PUT_LREAL_LE
		VAR_IN_OUT
			buf: ARRAY[*] OF BYTE;
		END_VAR
		VAR_INPUT
			pos: DINT;
			value: LREAL;
		END_VAR
		VAR
			v: LWORD;
		END_VAR
		{wasm 'i64.reinterpret_f64' (params value) (result v)}
		PUT_LWORD_LE(buf, pos, v);
	END_FUNCTION

	NAMESPACE Test
		USING Std.Unit

		{test}
		FUNCTION test_bounds_of_one_dimension
			VAR
				a: ARRAY[1..10] OF INT;
			END_VAR
			ASSERT(LOWER_BOUND(a, 1) = 1 AND UPPER_BOUND(a, 1) = 10);
		END_FUNCTION

		{test}
		FUNCTION test_bounds_of_each_dimension
			VAR
				a: ARRAY[1..20, -2..2] OF INT;
			END_VAR
			ASSERT(LOWER_BOUND(a, 1) = 1 AND UPPER_BOUND(a, 1) = 20);
			ASSERT(LOWER_BOUND(a, 2) = -2 AND UPPER_BOUND(a, 2) = 2);
		END_FUNCTION

		{test}
		FUNCTION test_bounds_by_name
			VAR
				a: ARRAY[5..7] OF STRING[4];
			END_VAR
			ASSERT(LOWER_BOUND(ARR := a, DIM := 1) = 5);
			ASSERT(UPPER_BOUND(DIM := 1, ARR := a) = 7);
		END_FUNCTION

		FUNCTION Sum: DINT
			VAR_IN_OUT
				values: ARRAY[*] OF INT;
			END_VAR
			VAR
				i: DINT;
			END_VAR
			FOR i := LOWER_BOUND(values, 1) TO UPPER_BOUND(values, 1) DO
				Sum := Sum + values[i];
			END_FOR
		END_FUNCTION

		{test}
		FUNCTION test_bounds_of_an_array_any_call_binds
			VAR
				a: ARRAY[1..20, -2..2] OF INT := [20(5(1))];
				b: ARRAY[0..2] OF INT := [1, 2, 3];
			END_VAR
			ASSERT(Sum(a[2]) = 5, 'a row of a');
			ASSERT(Sum(b) = 6, 'b');
		END_FUNCTION

		{test}
		FUNCTION test_get_word_be
			VAR b : ARRAY[0..1] OF BYTE := [16#12, 16#34]; END_VAR
			ASSERT_EQ(GET_WORD_BE(b, 0), WORD#16#1234);
		END_FUNCTION

		{test}
		FUNCTION test_get_word_le
			VAR b : ARRAY[0..1] OF BYTE := [16#12, 16#34]; END_VAR
			ASSERT_EQ(GET_WORD_LE(b, 0), WORD#16#3412);
		END_FUNCTION

		{test}
		FUNCTION test_get_dword_at_a_subscript
			VAR b : ARRAY[1..6] OF BYTE := [16#FF, 16#12, 16#34, 16#56, 16#78, 16#FF]; END_VAR
			ASSERT_EQ(GET_DWORD_BE(b, 2), DWORD#16#12345678);
		END_FUNCTION

		{test}
		FUNCTION test_put_dword_be
			VAR b : ARRAY[0..3] OF BYTE; END_VAR
			PUT_DWORD_BE(b, 0, DWORD#16#12345678);
			ASSERT_EQ(b[0], BYTE#16#12);
			ASSERT_EQ(b[3], BYTE#16#78);
		END_FUNCTION

		{test}
		FUNCTION test_put_dword_le
			VAR b : ARRAY[0..3] OF BYTE; END_VAR
			PUT_DWORD_LE(b, 0, DWORD#16#12345678);
			ASSERT_EQ(b[0], BYTE#16#78);
			ASSERT_EQ(b[3], BYTE#16#12);
		END_FUNCTION

		{test}
		FUNCTION test_lword_round_trip
			VAR b : ARRAY[0..9] OF BYTE; END_VAR
			PUT_LWORD_BE(b, 1, LWORD#16#0102030405060708);
			ASSERT_EQ(b[1], BYTE#16#01);
			ASSERT_EQ(b[8], BYTE#16#08);
			ASSERT_EQ(GET_LWORD_BE(b, 1), LWORD#16#0102030405060708);
			ASSERT_EQ(GET_LWORD_LE(b, 1), LWORD#16#0807060504030201);
		END_FUNCTION

		{test}
		FUNCTION test_put_real_be
			VAR b : ARRAY[0..3] OF BYTE; END_VAR
			PUT_REAL_BE(b, 0, REAL#1.0);
			ASSERT_EQ(b[0], BYTE#16#3F);
			ASSERT_EQ(b[1], BYTE#16#80);
			ASSERT_EQ(GET_REAL_BE(b, 0), REAL#1.0);
		END_FUNCTION

		{test}
		FUNCTION test_real_le_round_trip
			VAR b : ARRAY[0..3] OF BYTE; END_VAR
			PUT_REAL_LE(b, 0, REAL#-2.5);
			ASSERT_EQ(b[3], BYTE#16#C0);
			ASSERT_EQ(GET_REAL_LE(b, 0), REAL#-2.5);
		END_FUNCTION

		{test}
		FUNCTION test_put_lreal_be
			VAR b : ARRAY[0..7] OF BYTE; END_VAR
			PUT_LREAL_BE(b, 0, LREAL#1.5);
			ASSERT_EQ(b[0], BYTE#16#3F);
			ASSERT_EQ(b[1], BYTE#16#F8);
			ASSERT_EQ(GET_LREAL_BE(b, 0), LREAL#1.5);
		END_FUNCTION

		{test}
		FUNCTION test_lreal_le_round_trip
			VAR b : ARRAY[0..7] OF BYTE; END_VAR
			PUT_LREAL_LE(b, 0, LREAL#1.5);
			ASSERT_EQ(b[7], BYTE#16#3F);
			ASSERT_EQ(GET_LREAL_LE(b, 0), LREAL#1.5);
		END_FUNCTION
	END_NAMESPACE

END_NAMESPACE
