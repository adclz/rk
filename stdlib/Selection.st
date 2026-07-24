/*
# Standard Selection Functions

IEC 61131-3 **Table 28** - Selection and comparison operations.

Binary selection (`SEL`) and bounding (`LIMIT`).
*/
NAMESPACE Std.Selection

	/*
    ## SEL - Binary Selection

    Selects one of two inputs based on a boolean selector.

    - `G` = **FALSE** returns `IN0`
    - `G` = **TRUE** returns `IN1`

    Provided as one overload per value type.

    ```iecst
    result := SEL(G := TRUE, IN0 := 0, IN1 := 42); // 42
    result := SEL(G := FALSE, IN0 := 0, IN1 := 42); // 0
    ```
    */
	FUNCTION SEL: BOOL
		VAR_INPUT
			G: BOOL;
			IN0: BOOL;
			IN1: BOOL;
		END_VAR

		IF G THEN
			SEL := IN1;
		ELSE
			SEL := IN0;
		END_IF;
	END_FUNCTION

	FUNCTION SEL: SINT
		VAR_INPUT
			G: BOOL;
			IN0: SINT;
			IN1: SINT;
		END_VAR

		IF G THEN
			SEL := IN1;
		ELSE
			SEL := IN0;
		END_IF;
	END_FUNCTION

	FUNCTION SEL: INT
		VAR_INPUT
			G: BOOL;
			IN0: INT;
			IN1: INT;
		END_VAR

		IF G THEN
			SEL := IN1;
		ELSE
			SEL := IN0;
		END_IF;
	END_FUNCTION

	FUNCTION SEL: DINT
		VAR_INPUT
			G: BOOL;
			IN0: DINT;
			IN1: DINT;
		END_VAR

		IF G THEN
			SEL := IN1;
		ELSE
			SEL := IN0;
		END_IF;
	END_FUNCTION

	FUNCTION SEL: LINT
		VAR_INPUT
			G: BOOL;
			IN0: LINT;
			IN1: LINT;
		END_VAR

		IF G THEN
			SEL := IN1;
		ELSE
			SEL := IN0;
		END_IF;
	END_FUNCTION

	FUNCTION SEL: USINT
		VAR_INPUT
			G: BOOL;
			IN0: USINT;
			IN1: USINT;
		END_VAR

		IF G THEN
			SEL := IN1;
		ELSE
			SEL := IN0;
		END_IF;
	END_FUNCTION

	FUNCTION SEL: UINT
		VAR_INPUT
			G: BOOL;
			IN0: UINT;
			IN1: UINT;
		END_VAR

		IF G THEN
			SEL := IN1;
		ELSE
			SEL := IN0;
		END_IF;
	END_FUNCTION

	FUNCTION SEL: UDINT
		VAR_INPUT
			G: BOOL;
			IN0: UDINT;
			IN1: UDINT;
		END_VAR

		IF G THEN
			SEL := IN1;
		ELSE
			SEL := IN0;
		END_IF;
	END_FUNCTION

	FUNCTION SEL: ULINT
		VAR_INPUT
			G: BOOL;
			IN0: ULINT;
			IN1: ULINT;
		END_VAR

		IF G THEN
			SEL := IN1;
		ELSE
			SEL := IN0;
		END_IF;
	END_FUNCTION

	FUNCTION SEL: BYTE
		VAR_INPUT
			G: BOOL;
			IN0: BYTE;
			IN1: BYTE;
		END_VAR

		IF G THEN
			SEL := IN1;
		ELSE
			SEL := IN0;
		END_IF;
	END_FUNCTION

	FUNCTION SEL: WORD
		VAR_INPUT
			G: BOOL;
			IN0: WORD;
			IN1: WORD;
		END_VAR

		IF G THEN
			SEL := IN1;
		ELSE
			SEL := IN0;
		END_IF;
	END_FUNCTION

	FUNCTION SEL: DWORD
		VAR_INPUT
			G: BOOL;
			IN0: DWORD;
			IN1: DWORD;
		END_VAR

		IF G THEN
			SEL := IN1;
		ELSE
			SEL := IN0;
		END_IF;
	END_FUNCTION

	FUNCTION SEL: LWORD
		VAR_INPUT
			G: BOOL;
			IN0: LWORD;
			IN1: LWORD;
		END_VAR

		IF G THEN
			SEL := IN1;
		ELSE
			SEL := IN0;
		END_IF;
	END_FUNCTION

	FUNCTION SEL: REAL
		VAR_INPUT
			G: BOOL;
			IN0: REAL;
			IN1: REAL;
		END_VAR

		IF G THEN
			SEL := IN1;
		ELSE
			SEL := IN0;
		END_IF;
	END_FUNCTION

	FUNCTION SEL: LREAL
		VAR_INPUT
			G: BOOL;
			IN0: LREAL;
			IN1: LREAL;
		END_VAR

		IF G THEN
			SEL := IN1;
		ELSE
			SEL := IN0;
		END_IF;
	END_FUNCTION

	FUNCTION SEL: TIME
		VAR_INPUT
			G: BOOL;
			IN0: TIME;
			IN1: TIME;
		END_VAR

		IF G THEN
			SEL := IN1;
		ELSE
			SEL := IN0;
		END_IF;
	END_FUNCTION

	FUNCTION SEL: STRING
		VAR_INPUT
			G: BOOL;
			IN0: STRING;
			IN1: STRING;
		END_VAR

		IF G THEN
			SEL := IN1;
		ELSE
			SEL := IN0;
		END_IF;
	END_FUNCTION

	/*
    ## LIMIT - Bounding

    Clamps `IN` to the range [`MN`, `MX`].

    - Returns `MN` if `IN < MN`
    - Returns `MX` if `IN > MX`
    - Returns `IN` otherwise

    Provided as one overload per comparable numeric type.

    ```iecst
    result := LIMIT(MN := 0, 15, MX := 10);  // 10
    result := LIMIT(MN := 0, -5, MX := 10);   // 0
    result := LIMIT(MN := 0, 5, MX := 10);    // 5
    ```
    */
	FUNCTION LIMIT: SINT
		VAR_INPUT
			MN: SINT;
			IN: SINT;
			MX: SINT;
		END_VAR

		IF IN < MN THEN
			LIMIT := MN;
		ELSIF IN > MX THEN
			LIMIT := MX;
		ELSE
			LIMIT := IN;
		END_IF;
	END_FUNCTION

	FUNCTION LIMIT: INT
		VAR_INPUT
			MN: INT;
			IN: INT;
			MX: INT;
		END_VAR

		IF IN < MN THEN
			LIMIT := MN;
		ELSIF IN > MX THEN
			LIMIT := MX;
		ELSE
			LIMIT := IN;
		END_IF;
	END_FUNCTION

	FUNCTION LIMIT: DINT
		VAR_INPUT
			MN: DINT;
			IN: DINT;
			MX: DINT;
		END_VAR

		IF IN < MN THEN
			LIMIT := MN;
		ELSIF IN > MX THEN
			LIMIT := MX;
		ELSE
			LIMIT := IN;
		END_IF;
	END_FUNCTION

	FUNCTION LIMIT: LINT
		VAR_INPUT
			MN: LINT;
			IN: LINT;
			MX: LINT;
		END_VAR

		IF IN < MN THEN
			LIMIT := MN;
		ELSIF IN > MX THEN
			LIMIT := MX;
		ELSE
			LIMIT := IN;
		END_IF;
	END_FUNCTION

	FUNCTION LIMIT: USINT
		VAR_INPUT
			MN: USINT;
			IN: USINT;
			MX: USINT;
		END_VAR

		IF IN < MN THEN
			LIMIT := MN;
		ELSIF IN > MX THEN
			LIMIT := MX;
		ELSE
			LIMIT := IN;
		END_IF;
	END_FUNCTION

	FUNCTION LIMIT: UINT
		VAR_INPUT
			MN: UINT;
			IN: UINT;
			MX: UINT;
		END_VAR

		IF IN < MN THEN
			LIMIT := MN;
		ELSIF IN > MX THEN
			LIMIT := MX;
		ELSE
			LIMIT := IN;
		END_IF;
	END_FUNCTION

	FUNCTION LIMIT: UDINT
		VAR_INPUT
			MN: UDINT;
			IN: UDINT;
			MX: UDINT;
		END_VAR

		IF IN < MN THEN
			LIMIT := MN;
		ELSIF IN > MX THEN
			LIMIT := MX;
		ELSE
			LIMIT := IN;
		END_IF;
	END_FUNCTION

	FUNCTION LIMIT: ULINT
		VAR_INPUT
			MN: ULINT;
			IN: ULINT;
			MX: ULINT;
		END_VAR

		IF IN < MN THEN
			LIMIT := MN;
		ELSIF IN > MX THEN
			LIMIT := MX;
		ELSE
			LIMIT := IN;
		END_IF;
	END_FUNCTION

	FUNCTION LIMIT: REAL
		VAR_INPUT
			MN: REAL;
			IN: REAL;
			MX: REAL;
		END_VAR

		IF IN < MN THEN
			LIMIT := MN;
		ELSIF IN > MX THEN
			LIMIT := MX;
		ELSE
			LIMIT := IN;
		END_IF;
	END_FUNCTION

	FUNCTION LIMIT: LREAL
		VAR_INPUT
			MN: LREAL;
			IN: LREAL;
			MX: LREAL;
		END_VAR

		IF IN < MN THEN
			LIMIT := MN;
		ELSIF IN > MX THEN
			LIMIT := MX;
		ELSE
			LIMIT := IN;
		END_IF;
	END_FUNCTION

	FUNCTION LIMIT: TIME
		VAR_INPUT
			MN: TIME;
			IN: TIME;
			MX: TIME;
		END_VAR

		IF IN < MN THEN
			LIMIT := MN;
		ELSIF IN > MX THEN
			LIMIT := MX;
		ELSE
			LIMIT := IN;
		END_IF;
	END_FUNCTION

	FUNCTION LIMIT: LTIME
		VAR_INPUT
			MN: LTIME;
			IN: LTIME;
			MX: LTIME;
		END_VAR

		IF IN < MN THEN
			LIMIT := MN;
		ELSIF IN > MX THEN
			LIMIT := MX;
		ELSE
			LIMIT := IN;
		END_IF;
	END_FUNCTION

	NAMESPACE Test
		USING Std.Unit
		USING Std.Selection

		// SEL tests

		{test}
		FUNCTION test_sel_true_returns_in1 ASSERT_EQ(SEL(TRUE, 10, 42), 42);
		END_FUNCTION

		{test}
		FUNCTION test_sel_false_returns_in0 ASSERT_EQ(SEL(FALSE, 10, 42), 10);
		END_FUNCTION

		{test}
		FUNCTION test_sel_real_true ASSERT_EQ(SEL(TRUE, 1.0, 3.14), 3.14);
		END_FUNCTION

		{test}
		FUNCTION test_sel_real_false ASSERT_EQ(SEL(FALSE, 1.0, 3.14), 1.0);
		END_FUNCTION

		// LIMIT tests

		{test}
		FUNCTION test_limit_in_range ASSERT_EQ(LIMIT(0, 5, 10), 5);
		END_FUNCTION

		{test}
		FUNCTION test_limit_below_min ASSERT_EQ(LIMIT(0, -5, 10), 0);
		END_FUNCTION

		{test}
		FUNCTION test_limit_above_max ASSERT_EQ(LIMIT(0, 15, 10), 10);
		END_FUNCTION

		{test}
		FUNCTION test_limit_at_min ASSERT_EQ(LIMIT(0, 0, 10), 0);
		END_FUNCTION

		{test}
		FUNCTION test_limit_at_max ASSERT_EQ(LIMIT(0, 10, 10), 10);
		END_FUNCTION

		{test}
		FUNCTION test_limit_real_clamp_low ASSERT_EQ(LIMIT(0.0, -1.5, 10.0), 0.0);
		END_FUNCTION

		{test}
		FUNCTION test_limit_real_clamp_high ASSERT_EQ(LIMIT(0.0, 15.5, 10.0), 10.0);
		END_FUNCTION

		{test}
		FUNCTION test_limit_real_in_range ASSERT_EQ(LIMIT(0.0, 5.5, 10.0), 5.5);
		END_FUNCTION
	END_NAMESPACE
END_NAMESPACE
