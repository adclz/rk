/*
# Standard Bit Shift Functions

IEC 61131-3 **Table 30** - Bit shift and rotate operations.

Each function takes a bit-type input `IN` and an integer shift count `N`.
The result type matches the input type.
*/
NAMESPACE Std.Bits

	/*
    ## SHL - Shift Left

    Shifts `IN` left by `N` bit positions, filling with zeros.

    - **IN**: `ANY_BIT` (BOOL, BYTE, WORD, DWORD, LWORD)
    - **N**: `ANY_INT` (shift count)

    ```iecst
    result := SHL(16#0F, 4); // 16#F0
    result := SHL(16#01, 8); // 16#0100
    ```
    */
	FUNCTION SHL: ANY_BIT
		VAR_INPUT
			IN: INTO(SHL);
			N: INT;
		END_VAR
		{wasm IN 'shl' (params IN N) (result SHL)}
	END_FUNCTION

	
	/*
    ## SHR - Shift Right

    Shifts `IN` right by `N` bit positions, filling with zeros.

    - **IN**: `ANY_BIT` (BOOL, BYTE, WORD, DWORD, LWORD)
    - **N**: `ANY_INT` (shift count)

    ```iecst
    result := SHR(16#F0, 4); // 16#0F
    ```
    */
	FUNCTION SHR: ANY_BIT
		VAR_INPUT
			IN: INTO(SHR);
			N: INT;
		END_VAR
		{wasm IN 'shr_u' (params IN N) (result SHR)}
	END_FUNCTION

	
	/*
    ## ROL - Rotate Left

    Rotates `IN` left by `N` bit positions.
    Bits shifted out on the left re-enter on the right.

    - **IN**: `ANY_BIT` (BOOL, BYTE, WORD, DWORD, LWORD)
    - **N**: `ANY_INT` (rotate count)

    ```iecst
    result := ROL(16#81, 1); // 16#03 (BYTE)
    ```
    */
	FUNCTION ROL: ANY_BIT
		VAR_INPUT
			IN: INTO(ROL);
			N: INT;
		END_VAR
		{wasm IN 'rotl' (params IN N) (result ROL)}
	END_FUNCTION

	
	/*
    ## ROR - Rotate Right

    Rotates `IN` right by `N` bit positions.
    Bits shifted out on the right re-enter on the left.

    - **IN**: `ANY_BIT` (BOOL, BYTE, WORD, DWORD, LWORD)
    - **N**: `ANY_INT` (rotate count)

    ```iecst
    result := ROR(16#81, 1); // 16#C0 (BYTE)
    ```
    */
	FUNCTION ROR: ANY_BIT
		VAR_INPUT
			IN: INTO(ROR);
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
		FUNCTION test_shl_word ASSERT_EQ(SHL(WORD#16#0001, 8), WORD#16#0100);
		END_FUNCTION

		{test}
		FUNCTION test_shr_byte ASSERT_EQ(SHR(BYTE#16#F0, 4), BYTE#16#0F);
		END_FUNCTION

		{test}
		FUNCTION test_shr_word ASSERT_EQ(SHR(WORD#16#0100, 8), WORD#16#0001);
		END_FUNCTION

		{test}
		FUNCTION test_rol_dword ASSERT_EQ(ROL(DWORD#1, 4), DWORD#16);
		END_FUNCTION

		{test}
		FUNCTION test_ror_dword
			// ROR(16, 4) = 1
			ASSERT_EQ(ROR(DWORD#16, 4), DWORD#1);
		END_FUNCTION

	
	END_NAMESPACE


END_NAMESPACE
