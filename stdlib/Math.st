/*
# Standard Arithmetic Functions

IEC 61131-3 **Table 29** - Arithmetic operations on numeric and magnitude types.

All functions are variadic and accept a type specification
constrained to the appropriate `ANY_*` group.
*/
NAMESPACE Std.Math

	/*
    ## ADD - Addition

    Computes the sum of all input arguments.

    - **Constraint**: `ANY_MAGNITUDE` (integers, reals, and durations)

    ```iecst
    result := ADD(1, 2, 3);       // 6
    result := ADD(1.0, 2.5, 3.5); // 7.0
    elapsed := ADD(T#1s, T#2s);   // T#3s
    ```
    */
	FUNCTION ADD: ANY_MAGNITUDE
		VAR_INPUT
			args: INTO(ADD)...
		END_VAR
		ADD := ... args+
	END_FUNCTION

	/*
    ## MUL - Multiplication

    Computes the product of all input arguments.

    - **Constraint**: `ANY_NUM` (integers and reals)

    ```iecst
    result := MUL(2, 3, 4);       // 24
    result := MUL(1.5, 2.0, 3.0); // 9.0
    ```
    */
	FUNCTION MUL: ANY_NUM
		VAR_INPUT
			args: INTO(MUL)...
		END_VAR
		MUL := ... args*
	END_FUNCTION

	/*
    ## SUB - Subtraction

    Computes the successive difference of all input arguments.

    - **Constraint**: `ANY_MAGNITUDE` (integers, reals, and durations)

    ```iecst
    result := SUB(10, 3, 2);     // 5
    elapsed := SUB(T#10s, T#3s); // T#7s
    ```
    */
	FUNCTION SUB: ANY_MAGNITUDE
		VAR_INPUT
			args: INTO(SUB)...
		END_VAR
		SUB := ... args-
	END_FUNCTION

	/*
    ## DIV - Division

    Computes the successive quotient of all input arguments.
    Integer division truncates toward zero.

    - **Constraint**: `ANY_NUM` (integers and reals)

    ```iecst
    result := DIV(100, 2, 5); // 10
    result := DIV(7, 2);      // 3 (integer truncation)
    result := DIV(7.0, 2.0);  // 3.5
    ```
    */
	FUNCTION DIV: ANY_NUM
		VAR_INPUT
			args: INTO(DIV)...
		END_VAR
		DIV := ... args/
	END_FUNCTION

	/*
    ## MOD - Modulo

    Computes the successive remainder of all input arguments.

    - **Constraint**: `ANY_INT` (signed and unsigned integers)

    ```iecst
    result := MOD(10, 3);  // 1
    result := MOD(100, 7); // 2
    ```
    */
	FUNCTION MOD: ANY_INT
		VAR_INPUT
			args: INTO(MOD)...
		END_VAR
		MOD := ... args%
	END_FUNCTION

	/*
    ## ABS - Absolute Value

    Returns the absolute value of the input.

    - **Constraint**: `ANY_NUM` (integers and reals)

    ```iecst
    result := ABS(-5);    // 5
    result := ABS(3.14);  // 3.14
    ```
    */
	FUNCTION ABS: ANY_NUM
		VAR_INPUT
			IN: INTO(ABS)
		END_VAR
		{wasm IN 'abs' (params IN) (result ABS)}
	END_FUNCTION

	/*
    ## SQRT - Square Root

    Returns the square root of the input.

    - **Constraint**: `ANY_REAL` (REAL, LREAL)

    ```iecst
    result := SQRT(9.0);   // 3.0
    result := SQRT(2.0);   // 1.414...
    ```
    */
	FUNCTION SQRT: ANY_REAL
		VAR_INPUT
			IN: INTO(SQRT)
		END_VAR
		{wasm IN 'sqrt' (params IN) (result SQRT)}
	END_FUNCTION

	/*
    ## LN - Natural Logarithm

    Returns the natural logarithm (base e) of the input.

    - **Constraint**: `ANY_REAL` (REAL, LREAL)

    ```iecst
    result := LN(2.718);  // ~1.0
    result := LN(1.0);    // 0.0
    ```
    */
	FUNCTION LN: ANY_REAL
		VAR_INPUT
			IN: INTO(LN)
		END_VAR
		{wasm IN 'ln' (params IN) (result LN)}
	END_FUNCTION

	/*
    ## LOG - Common Logarithm

    Returns the common logarithm (base 10) of the input.

    - **Constraint**: `ANY_REAL` (REAL, LREAL)

    ```iecst
    result := LOG(100.0);  // 2.0
    result := LOG(1000.0); // 3.0
    ```
    */
	FUNCTION LOG: ANY_REAL
		VAR_INPUT
			IN: INTO(LOG)
		END_VAR
		{wasm IN 'log' (params IN) (result LOG)}
	END_FUNCTION

	/*
    ## EXP - Natural Exponential

    Returns e raised to the power of the input.

    - **Constraint**: `ANY_REAL` (REAL, LREAL)

    ```iecst
    result := EXP(1.0);  // 2.718...
    result := EXP(0.0);  // 1.0
    ```
    */
	FUNCTION EXP: ANY_REAL
		VAR_INPUT
			IN: INTO(EXP)
		END_VAR
		{wasm IN 'exp' (params IN) (result EXP)}
	END_FUNCTION

	/*
    ## SIN - Sine

    Returns the sine of the input (in radians).

    - **Constraint**: `ANY_REAL` (REAL, LREAL)

    ```iecst
    result := SIN(0.0);       // 0.0
    result := SIN(1.5708);    // ~1.0
    ```
    */
	FUNCTION SIN: ANY_REAL
		VAR_INPUT
			IN: INTO(SIN)
		END_VAR
		{wasm IN 'sin' (params IN) (result SIN)}
	END_FUNCTION

	/*
    ## COS - Cosine

    Returns the cosine of the input (in radians).

    - **Constraint**: `ANY_REAL` (REAL, LREAL)

    ```iecst
    result := COS(0.0);       // 1.0
    result := COS(3.1416);    // ~-1.0
    ```
    */
	FUNCTION COS: ANY_REAL
		VAR_INPUT
			IN: INTO(COS)
		END_VAR
		{wasm IN 'cos' (params IN) (result COS)}
	END_FUNCTION

	/*
    ## TAN - Tangent

    Returns the tangent of the input (in radians).

    - **Constraint**: `ANY_REAL` (REAL, LREAL)

    ```iecst
    result := TAN(0.0);       // 0.0
    result := TAN(0.7854);    // ~1.0
    ```
    */
	FUNCTION TAN: ANY_REAL
		VAR_INPUT
			IN: INTO(TAN)
		END_VAR
		{wasm IN 'tan' (params IN) (result TAN)}
	END_FUNCTION

	/*
    ## ASIN - Arc Sine

    Returns the arc sine of the input (result in radians).

    - **Constraint**: `ANY_REAL` (REAL, LREAL)

    ```iecst
    result := ASIN(0.0);   // 0.0
    result := ASIN(1.0);   // 1.5708 (~pi/2)
    ```
    */
	FUNCTION ASIN: ANY_REAL
		VAR_INPUT
			IN: INTO(ASIN)
		END_VAR
		{wasm IN 'asin' (params IN) (result ASIN)}
	END_FUNCTION

	/*
    ## ACOS - Arc Cosine

    Returns the arc cosine of the input (result in radians).

    - **Constraint**: `ANY_REAL` (REAL, LREAL)

    ```iecst
    result := ACOS(1.0);   // 0.0
    result := ACOS(0.0);   // 1.5708 (~pi/2)
    ```
    */
	FUNCTION ACOS: ANY_REAL
		VAR_INPUT
			IN: INTO(ACOS)
		END_VAR
		{wasm IN 'acos' (params IN) (result ACOS)}
	END_FUNCTION

	/*
    ## ATAN - Arc Tangent

    Returns the arc tangent of the input (result in radians).

    - **Constraint**: `ANY_REAL` (REAL, LREAL)

    ```iecst
    result := ATAN(0.0);   // 0.0
    result := ATAN(1.0);   // 0.7854 (~pi/4)
    ```
    */
	FUNCTION ATAN: ANY_REAL
		VAR_INPUT
			IN: INTO(ATAN)
		END_VAR
		{wasm IN 'atan' (params IN) (result ATAN)}
	END_FUNCTION

	/*
    ## ATAN2 - Two-Argument Arc Tangent

    Returns the arc tangent of `IN / IN2`, using the signs of both
    arguments to determine the quadrant of the result (in radians).

    - **Constraint**: `ANY_REAL` (REAL, LREAL)

    ```iecst
    result := ATAN2(1.0, IN2 := 1.0);   // 0.7854 (~pi/4)
    result := ATAN2(-1.0, IN2 := -1.0);  // -2.356 (~-3pi/4)
    ```
    */
	FUNCTION ATAN2: ANY_REAL
		VAR_INPUT
			IN: INTO(ATAN2)
			IN2: INTO(ATAN2)
		END_VAR
		{wasm IN 'atan2' (params IN IN2) (result ATAN2)}
	END_FUNCTION

	/*
    ## EXPT - Exponentiation

    Raises the base to successive powers.

    - **Constraint**: `ANY_REAL` (REAL, LREAL)

    ```iecst
    result := EXPT(2.0, 3.0);  // 8.0
    result := EXPT(9.0, 0.5);  // 3.0 (square root)
    ```
    */
	FUNCTION EXPT: ANY_REAL
		VAR_INPUT
			args: INTO(EXPT)...
		END_VAR
		EXPT := ... args**
	END_FUNCTION

	NAMESPACE Test
		USING Std.Unit

		// ----- ABS (extern, ANY_NUM) ----------------------------------
		{test}
		FUNCTION test_abs_positive ASSERT(ABS(5) = 5);
		END_FUNCTION

		{test}
		FUNCTION test_abs_negative ASSERT(ABS(-42) = 42);
		END_FUNCTION

		{test}
		FUNCTION test_abs_zero ASSERT(ABS(0) = 0);
		END_FUNCTION

		{test}
		FUNCTION test_abs_real ASSERT(ABS(-2.5) = 2.5);
		END_FUNCTION

		{test}
		FUNCTION test_abs_real_positive ASSERT(ABS(3.14) = 3.14);
		END_FUNCTION

		{test}
		FUNCTION test_abs_with_message ASSERT(ABS(-7) = 7, 'ABS should return the positive value');
		END_FUNCTION

		// ----- SQRT (extern, ANY_REAL) --------------------------------
		{test}
		FUNCTION test_sqrt_9 ASSERT(SQRT(9.0) = 3.0);
		END_FUNCTION

		{test}
		FUNCTION test_sqrt_4 ASSERT(SQRT(4.0) = 2.0);
		END_FUNCTION

		{test}
		FUNCTION test_sqrt_1 ASSERT(SQRT(1.0) = 1.0);
		END_FUNCTION

		// ----- Trigonometric (extern, ANY_REAL) -----------------------
		{test}
		FUNCTION test_sin_zero ASSERT(SIN(0.0) = 0.0);
		END_FUNCTION

		{test}
		FUNCTION test_cos_zero ASSERT(COS(0.0) = 1.0);
		END_FUNCTION

		{test}
		FUNCTION test_tan_zero ASSERT(TAN(0.0) = 0.0);
		END_FUNCTION

		{test}
		FUNCTION test_asin_zero ASSERT(ASIN(0.0) = 0.0);
		END_FUNCTION

		{test}
		FUNCTION test_acos_one ASSERT(ACOS(1.0) = 0.0);
		END_FUNCTION

		{test}
		FUNCTION test_atan_zero ASSERT(ATAN(0.0) = 0.0);
		END_FUNCTION

		// ----- Logarithmic / Exponential (extern, ANY_REAL) ----------
		{test}
		FUNCTION test_ln_1 ASSERT(LN(1.0) = 0.0);
		END_FUNCTION

		{test}
		FUNCTION test_log_1 ASSERT(LOG(1.0) = 0.0);
		END_FUNCTION

		{test}
		FUNCTION test_exp_0 ASSERT(EXP(0.0) = 1.0);
		END_FUNCTION

		// ----- Arithmetic operators --------------------------------------------------------------------------------
		{test}
		FUNCTION test_add_int
			VAR
				r: INT;
			END_VAR
			r := 3 + 4;
			ASSERT(r = 7);
		END_FUNCTION

		{test}
		FUNCTION test_sub_int
			VAR
				r: INT;
			END_VAR
			r := 10 - 3;
			ASSERT(r = 7);
		END_FUNCTION

		{test}
		FUNCTION test_mul_int
			VAR
				r: INT;
			END_VAR
			r := 6 * 7;
			ASSERT(r = 42);
		END_FUNCTION

		{test}
		FUNCTION test_div_int
			VAR
				r: INT;
			END_VAR
			r := 100 / 10;
			ASSERT(r = 10);
		END_FUNCTION

		{test}
		FUNCTION test_mod_int
			VAR
				r: INT;
			END_VAR
			r := 10 MOD 3;
			ASSERT(r = 1);
		END_FUNCTION

		{test}
		FUNCTION test_add_real
			VAR
				r: REAL;
			END_VAR
			r := 1.5 + 2.5;
			ASSERT(r = 4.0);
		END_FUNCTION

		{test}
		FUNCTION test_sub_negative
			VAR
				r: INT;
			END_VAR
			r := 3 - 10;
			ASSERT(r = -7);
		END_FUNCTION

		{test}
		FUNCTION test_nested_arithmetic
			VAR
				a: INT;
				b: INT;
				c: INT;
			END_VAR
			a := 2 * 3;
			b := 4 * 5;
			c := a + b;
			ASSERT(c = 26);
		END_FUNCTION

		// ----- Variable assignment ---------------------------------------------------------------------------------
		{test}
		FUNCTION test_var_assignment
			VAR
				x: INT := 10;
				y: INT;
			END_VAR
			y := x;
			ASSERT(x = y);
		END_FUNCTION

		{test}
		FUNCTION test_var_init
			VAR
				x: INT := 42;
			END_VAR
			ASSERT(x = 42);
		END_FUNCTION

		{test}
		FUNCTION test_var_default_zero
			VAR
				x: INT;
			END_VAR
			ASSERT(x = 0);
		END_FUNCTION

		{test}
		FUNCTION test_var_bool_init
			VAR
				b: BOOL := TRUE;
			END_VAR
			ASSERT(b);
		END_FUNCTION

		{test}
		FUNCTION test_var_real_init
			VAR
				r: REAL := 3.14;
			END_VAR
			ASSERT(r = 3.14);
		END_FUNCTION

		// ----- Boolean logic ---------------------------------------------------------------------------------------
		{test}
		FUNCTION test_bool_and_tt
			VAR
				a: BOOL := TRUE;
				b: BOOL := TRUE;
			END_VAR
			ASSERT(a AND b);
		END_FUNCTION

		{test}
		FUNCTION test_bool_and_tf
			VAR
				a: BOOL := TRUE;
				b: BOOL := FALSE;
			END_VAR
			ASSERT(NOT (a AND b));
		END_FUNCTION

		{test}
		FUNCTION test_bool_or_tf
			VAR
				a: BOOL := FALSE;
				b: BOOL := TRUE;
			END_VAR
			ASSERT(a OR b);
		END_FUNCTION

		{test}
		FUNCTION test_bool_or_ff
			VAR
				a: BOOL := FALSE;
				b: BOOL := FALSE;
			END_VAR
			ASSERT(NOT (a OR b));
		END_FUNCTION

		{test}
		FUNCTION test_bool_not
			VAR
				a: BOOL := FALSE;
			END_VAR
			ASSERT(NOT a);
		END_FUNCTION

		{test}
		FUNCTION test_bool_xor
			VAR
				a: BOOL := TRUE;
				b: BOOL := FALSE;
			END_VAR
			ASSERT(a XOR b);
		END_FUNCTION

		{test}
		FUNCTION test_bool_xor_same
			VAR
				a: BOOL := TRUE;
				b: BOOL := TRUE;
			END_VAR
			ASSERT(NOT (a XOR b));
		END_FUNCTION

		// ----- Comparison operators --------------------------------------------------------------------------------
		{test}
		FUNCTION test_cmp_eq
			VAR
				a: INT := 5;
			END_VAR
			ASSERT(a = 5);
		END_FUNCTION

		{test}
		FUNCTION test_cmp_ne
			VAR
				a: INT := 1;
				b: INT := 2;
			END_VAR
			ASSERT(a <> b);
		END_FUNCTION

		{test}
		FUNCTION test_cmp_lt
			VAR
				a: INT := 3;
				b: INT := 5;
			END_VAR
			ASSERT(a < b);
		END_FUNCTION

		{test}
		FUNCTION test_cmp_le
			VAR
				a: INT := 5;
			END_VAR
			ASSERT(a <= 5);
		END_FUNCTION

		{test}
		FUNCTION test_cmp_gt
			VAR
				a: INT := 10;
				b: INT := 5;
			END_VAR
			ASSERT(a > b);
		END_FUNCTION

		{test}
		FUNCTION test_cmp_ge
			VAR
				a: INT := 5;
			END_VAR
			ASSERT(a >= 5);
		END_FUNCTION

		// ----- Control flow: IF ------------------------------------------------------------------------------------
		{test}
		FUNCTION test_if_true
			VAR
				r: INT := 0;
			END_VAR
			IF TRUE THEN
				r := 1;
			END_IF;
			ASSERT(r = 1);
		END_FUNCTION

		{test}
		FUNCTION test_if_false
			VAR
				r: INT := 0;
			END_VAR
			IF FALSE THEN
				r := 1;
			END_IF;
			ASSERT(r = 0);
		END_FUNCTION

		{test}
		FUNCTION test_if_else
			VAR
				r: INT := 0;
			END_VAR
			IF FALSE THEN
				r := 1;
			ELSE
				r := 2;
			END_IF;
			ASSERT(r = 2);
		END_FUNCTION

		{test}
		FUNCTION test_if_elsif
			VAR
				r: INT := 0;
				x: INT := 2;
			END_VAR
			IF x = 1 THEN
				r := 10;
			ELSIF x = 2 THEN
				r := 20;
			ELSE
				r := 30;
			END_IF;
			ASSERT(r = 20);
		END_FUNCTION

		// ----- Control flow: FOR -----------------------------------------------------------------------------------
		{test}
		FUNCTION test_for_sum
			VAR
				sum: INT := 0;
				i: INT;
			END_VAR
			FOR i := 1 TO 5 DO
				sum := sum + i;
			END_FOR;
			ASSERT(sum = 15);
		END_FUNCTION

		{test}
		FUNCTION test_for_single
			VAR
				r: INT := 0;
				i: INT;
			END_VAR
			FOR i := 5 TO 5 DO
				r := r + 1;
			END_FOR;
			ASSERT(r = 1);
		END_FUNCTION

		// ----- Control flow: WHILE ---------------------------------------------------------------------------------
		{test}
		FUNCTION test_while_count
			VAR
				count: INT := 0;
			END_VAR
			WHILE count < 10 DO
				count := count + 1;
			END_WHILE;
			ASSERT(count = 10);
		END_FUNCTION

		{test}
		FUNCTION test_while_false
			VAR
				count: INT := 0;
			END_VAR
			WHILE FALSE DO
				count := count + 1;
			END_WHILE;
			ASSERT(count = 0);
		END_FUNCTION

		// ----- Control flow: REPEAT --------------------------------------------------------------------------------
		{test}
		FUNCTION test_repeat_until
			VAR
				count: INT := 0;
			END_VAR
			REPEAT count := count + 1;
				UNTIL count >= 5
			END_REPEAT;
			ASSERT(count = 5);
		END_FUNCTION

		// ----- Function calls with return values ----------------------
		{test}
		FUNCTION test_func_return
			VAR
				r: INT;
			END_VAR
			r := ABS(-99);
			ASSERT(r = 99);
		END_FUNCTION

		// Chained ANY_* calls via intermediate variable
		{test}
		FUNCTION test_chained_abs
			VAR
				inner: INT;
				r: INT;
			END_VAR
			inner := ABS(-5);
			r := ABS(inner);
			ASSERT(r = 5);
		END_FUNCTION

		// ----- Type widths -----------------------------------------------------------------------------------------
		{test}
		FUNCTION test_lint_arithmetic
			VAR
				a: LINT := 1000000000;
				b: LINT := 2000000000;
				c: LINT;
			END_VAR
			c := a + b;
			ASSERT(c = 3000000000);
		END_FUNCTION

		{test}
		FUNCTION test_lreal_precision
			VAR
				a: LREAL := 1.0;
				b: LREAL := 3.0;
				c: LREAL;
			END_VAR
			c := a / b;
			// Can't compare exactly, but c * 3.0 should be 1.0
			ASSERT(c * b = a);
		END_FUNCTION
	END_NAMESPACE
END_NAMESPACE
