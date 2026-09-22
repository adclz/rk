/*
Bit shift and rotate functions.

Each takes a `BYTE`, `WORD`, `DWORD` or `LWORD` and a count `N`, and returns
the type it was given. The width is that of the type: a `BYTE` is shifted
and rotated within its 8 bits.
*/
NAMESPACE Std.Bits

	/* Returns `IN` shifted left by `N` bits, filling with zeros.

	   Bits shifted past the top of the type are lost. An `N` of the type's width
	   or more gives 0.

	   # Examples

	   ```iecst
	   b := SHL(BYTE#16#0F, 4);    // BYTE#16#F0
	   b := SHL(BYTE#16#FF, 4);    // BYTE#16#F0
	   w := SHL(WORD#16#0001, 8);  // WORD#16#0100
	   b := SHL(BYTE#16#01, 8);    // BYTE#16#00
	   ```
	*/
	FUNCTION SHL: BYTE
		VAR_INPUT
			IN: BYTE;
			N: INT;
		END_VAR
		{wasm IN 'shl' (params IN N) (result SHL)}
	END_FUNCTION

	FUNCTION SHL: WORD
		VAR_INPUT
			IN: WORD;
			N: INT;
		END_VAR
		{wasm IN 'shl' (params IN N) (result SHL)}
	END_FUNCTION

	FUNCTION SHL: DWORD
		VAR_INPUT
			IN: DWORD;
			N: INT;
		END_VAR
		{wasm IN 'shl' (params IN N) (result SHL)}
	END_FUNCTION

	FUNCTION SHL: LWORD
		VAR_INPUT
			IN: LWORD;
			N: INT;
		END_VAR
		{wasm IN 'shl' (params IN N) (result SHL)}
	END_FUNCTION

	/* Returns `IN` shifted right by `N` bits, filling with zeros.

	   An `N` of the type's width or more gives 0.

	   # Examples

	   ```iecst
	   b := SHR(BYTE#16#F0, 4);         // BYTE#16#0F
	   d := SHR(DWORD#16#80000000, 31);  // DWORD#1
	   ```
	*/
	FUNCTION SHR: BYTE
		VAR_INPUT
			IN: BYTE;
			N: INT;
		END_VAR
		{wasm IN 'shr_u' (params IN N) (result SHR)}
	END_FUNCTION

	FUNCTION SHR: WORD
		VAR_INPUT
			IN: WORD;
			N: INT;
		END_VAR
		{wasm IN 'shr_u' (params IN N) (result SHR)}
	END_FUNCTION

	FUNCTION SHR: DWORD
		VAR_INPUT
			IN: DWORD;
			N: INT;
		END_VAR
		{wasm IN 'shr_u' (params IN N) (result SHR)}
	END_FUNCTION

	FUNCTION SHR: LWORD
		VAR_INPUT
			IN: LWORD;
			N: INT;
		END_VAR
		{wasm IN 'shr_u' (params IN N) (result SHR)}
	END_FUNCTION

	/* Returns `IN` rotated left by `N` bits.

	   Bits that leave at the top come back at the bottom. `N` wraps at the type's
	   width, so rotating a `BYTE` by 8 returns it unchanged.

	   # Examples

	   ```iecst
	   b := ROL(BYTE#16#81, 1);  // BYTE#16#03
	   b := ROL(BYTE#16#81, 8);  // BYTE#16#81
	   ```
	*/
	FUNCTION ROL: BYTE
		VAR_INPUT
			IN: BYTE;
			N: INT;
		END_VAR
		{wasm IN 'rotl' (params IN N) (result ROL)}
	END_FUNCTION

	FUNCTION ROL: WORD
		VAR_INPUT
			IN: WORD;
			N: INT;
		END_VAR
		{wasm IN 'rotl' (params IN N) (result ROL)}
	END_FUNCTION

	FUNCTION ROL: DWORD
		VAR_INPUT
			IN: DWORD;
			N: INT;
		END_VAR
		{wasm IN 'rotl' (params IN N) (result ROL)}
	END_FUNCTION

	FUNCTION ROL: LWORD
		VAR_INPUT
			IN: LWORD;
			N: INT;
		END_VAR
		{wasm IN 'rotl' (params IN N) (result ROL)}
	END_FUNCTION

	/* Returns `IN` rotated right by `N` bits.

	   Bits that leave at the bottom come back at the top. `N` wraps at the type's
	   width.

	   # Examples

	   ```iecst
	   b := ROR(BYTE#16#01, 1);  // BYTE#16#80
	   w := ROR(WORD#16#0001, 1);  // WORD#16#8000
	   ```
	*/
	FUNCTION ROR: BYTE
		VAR_INPUT
			IN: BYTE;
			N: INT;
		END_VAR
		{wasm IN 'rotr' (params IN N) (result ROR)}
	END_FUNCTION

	FUNCTION ROR: WORD
		VAR_INPUT
			IN: WORD;
			N: INT;
		END_VAR
		{wasm IN 'rotr' (params IN N) (result ROR)}
	END_FUNCTION

	FUNCTION ROR: DWORD
		VAR_INPUT
			IN: DWORD;
			N: INT;
		END_VAR
		{wasm IN 'rotr' (params IN N) (result ROR)}
	END_FUNCTION

	FUNCTION ROR: LWORD
		VAR_INPUT
			IN: LWORD;
			N: INT;
		END_VAR
		{wasm IN 'rotr' (params IN N) (result ROR)}
	END_FUNCTION

	NAMESPACE Test
		USING Std.Unit

		{test}
		FUNCTION test_shl_byte ASSERT_EQ(SHL(BYTE#16#0F, 4), BYTE#16#F0);
		END_FUNCTION

		{test}
		FUNCTION test_shr_byte ASSERT_EQ(SHR(BYTE#16#F0, 4), BYTE#16#0F);
		END_FUNCTION

		{test}
		FUNCTION test_shl_word ASSERT_EQ(SHL(WORD#16#0001, 8), WORD#16#0100);
		END_FUNCTION

		{test}
		FUNCTION test_shr_word ASSERT_EQ(SHR(WORD#16#0100, 8), WORD#16#0001);
		END_FUNCTION

		{test}
		FUNCTION test_rol_dword ASSERT_EQ(ROL(DWORD#1, 4), DWORD#16);
		END_FUNCTION

		{test}
		FUNCTION test_ror_dword ASSERT_EQ(ROR(DWORD#16, 4), DWORD#1);
		END_FUNCTION

		// --- edge cases ---

		// Shift/rotate by 0 is the identity, independent of storage width.
		{test}
		FUNCTION test_shl_by_zero ASSERT_EQ(SHL(BYTE#16#0F, 0), BYTE#16#0F);
		END_FUNCTION

		{test}
		FUNCTION test_shr_by_zero ASSERT_EQ(SHR(WORD#16#00F0, 0), WORD#16#00F0);
		END_FUNCTION

		{test}
		FUNCTION test_rol_by_zero ASSERT_EQ(ROL(DWORD#16#12345678, 0), DWORD#16#12345678);
		END_FUNCTION

		{test}
		FUNCTION test_ror_by_zero ASSERT_EQ(ROR(DWORD#16#12345678, 0), DWORD#16#12345678);
		END_FUNCTION

		// DWORD is i32-backed, so rotate wraps correctly at 32 bits:
		// the top bit rotated left re-enters at bit 0.
		{test}
		FUNCTION test_rol_dword_wraps_msb ASSERT_EQ(ROL(DWORD#16#80000000, 1), DWORD#1);
		END_FUNCTION

		{test}
		FUNCTION test_ror_dword_wraps_lsb ASSERT_EQ(ROR(DWORD#1, 1), DWORD#16#80000000);
		END_FUNCTION

		// Logical shift right fills with zeros (no sign extension) on a bit type.
		{test}
		FUNCTION test_shr_dword_top_bit ASSERT_EQ(SHR(DWORD#16#80000000, 31), DWORD#1);
		END_FUNCTION

		// --- sub-width masking: shifted-out bits must not leak past the type width ---

		{test}
		FUNCTION test_shl_byte_masks_leak
			ASSERT_EQ(SHL(BYTE#16#FF, 4), BYTE#16#F0, 'SHL BYTE must mask bits above bit 7');
		END_FUNCTION

		{test}
		FUNCTION test_shl_word_masks_leak
			ASSERT_EQ(SHL(WORD#16#FFFF, 8), WORD#16#FF00, 'SHL WORD must mask bits above bit 15');
		END_FUNCTION

		{test}
		FUNCTION test_shl_byte_by_type_width
			ASSERT_EQ(SHL(BYTE#16#01, 8), BYTE#16#00, 'shift by the full type width empties the value');
		END_FUNCTION

		// --- shift count >= width yields 0 (wasm masks the count mod the lane
		// width, so a raw lowering would return the input unchanged) ---

		{test}
		FUNCTION test_shl_dword_by_32
			ASSERT_EQ(SHL(DWORD#1, 32), DWORD#0, 'SHL DWORD by 32 is 0, not a wasm count-mask no-op');
		END_FUNCTION

		{test}
		FUNCTION test_shr_dword_by_32
			ASSERT_EQ(SHR(DWORD#16#80000000, 32), DWORD#0, 'SHR DWORD by 32 is 0');
		END_FUNCTION

		{test}
		FUNCTION test_shr_word_by_32
			ASSERT_EQ(SHR(WORD#16#0001, 32), WORD#0, 'SHR WORD by 32 hits the wasm lane mask');
		END_FUNCTION

		// --- sub-width rotates wrap at the type MSB/LSB, not bit 31 ---

		{test}
		FUNCTION test_rol_byte_wraps_msb
			// The doc example: the wrapped bit re-enters at bit 0, not bit 8.
			ASSERT_EQ(ROL(BYTE#16#81, 1), BYTE#16#03, 'ROL BYTE MSB wraps to bit 0');
		END_FUNCTION

		{test}
		FUNCTION test_ror_byte_wraps_lsb
			ASSERT_EQ(ROR(BYTE#16#01, 1), BYTE#16#80, 'ROR BYTE LSB wraps to bit 7, not bit 31');
		END_FUNCTION

		{test}
		FUNCTION test_rol_word_wraps_msb
			ASSERT_EQ(ROL(WORD#16#8001, 1), WORD#16#0003, 'ROL WORD bit 15 wraps to bit 0');
		END_FUNCTION

		{test}
		FUNCTION test_ror_word_wraps_lsb
			ASSERT_EQ(ROR(WORD#16#0001, 1), WORD#16#8000, 'ROR WORD LSB wraps to bit 15');
		END_FUNCTION

		// Rotate counts are mod the type width.
		{test}
		FUNCTION test_rol_byte_count_mod_width
			ASSERT_EQ(ROL(BYTE#16#81, 0), BYTE#16#81, 'ROL by 0 is identity');
			ASSERT_EQ(ROL(BYTE#16#81, 8), BYTE#16#81, 'ROL by the type width is identity');
			ASSERT_EQ(ROL(BYTE#16#81, 9), BYTE#16#03, 'ROL by 9 = ROL by 1 (mod 8)');
		END_FUNCTION

		// --- LWORD (i64 lane): value, count extension, and 64-bit wraps ---

		{test}
		FUNCTION test_shl_lword_crosses_32
			ASSERT_EQ(SHL(LWORD#1, 32), LWORD#16#0000_0001_0000_0000, 'SHL LWORD must cross the 32-bit line');
		END_FUNCTION

		{test}
		FUNCTION test_shl_lword_by_64
			ASSERT_EQ(SHL(LWORD#1, 64), LWORD#0, 'SHL LWORD by 64 is 0');
		END_FUNCTION

		{test}
		FUNCTION test_shr_lword_top_bit
			ASSERT_EQ(SHR(LWORD#16#8000_0000_0000_0000, 63), LWORD#1, 'logical shift on the i64 lane');
		END_FUNCTION

		{test}
		FUNCTION test_rol_lword_wraps_msb
			ASSERT_EQ(ROL(LWORD#16#8000_0000_0000_0000, 1), LWORD#1, '64-bit MSB wraps to bit 0');
		END_FUNCTION

		{test}
		FUNCTION test_ror_lword_wraps_lsb
			ASSERT_EQ(ROR(LWORD#1, 1), LWORD#16#8000_0000_0000_0000, '64-bit LSB wraps to bit 63');
		END_FUNCTION

		{test}
		FUNCTION test_shr_byte_after_shl_roundtrip
			// SHR on a sub-width type relies on the masked-value invariant.
			ASSERT_EQ(SHR(SHL(BYTE#16#FF, 4), 4), BYTE#16#0F, 'SHL then SHR keeps only surviving bits');
		END_FUNCTION
	END_NAMESPACE
END_NAMESPACE
