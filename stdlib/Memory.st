/*
# Standard Memory Functions

*/
NAMESPACE Std.Memory

	/*
    ## MOVE - Memory Copy

    Copies the input value to the output.
    One overload per elementary type.
    */
	FUNCTION MOVE
		VAR_INPUT
			IN: BOOL;
		END_VAR
		VAR_OUTPUT
			OUT: BOOL;
		END_VAR

		OUT := IN;
	END_FUNCTION

	FUNCTION MOVE
		VAR_INPUT
			IN: SINT;
		END_VAR
		VAR_OUTPUT
			OUT: SINT;
		END_VAR

		OUT := IN;
	END_FUNCTION

	FUNCTION MOVE
		VAR_INPUT
			IN: INT;
		END_VAR
		VAR_OUTPUT
			OUT: INT;
		END_VAR

		OUT := IN;
	END_FUNCTION

	FUNCTION MOVE
		VAR_INPUT
			IN: DINT;
		END_VAR
		VAR_OUTPUT
			OUT: DINT;
		END_VAR

		OUT := IN;
	END_FUNCTION

	FUNCTION MOVE
		VAR_INPUT
			IN: LINT;
		END_VAR
		VAR_OUTPUT
			OUT: LINT;
		END_VAR

		OUT := IN;
	END_FUNCTION

	FUNCTION MOVE
		VAR_INPUT
			IN: USINT;
		END_VAR
		VAR_OUTPUT
			OUT: USINT;
		END_VAR

		OUT := IN;
	END_FUNCTION

	FUNCTION MOVE
		VAR_INPUT
			IN: UINT;
		END_VAR
		VAR_OUTPUT
			OUT: UINT;
		END_VAR

		OUT := IN;
	END_FUNCTION

	FUNCTION MOVE
		VAR_INPUT
			IN: UDINT;
		END_VAR
		VAR_OUTPUT
			OUT: UDINT;
		END_VAR

		OUT := IN;
	END_FUNCTION

	FUNCTION MOVE
		VAR_INPUT
			IN: ULINT;
		END_VAR
		VAR_OUTPUT
			OUT: ULINT;
		END_VAR

		OUT := IN;
	END_FUNCTION

	FUNCTION MOVE
		VAR_INPUT
			IN: BYTE;
		END_VAR
		VAR_OUTPUT
			OUT: BYTE;
		END_VAR

		OUT := IN;
	END_FUNCTION

	FUNCTION MOVE
		VAR_INPUT
			IN: WORD;
		END_VAR
		VAR_OUTPUT
			OUT: WORD;
		END_VAR

		OUT := IN;
	END_FUNCTION

	FUNCTION MOVE
		VAR_INPUT
			IN: DWORD;
		END_VAR
		VAR_OUTPUT
			OUT: DWORD;
		END_VAR

		OUT := IN;
	END_FUNCTION

	FUNCTION MOVE
		VAR_INPUT
			IN: LWORD;
		END_VAR
		VAR_OUTPUT
			OUT: LWORD;
		END_VAR

		OUT := IN;
	END_FUNCTION

	FUNCTION MOVE
		VAR_INPUT
			IN: REAL;
		END_VAR
		VAR_OUTPUT
			OUT: REAL;
		END_VAR

		OUT := IN;
	END_FUNCTION

	FUNCTION MOVE
		VAR_INPUT
			IN: LREAL;
		END_VAR
		VAR_OUTPUT
			OUT: LREAL;
		END_VAR

		OUT := IN;
	END_FUNCTION

	FUNCTION MOVE
		VAR_INPUT
			IN: TIME;
		END_VAR
		VAR_OUTPUT
			OUT: TIME;
		END_VAR

		OUT := IN;
	END_FUNCTION

	FUNCTION MOVE
		VAR_INPUT
			IN: LTIME;
		END_VAR
		VAR_OUTPUT
			OUT: LTIME;
		END_VAR

		OUT := IN;
	END_FUNCTION

	FUNCTION MOVE
		VAR_INPUT
			IN: STRING;
		END_VAR
		VAR_OUTPUT
			OUT: STRING;
		END_VAR

		OUT := IN;
	END_FUNCTION

	FUNCTION MOVE
		VAR_INPUT
			IN: CHAR;
		END_VAR
		VAR_OUTPUT
			OUT: CHAR;
		END_VAR

		OUT := IN;
	END_FUNCTION

	NAMESPACE Test
		USING Std.Unit
		USING Std.Memory
		USING Std.Strings

		{test}
		FUNCTION test_move_int
			VAR
				x: INT := 0;
			END_VAR
			MOVE(42, OUT => x);
			ASSERT_EQ(x, 42);
		END_FUNCTION

		{test}
		FUNCTION test_move_real
			VAR
				x: REAL := 0.0;
			END_VAR
			MOVE(3.14, OUT => x);
			ASSERT_EQ(x, 3.14);
		END_FUNCTION

		{test}
		FUNCTION test_move_bool
			VAR
				x: BOOL := FALSE;
			END_VAR
			MOVE(TRUE, OUT => x);
			ASSERT(x);
		END_FUNCTION

		// --- every overload is independent compiled code: cover the 16 dark
		// ones, each with a boundary value for its type. The i64 lane
		// (LWORD/LINT/ULINT/LREAL/LTIME) is the historically bug-prone path ---

		{test}
		FUNCTION test_move_signed_boundaries
			VAR
				s: SINT;
				d: DINT;
			END_VAR
			MOVE(SINT#-128, OUT => s);
			ASSERT_EQ(s, SINT#-128, 'MOVE SINT at MIN');
			MOVE(DINT#-2147483648, OUT => d);
			ASSERT_EQ(d, DINT#-2147483648, 'MOVE DINT at MIN');
		END_FUNCTION

		{test}
		FUNCTION test_move_unsigned_boundaries
			VAR
				us: USINT;
				u: UINT;
				ud: UDINT;
			END_VAR
			MOVE(USINT#255, OUT => us);
			ASSERT_EQ(us, USINT#255, 'MOVE USINT at MAX');
			MOVE(UINT#65535, OUT => u);
			ASSERT_EQ(u, UINT#65535, 'MOVE UINT at MAX');
			MOVE(UDINT#4294967295, OUT => ud);
			ASSERT_EQ(ud, UDINT#4294967295, 'MOVE UDINT at MAX (above 2^31)');
		END_FUNCTION

		{test}
		FUNCTION test_move_bit_types_sign_bit
			VAR
				b: BYTE;
				w: WORD;
				dw: DWORD;
			END_VAR
			MOVE(BYTE#16#80, OUT => b);
			ASSERT_EQ(b, BYTE#16#80, 'MOVE BYTE with MSB set');
			MOVE(WORD#16#8000, OUT => w);
			ASSERT_EQ(w, WORD#16#8000, 'MOVE WORD with MSB set');
			MOVE(DWORD#16#80000000, OUT => dw);
			ASSERT_EQ(dw, DWORD#16#80000000, 'MOVE DWORD with sign bit set');
		END_FUNCTION

		{test}
		FUNCTION test_move_i64_lane
			VAR
				lw: LWORD;
				li: LINT;
				ul: ULINT;
				lr: LREAL;
			END_VAR
			MOVE(LWORD#16#8000_0000_0000_0000, OUT => lw);
			ASSERT_EQ(lw, LWORD#16#8000_0000_0000_0000, 'MOVE LWORD sign bit');
			MOVE(LINT#-9223372036854775807, OUT => li);
			ASSERT_EQ(li, LINT#-9223372036854775807, 'MOVE LINT near i64 MIN');
			MOVE(ULINT#18446744073709551615, OUT => ul);
			ASSERT_EQ(ul, ULINT#18446744073709551615, 'MOVE ULINT at u64 MAX');
			MOVE(LREAL#1.7976931348623157, OUT => lr);
			ASSERT_EQ(lr, LREAL#1.7976931348623157, 'MOVE LREAL f64 payload');
		END_FUNCTION

		{test}
		FUNCTION test_move_time_types
			VAR
				t: TIME;
				lt: LTIME;
			END_VAR
			MOVE(T#24d20h31m23s647ms, OUT => t);
			ASSERT(t = T#24d20h31m23s647ms, 'MOVE TIME at the signed-ms ceiling');
			MOVE(LTIME#1000d, OUT => lt);
			ASSERT(lt = LTIME#1000d, 'MOVE LTIME on the i64-ns lane');
		END_FUNCTION

		{test}
		FUNCTION test_move_char
			VAR
				c: CHAR;
			END_VAR
			MOVE(CHAR#'z', OUT => c);
			ASSERT_EQ(c, CHAR#'z', 'MOVE CHAR');
		END_FUNCTION

		{test}
		FUNCTION test_move_string
			VAR
				s: STRING;
			END_VAR
			MOVE('edge case', OUT => s);
			ASSERT(LEN(s) = 9, 'MOVE STRING length');
			ASSERT(FIND(s, 'edge case') = 1, 'MOVE STRING content');
		END_FUNCTION
	END_NAMESPACE
END_NAMESPACE
