/*
# Standard Arithmetic Functions

IEC 61131-3 **Table 29** - Arithmetic operations on numeric and magnitude types.

Each function is provided as a set of overloads, one per concrete type
in the applicable numeric family.
*/
NAMESPACE Std.Math

	/*
    ## ABS - Absolute Value

    Returns the absolute value of the input.

    - **Overloads**: `SINT`, `INT`, `DINT`, `LINT`, `REAL`, `LREAL`

    ```iecst
    result := ABS(-5);    // 5
    result := ABS(3.14);  // 3.14
    ```
    */
	FUNCTION ABS: SINT
		VAR_INPUT
			IN: SINT
		END_VAR
		{wasm IN 'abs' (params IN) (result ABS)}
	END_FUNCTION

	FUNCTION ABS: INT
		VAR_INPUT
			IN: INT
		END_VAR
		{wasm IN 'abs' (params IN) (result ABS)}
	END_FUNCTION

	FUNCTION ABS: DINT
		VAR_INPUT
			IN: DINT
		END_VAR
		{wasm IN 'abs' (params IN) (result ABS)}
	END_FUNCTION

	FUNCTION ABS: LINT
		VAR_INPUT
			IN: LINT
		END_VAR
		{wasm IN 'abs' (params IN) (result ABS)}
	END_FUNCTION

	FUNCTION ABS: REAL
		VAR_INPUT
			IN: REAL
		END_VAR
		{wasm IN 'abs' (params IN) (result ABS)}
	END_FUNCTION

	FUNCTION ABS: LREAL
		VAR_INPUT
			IN: LREAL
		END_VAR
		{wasm IN 'abs' (params IN) (result ABS)}
	END_FUNCTION

	/*
    ## SQRT - Square Root

    Returns the square root of the input.

    - **Overloads**: `REAL`, `LREAL`

    ```iecst
    result := SQRT(9.0);   // 3.0
    result := SQRT(2.0);   // 1.414...
    ```
    */
	FUNCTION SQRT: REAL
		VAR_INPUT
			IN: REAL
		END_VAR
		{wasm IN 'sqrt' (params IN) (result SQRT)}
	END_FUNCTION

	FUNCTION SQRT: LREAL
		VAR_INPUT
			IN: LREAL
		END_VAR
		{wasm IN 'sqrt' (params IN) (result SQRT)}
	END_FUNCTION

	/*
    ## LN - Natural Logarithm

    Returns the natural logarithm (base e) of the input.

    - **Overloads**: `REAL`, `LREAL`

    ```iecst
    result := LN(2.718);  // ~1.0
    result := LN(1.0);    // 0.0
    ```
    */
	FUNCTION LN: REAL
		VAR_INPUT
			IN: REAL
		END_VAR
		{wasm IN 'ln' (params IN) (result LN)}
	END_FUNCTION

	FUNCTION LN: LREAL
		VAR_INPUT
			IN: LREAL
		END_VAR
		{wasm IN 'ln' (params IN) (result LN)}
	END_FUNCTION

	/*
    ## LOG - Common Logarithm

    Returns the common logarithm (base 10) of the input.

    - **Overloads**: `REAL`, `LREAL`

    ```iecst
    result := LOG(100.0);  // 2.0
    result := LOG(1000.0); // 3.0
    ```
    */
	FUNCTION LOG: REAL
		VAR_INPUT
			IN: REAL
		END_VAR
		{wasm IN 'log' (params IN) (result LOG)}
	END_FUNCTION

	FUNCTION LOG: LREAL
		VAR_INPUT
			IN: LREAL
		END_VAR
		{wasm IN 'log' (params IN) (result LOG)}
	END_FUNCTION

	/*
    ## EXP - Natural Exponential

    Returns e raised to the power of the input.

    - **Overloads**: `REAL`, `LREAL`

    ```iecst
    result := EXP(1.0);  // 2.718...
    result := EXP(0.0);  // 1.0
    ```
    */
	FUNCTION EXP: REAL
		VAR_INPUT
			IN: REAL
		END_VAR
		{wasm IN 'exp' (params IN) (result EXP)}
	END_FUNCTION

	FUNCTION EXP: LREAL
		VAR_INPUT
			IN: LREAL
		END_VAR
		{wasm IN 'exp' (params IN) (result EXP)}
	END_FUNCTION

	/*
    ## SIN - Sine

    Returns the sine of the input (in radians).

    - **Overloads**: `REAL`, `LREAL`

    ```iecst
    result := SIN(0.0);       // 0.0
    result := SIN(1.5708);    // ~1.0
    ```
    */
	FUNCTION SIN: REAL
		VAR_INPUT
			IN: REAL
		END_VAR
		{wasm IN 'sin' (params IN) (result SIN)}
	END_FUNCTION

	FUNCTION SIN: LREAL
		VAR_INPUT
			IN: LREAL
		END_VAR
		{wasm IN 'sin' (params IN) (result SIN)}
	END_FUNCTION

	/*
    ## COS - Cosine

    Returns the cosine of the input (in radians).

    - **Overloads**: `REAL`, `LREAL`

    ```iecst
    result := COS(0.0);       // 1.0
    result := COS(3.1416);    // ~-1.0
    ```
    */
	FUNCTION COS: REAL
		VAR_INPUT
			IN: REAL
		END_VAR
		{wasm IN 'cos' (params IN) (result COS)}
	END_FUNCTION

	FUNCTION COS: LREAL
		VAR_INPUT
			IN: LREAL
		END_VAR
		{wasm IN 'cos' (params IN) (result COS)}
	END_FUNCTION

	/*
    ## TAN - Tangent

    Returns the tangent of the input (in radians).

    - **Overloads**: `REAL`, `LREAL`

    ```iecst
    result := TAN(0.0);       // 0.0
    result := TAN(0.7854);    // ~1.0
    ```
    */
	FUNCTION TAN: REAL
		VAR_INPUT
			IN: REAL
		END_VAR
		{wasm IN 'tan' (params IN) (result TAN)}
	END_FUNCTION

	FUNCTION TAN: LREAL
		VAR_INPUT
			IN: LREAL
		END_VAR
		{wasm IN 'tan' (params IN) (result TAN)}
	END_FUNCTION

	/*
    ## ASIN - Arc Sine

    Returns the arc sine of the input (result in radians).

    - **Overloads**: `REAL`, `LREAL`

    ```iecst
    result := ASIN(0.0);   // 0.0
    result := ASIN(1.0);   // 1.5708 (~pi/2)
    ```
    */
	FUNCTION ASIN: REAL
		VAR_INPUT
			IN: REAL
		END_VAR
		{wasm IN 'asin' (params IN) (result ASIN)}
	END_FUNCTION

	FUNCTION ASIN: LREAL
		VAR_INPUT
			IN: LREAL
		END_VAR
		{wasm IN 'asin' (params IN) (result ASIN)}
	END_FUNCTION

	/*
    ## ACOS - Arc Cosine

    Returns the arc cosine of the input (result in radians).

    - **Overloads**: `REAL`, `LREAL`

    ```iecst
    result := ACOS(1.0);   // 0.0
    result := ACOS(0.0);   // 1.5708 (~pi/2)
    ```
    */
	FUNCTION ACOS: REAL
		VAR_INPUT
			IN: REAL
		END_VAR
		{wasm IN 'acos' (params IN) (result ACOS)}
	END_FUNCTION

	FUNCTION ACOS: LREAL
		VAR_INPUT
			IN: LREAL
		END_VAR
		{wasm IN 'acos' (params IN) (result ACOS)}
	END_FUNCTION

	/*
    ## ATAN - Arc Tangent

    Returns the arc tangent of the input (result in radians).

    - **Overloads**: `REAL`, `LREAL`

    ```iecst
    result := ATAN(0.0);   // 0.0
    result := ATAN(1.0);   // 0.7854 (~pi/4)
    ```
    */
	FUNCTION ATAN: REAL
		VAR_INPUT
			IN: REAL
		END_VAR
		{wasm IN 'atan' (params IN) (result ATAN)}
	END_FUNCTION

	FUNCTION ATAN: LREAL
		VAR_INPUT
			IN: LREAL
		END_VAR
		{wasm IN 'atan' (params IN) (result ATAN)}
	END_FUNCTION

	/*
    ## ATAN2 - Two-Argument Arc Tangent

    Returns the arc tangent of `IN / IN2`, using the signs of both
    arguments to determine the quadrant of the result (in radians).

    - **Overloads**: `REAL`, `LREAL`

    ```iecst
    result := ATAN2(1.0, IN2 := 1.0);   // 0.7854 (~pi/4)
    result := ATAN2(-1.0, IN2 := -1.0);  // -2.356 (~-3pi/4)
    ```
    */
	FUNCTION ATAN2: REAL
		VAR_INPUT
			IN: REAL
			IN2: REAL
		END_VAR
		{wasm IN 'atan2' (params IN IN2) (result ATAN2)}
	END_FUNCTION

	FUNCTION ATAN2: LREAL
		VAR_INPUT
			IN: LREAL
			IN2: LREAL
		END_VAR
		{wasm IN 'atan2' (params IN IN2) (result ATAN2)}
	END_FUNCTION

	/*
    ## EXPT - Exponentiation

    Raises `IN` to the power of `IN2` — the function form of the `**`
    operator; both lower to the same implementation. Per IEC 61131-3,
    `IN` is ANY_REAL, `IN2` is ANY_NUM (an integer exponent widens
    implicitly), and the result has `IN`'s type.

    - **Overloads**: `REAL`, `LREAL`

    ```iecst
    result := EXPT(2.0, 3.0);   // 8.0
    result := EXPT(9.0, 0.5);   // 3.0 (a fractional exponent is a root)
    ```
    */
	FUNCTION EXPT: REAL
		VAR_INPUT
			IN: REAL
			IN2: REAL
		END_VAR
		{wasm IN 'pow' (params IN IN2) (result EXPT)}
	END_FUNCTION

	FUNCTION EXPT: LREAL
		VAR_INPUT
			IN: LREAL
			IN2: LREAL
		END_VAR
		{wasm IN 'pow' (params IN IN2) (result EXPT)}
	END_FUNCTION

	NAMESPACE Test
		USING Std.Unit

		// ----- EXPT ---------------------------------------------------
		{test}
		FUNCTION test_expt_real ASSERT(EXPT(2.0, 3.0) = 8.0);
		END_FUNCTION

		{test}
		FUNCTION test_expt_fractional ASSERT(EXPT(9.0, 0.5) = 3.0);
		END_FUNCTION

		// The INT exponent goes through the LREAL overload: an LREAL IN1
		// fits only one candidate, so the call is unambiguous. (With a REAL
		// IN1 both overloads accept the widened INT and E0237 asks for a
		// cast — overload scoring has no exact-match preference yet.)
		{test}
		FUNCTION test_expt_integer_exponent
		VAR x: LREAL; n: INT; END_VAR
			x := 2.0;
			n := 10;
			ASSERT(EXPT(x, n) = LREAL#1024.0);
		END_FUNCTION

		{test}
		FUNCTION test_expt_lreal ASSERT(EXPT(LREAL#2.0, LREAL#10.0) = LREAL#1024.0);
		END_FUNCTION

		{test}
		FUNCTION test_expt_matches_operator
		VAR x: REAL; END_VAR
			x := 3.0;
			ASSERT(EXPT(x, 2.0) = x ** 2.0);
		END_FUNCTION

		// ----- ABS (per signed-numeric overloads) ---------------------
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

		// ----- SQRT (REAL/LREAL overloads) ----------------------------
		{test}
		FUNCTION test_sqrt_9 ASSERT(SQRT(9.0) = 3.0);
		END_FUNCTION

		{test}
		FUNCTION test_sqrt_4 ASSERT(SQRT(4.0) = 2.0);
		END_FUNCTION

		{test}
		FUNCTION test_sqrt_1 ASSERT(SQRT(1.0) = 1.0);
		END_FUNCTION

		// ----- Trigonometric (REAL/LREAL overloads) -------------------
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

		// ----- Logarithmic / Exponential (REAL/LREAL overloads) ------
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

		{test}
		FUNCTION test_for_descending
			VAR
				n: INT := 0;
				i: INT;
			END_VAR
			// A constant negative BY flips the exit comparison — this loop
			// used to exit before its first iteration.
			FOR i := 10 TO 1 BY -1 DO
				n := n + 1;
			END_FOR;
			ASSERT(n = 10, 'descending FOR runs all 10 iterations');
		END_FUNCTION

		{test}
		FUNCTION test_for_lint_counter
			VAR
				n: LINT := 0;
				i: LINT;
			END_VAR
			// i64-lane control variable (used to emit invalid wasm).
			FOR i := LINT#1 TO LINT#3 DO
				n := n + i;
			END_FOR;
			ASSERT(n = LINT#6, 'LINT FOR loop sums 1+2+3');
		END_FUNCTION

		{test}
		FUNCTION test_for_lint_descending_beyond_i32
			VAR
				n: LINT := 0;
				i: LINT;
			END_VAR
			FOR i := LINT#5000000002 TO LINT#5000000000 BY LINT#-1 DO
				n := n + 1;
			END_FOR;
			ASSERT(n = LINT#3, 'descending i64 FOR with bounds beyond i32 range');
		END_FUNCTION

		{test}
		FUNCTION test_for_subwidth_counter_stays_in_domain
			VAR
				n: USINT := 0;
				i: USINT;
			END_VAR
			// Counter increments stay in the type's domain. NOTE: an upper
			// bound at the type MAX (e.g. TO 255) wraps before the exit
			// check and never terminates — same documented footgun as
			// other toolchains; keep bounds below the type maximum.
			FOR i := 250 TO 254 DO
				n := n + 1;
			END_FOR;
			ASSERT(n = USINT#5, 'body ran 5 times');
			ASSERT(i = USINT#255, 'counter exits in-domain at 255');
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

		// Chained calls via intermediate variable
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

		// --- transcendentals at nonzero, non-colliding points: the fixed
		// points used above (0->0, 1->0, 0->1) are shared across the whole
		// family, so swapped imports (ln vs log, exp vs cos) pass unnoticed ---

		{test}
		FUNCTION test_trig_nonzero_points_real
			ASSERT(ABS(SIN(0.5) - 0.4794255) < 0.0001, 'SIN(0.5)');
			ASSERT(ABS(COS(1.0) - 0.5403023) < 0.0001, 'COS(1.0)');
			ASSERT(ABS(TAN(1.0) - 1.5574077) < 0.0001, 'TAN(1.0)');
			ASSERT(ABS(ASIN(0.5) - 0.5235988) < 0.0001, 'ASIN(0.5) = pi/6');
			ASSERT(ABS(ACOS(0.5) - 1.0471976) < 0.0001, 'ACOS(0.5) = pi/3');
			ASSERT(ABS(ATAN(1.0) - 0.7853982) < 0.0001, 'ATAN(1.0) = pi/4');
		END_FUNCTION

		{test}
		FUNCTION test_trig_nonzero_points_lreal
			ASSERT(ABS(SIN(LREAL#0.5) - LREAL#0.479425538604203) < LREAL#0.000000001, 'SIN LREAL');
			ASSERT(ABS(COS(LREAL#1.0) - LREAL#0.540302305868140) < LREAL#0.000000001, 'COS LREAL');
			ASSERT(ABS(ATAN(LREAL#1.0) - LREAL#0.785398163397448) < LREAL#0.000000001, 'ATAN LREAL = pi/4');
		END_FUNCTION

		{test}
		FUNCTION test_log_exp_nonzero_points
			ASSERT(ABS(LN(2.0) - 0.6931472) < 0.0001, 'LN(2)');
			ASSERT(ABS(LOG(100.0) - 2.0) < 0.0001, 'LOG(100) = 2, distinguishes LOG from LN');
			ASSERT(ABS(EXP(1.0) - 2.7182818) < 0.0001, 'EXP(1) = e');
			ASSERT(ABS(SQRT(2.0) - 1.4142136) < 0.0001, 'SQRT(2)');
			ASSERT(ABS(LN(LREAL#2.0) - LREAL#0.693147180559945) < LREAL#0.000000001, 'LN LREAL');
			ASSERT(ABS(EXP(LREAL#1.0) - LREAL#2.718281828459045) < LREAL#0.000000001, 'EXP LREAL');
			ASSERT(ABS(LOG(LREAL#1000.0) - LREAL#3.0) < LREAL#0.000000001, 'LOG LREAL');
			ASSERT(ABS(SQRT(LREAL#2.0) - LREAL#1.414213562373095) < LREAL#0.000000001, 'SQRT LREAL');
		END_FUNCTION

		// --- ATAN2: the one two-argument function — swapped y/x is only
		// detectable with an asymmetric quadrant pair ---

		{test}
		FUNCTION test_atan2_quadrants_real
			ASSERT(ABS(ATAN2(1.0, -1.0) - 2.3561945) < 0.0001, 'ATAN2(+y,-x) = 3pi/4 (2nd quadrant)');
			ASSERT(ABS(ATAN2(-1.0, 1.0) - (-0.7853982)) < 0.0001, 'ATAN2(-y,+x) = -pi/4 (4th quadrant)');
		END_FUNCTION

		{test}
		FUNCTION test_atan2_quadrants_lreal
			ASSERT(ABS(ATAN2(LREAL#1.0, LREAL#-1.0) - LREAL#2.356194490192345) < LREAL#0.000000001, 'ATAN2 LREAL 3pi/4');
			ASSERT(ABS(ATAN2(LREAL#-1.0, LREAL#1.0) - LREAL#-0.785398163397448) < LREAL#0.000000001, 'ATAN2 LREAL -pi/4');
		END_FUNCTION

		// --- ABS boundaries and the dark overloads ---

		{test}
		FUNCTION test_abs_boundaries
			ASSERT_EQ(ABS(SINT#-127), SINT#127, 'ABS SINT near MIN');
			ASSERT_EQ(ABS(INT#-32767), INT#32767, 'ABS INT near MIN');
			ASSERT_EQ(ABS(DINT#-2147483647), DINT#2147483647, 'ABS DINT near MIN');
			ASSERT_EQ(ABS(LINT#-5000000000), LINT#5000000000, 'ABS LINT on the i64 lane');
		END_FUNCTION

		// --- sub-width integer arithmetic wraps at the IEC type width
		// (type-faithful semantics; a C-emitting compiler's C native types
		// behave the same). Values must never escape the type's domain ---

		{test}
		FUNCTION test_subwidth_arithmetic_wraps
			VAR
				a: USINT := 255;
				b: USINT := 1;
				z: USINT := 0;
				m: USINT := 16;
				i: INT := 32767;
				one: INT := 1;
				imin: INT := -32768;
				u16a: UINT := 65535;
			END_VAR
			ASSERT(a + b = USINT#0, 'USINT 255+1 wraps to 0');
			ASSERT(z - b = USINT#255, 'USINT 0-1 wraps to 255');
			ASSERT(m * m = USINT#0, 'USINT 16*16 wraps to 0');
			ASSERT(i + one = INT#-32768, 'INT 32767+1 wraps to -32768');
			ASSERT(imin - one = INT#32767, 'INT -32768-1 wraps to 32767');
			ASSERT(u16a * u16a = UINT#1, 'UINT 65535*65535 wraps to 1');
		END_FUNCTION

		{test}
		FUNCTION test_subwidth_negation_wraps
			VAR
				s: SINT := -128;
				sm1: SINT := -1;
			END_VAR
			ASSERT(-s = SINT#-128, 'NEG(SINT#-128) wraps to -128');
			ASSERT(s / sm1 = SINT#-128, 'SINT -128/-1 wraps to -128');
			ASSERT(ABS(s) = SINT#-128, 'ABS(SINT#-128) wraps to -128');
			ASSERT(ABS(SINT#-127) = SINT#127, 'ABS in-range is unaffected');
		END_FUNCTION

		// --- domain edges (pins current IEEE 754 behavior through libm) ---

		{test}
		FUNCTION test_float_domain_edges_pin_current
			(* pins current behavior: IEEE semantics — NaN for out-of-domain,
			   infinities at the poles. NaN is the only value with x <> x. *)
			ASSERT(SQRT(-1.0) <> SQRT(-1.0), 'SQRT(-1) is NaN');
			ASSERT(LN(-1.0) <> LN(-1.0), 'LN(-1) is NaN');
			ASSERT(LN(0.0) < -1000000.0, 'LN(0) is -inf');
			ASSERT(EXP(1000.0) > 1000000.0, 'EXP(1000) overflows to +inf');
		END_FUNCTION
	END_NAMESPACE
END_NAMESPACE
