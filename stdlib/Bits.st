/*
# Standard Bit Shift Functions

IEC 61131-3 **Table 30** - Bit shift and rotate operations.

Each function takes a bit-type input `IN` and an integer shift count `N`.
The result type matches the input type.

Each operation is provided as one overload per bit type
(BYTE, WORD, DWORD, LWORD).
*/
NAMESPACE Std.Bits

	/*
    ## SHL - Shift Left

    Shifts `IN` left by `N` bit positions, filling with zeros.

    - **IN**: `BYTE`, `WORD`, `DWORD`, `LWORD`
    - **N**: `INT` (shift count)

    ```iecst
    result := SHL(16#0F, 4); // 16#F0
    result := SHL(16#01, 8); // 16#0100
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

	/*
    ## SHR - Shift Right

    Shifts `IN` right by `N` bit positions, filling with zeros.

    - **IN**: `BYTE`, `WORD`, `DWORD`, `LWORD`
    - **N**: `INT` (shift count)

    ```iecst
    result := SHR(16#F0, 4); // 16#0F
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

	/*
    ## ROL - Rotate Left

    Rotates `IN` left by `N` bit positions.
    Bits shifted out on the left re-enter on the right.

    - **IN**: `BYTE`, `WORD`, `DWORD`, `LWORD`
    - **N**: `INT` (rotate count)

    ```iecst
    result := ROL(16#81, 1); // 16#03 (BYTE)
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

	/*
    ## ROR - Rotate Right

    Rotates `IN` right by `N` bit positions.
    Bits shifted out on the right re-enter on the left.

    - **IN**: `BYTE`, `WORD`, `DWORD`, `LWORD`
    - **N**: `INT` (rotate count)

    ```iecst
    result := ROR(16#81, 1); // 16#C0 (BYTE)
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
	END_NAMESPACE
END_NAMESPACE
