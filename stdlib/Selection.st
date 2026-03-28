/*
# Standard Selection Functions

IEC 61131-3 **Table 28** - Selection and comparison operations.

Binary selection (`SEL`), extremum (`MAX`, `MIN`), and bounding (`LIMIT`).
*/
NAMESPACE Std.Selection

	/*
    ## SEL - Binary Selection

    Selects one of two inputs based on a boolean selector.

    - `G` = **FALSE** returns `IN0`
    - `G` = **TRUE** returns `IN1`

    ```iecst
    result := SEL(G := TRUE, IN0 := 0, IN1 := 42); // 42
    result := SEL(G := FALSE, IN0 := 0, IN1 := 42); // 0
    ```
    */
	FUNCTION SEL: ANY
		VAR_INPUT
			G: BOOL;
			IN0: INTO(SEL);
			IN1: INTO(SEL);
		END_VAR

		IF G THEN
			SEL := IN1;
		ELSE
			SEL := IN0;
		END_IF;
	END_FUNCTION

	/*
    ## MAX - Maximum

    Returns the largest of all input arguments.

    - **Constraint**: `ANY_ELEMENTARY`

    ```iecst
    result := MAX(1, 5, 3);        // 5
    result := MAX(1.0, 2.5, 0.5);  // 2.5
    ```
    */
	FUNCTION MAX: ANY_ELEMENTARY
		VAR_INPUT
			args: INTO(MAX)...
		END_VAR
	END_FUNCTION

	/*
    ## MIN - Minimum

    Returns the smallest of all input arguments.

    - **Constraint**: `ANY_ELEMENTARY`

    ```iecst
    result := MIN(1, 5, 3);        // 1
    result := MIN(1.0, 2.5, 0.5);  // 0.5
    ```
    */
	FUNCTION MIN: ANY_ELEMENTARY
		VAR_INPUT
			args: INTO(MIN)...
		END_VAR
	END_FUNCTION

	/*
    ## LIMIT - Bounding

    Clamps `IN` to the range [`MN`, `MX`].

    - Returns `MN` if `IN < MN`
    - Returns `MX` if `IN > MX`
    - Returns `IN` otherwise

    ```iecst
    result := LIMIT(MN := 0, 15, MX := 10);  // 10
    result := LIMIT(MN := 0, -5, MX := 10);   // 0
    result := LIMIT(MN := 0, 5, MX := 10);    // 5
    ```
    */
	FUNCTION LIMIT: ANY_ELEMENTARY
		VAR_INPUT
			MN: INTO(LIMIT);
			IN: INTO(LIMIT);
			MX: INTO(LIMIT);
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
