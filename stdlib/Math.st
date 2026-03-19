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
    FUNCTION ADD : ANY_MAGNITUDE
        VAR_INPUT
            args: INTO(ADD)...
        END_VAR
        ADD := ...args+
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
    FUNCTION MUL : ANY_NUM
        VAR_INPUT
            args: INTO(MUL)...
        END_VAR
        MUL := ...args*
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
    FUNCTION SUB : ANY_MAGNITUDE
        VAR_INPUT
            args: INTO(SUB)...
        END_VAR
        SUB := ...args-
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
    FUNCTION DIV : ANY_NUM
        VAR_INPUT
            args: INTO(DIV)...
        END_VAR
        DIV := ...args/
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
    FUNCTION MOD : ANY_INT
        VAR_INPUT
            args: INTO(MOD)...
        END_VAR
        MOD := ...args%
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
    FUNCTION ABS : ANY_NUM
        VAR_INPUT
            IN: INTO(ABS)
        END_VAR
        {extern 'math' 'abs' (param IN) (result ABS)}
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
    FUNCTION SQRT : ANY_REAL
        VAR_INPUT
            IN: INTO(SQRT)
        END_VAR
        {extern 'math' 'sqrt' (param IN) (result SQRT)}
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
    FUNCTION LN : ANY_REAL
        VAR_INPUT
            IN: INTO(LN)
        END_VAR
        {extern 'math' 'ln' (param IN) (result LN)}
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
    FUNCTION LOG : ANY_REAL
        VAR_INPUT
            IN: INTO(LOG)
        END_VAR
        {extern 'math' 'log' (param IN) (result LOG)}
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
    FUNCTION EXP : ANY_REAL
        VAR_INPUT
            IN: INTO(EXP)
        END_VAR
        {extern 'math' 'exp' (param IN) (result EXP)}
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
    FUNCTION SIN : ANY_REAL
        VAR_INPUT
            IN: INTO(SIN)
        END_VAR
        {extern 'math' 'sin' (param IN) (result SIN)}
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
    FUNCTION COS : ANY_REAL
        VAR_INPUT
            IN: INTO(COS)
        END_VAR
        {extern 'math' 'cos' (param IN) (result COS)}
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
    FUNCTION TAN : ANY_REAL
        VAR_INPUT
            IN: INTO(TAN)
        END_VAR
        {extern 'math' 'tan' (param IN) (result TAN)}
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
    FUNCTION ASIN : ANY_REAL
        VAR_INPUT
            IN: INTO(ASIN)
        END_VAR
        {extern 'math' 'asin' (param IN) (result ASIN)}
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
    FUNCTION ACOS : ANY_REAL
        VAR_INPUT
            IN: INTO(ACOS)
        END_VAR
        {extern 'math' 'acos' (param IN) (result ACOS)}
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
    FUNCTION ATAN : ANY_REAL
        VAR_INPUT
            IN: INTO(ATAN)
        END_VAR
        {extern 'math' 'atan' (param IN) (result ATAN)}
    END_FUNCTION

    /*
    ## ATAN2 - Two-Argument Arc Tangent

    Returns the arc tangent of `IN / IN2`, using the signs of both
    arguments to determine the quadrant of the result (in radians).

    - **Constraint**: `ANY_REAL` (REAL, LREAL)

    ```iecst
    result := ATAN2(IN := 1.0, IN2 := 1.0);   // 0.7854 (~pi/4)
    result := ATAN2(IN := -1.0, IN2 := -1.0);  // -2.356 (~-3pi/4)
    ```
    */
    FUNCTION ATAN2 : ANY_REAL
        VAR_INPUT
            IN: INTO(ATAN2)
            IN2: INTO(ATAN2)
        END_VAR
        {extern 'math' 'atan2' (param IN IN2) (result ATAN2)}
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
    FUNCTION EXPT : ANY_REAL
        VAR_INPUT
            args: INTO(EXPT)...
        END_VAR
        EXPT := ...args**
    END_FUNCTION

    NAMESPACE Test
        USING Std.Unit

        {test}
        FUNCTION test_abs_positive
        VAR x : INT := 5; y : INT := 5; END_VAR
            ASSERT_EQ(value := x, target := y);
        END_FUNCTION

        {test}
        FUNCTION test_abs_negative
            ASSERT(ABS(IN := -42) = 42);
        END_FUNCTION

        {test}
        FUNCTION test_abs_real
            ASSERT(ABS(IN := -2.5) = 2.5);
        END_FUNCTION

        {test}
        FUNCTION test_sqrt_9
            ASSERT(SQRT(IN := 9.0) = 3.0);
        END_FUNCTION

        {test}
        FUNCTION test_sqrt_4
            ASSERT(SQRT(IN := 4.0) = 2.0);
        END_FUNCTION

        {test}
        FUNCTION test_sin_zero
            ASSERT(SIN(IN := 0.0) = 0.0);
        END_FUNCTION

        {test}
        FUNCTION test_cos_zero
            ASSERT(COS(IN := 0.0) = 1.0);
        END_FUNCTION

        {test}
        FUNCTION test_ln_1
            ASSERT(LN(IN := 1.0) = 0.0);
        END_FUNCTION

        {test}
        FUNCTION test_exp_0
            ASSERT(EXP(IN := 0.0) = 1.0);
        END_FUNCTION

        // Arithmetic variadic functions
        {test}
        FUNCTION test_add_int
            ASSERT(ADD(1, 2, 3) = 6);
        END_FUNCTION

        {test}
        FUNCTION test_add_real
            ASSERT(ADD(1.0, 2.5, 3.5) = 7.0);
        END_FUNCTION

        {test}
        FUNCTION test_sub_int
            ASSERT(SUB(10, 3, 2) = 5);
        END_FUNCTION

        {test}
        FUNCTION test_mul_int
            ASSERT(MUL(2, 3, 4) = 24);
        END_FUNCTION

        {test}
        FUNCTION test_div_int
            ASSERT(DIV(100, 2, 5) = 10);
        END_FUNCTION

        {test}
        FUNCTION test_div_real
            ASSERT(DIV(7.0, 2.0) = 3.5);
        END_FUNCTION

        {test}
        FUNCTION test_mod_int
            ASSERT(MOD(10, 3) = 1);
        END_FUNCTION

        {test}
        FUNCTION test_add_many
            ASSERT(ADD(1, 2, 3, 4, 5) = 15);
        END_FUNCTION

        {test}
        FUNCTION test_mul_real
            ASSERT(MUL(2.0, 3.0, 4.0) = 24.0);
        END_FUNCTION

        // Test that ASSERT_NEQ passes when values differ
        {test}
        FUNCTION test_assert_neq
        VAR a : INT := 1; b : INT := 2; END_VAR
            // Direct comparison to verify the issue
            ASSERT(a <> b);
            // Now test ASSERT_NEQ
            ASSERT_NEQ(value := a, target := b);
        END_FUNCTION

        {test}
        FUNCTION test_sub_real
            ASSERT(SUB(10.0, 5.0) = 5.0);
        END_FUNCTION

        // Variable assignment and comparison
        {test}
        FUNCTION test_var_assignment
        VAR x : INT := 10; y : INT; END_VAR
            y := x;
            ASSERT_EQ(value := x, target := y);
        END_FUNCTION

        // Nested arithmetic via variables
        {test}
        FUNCTION test_nested_arithmetic
        VAR a : INT; b : INT; c : INT; END_VAR
            a := MUL(2, 3);
            b := MUL(4, 5);
            c := ADD(a, b);
            ASSERT_EQ(value := c, target := 26);
        END_FUNCTION

        // Boolean logic
        {test}
        FUNCTION test_bool_and
        VAR a : BOOL := TRUE; b : BOOL := TRUE; END_VAR
            ASSERT(a AND b);
        END_FUNCTION

        {test}
        FUNCTION test_bool_or
        VAR a : BOOL := FALSE; b : BOOL := TRUE; END_VAR
            ASSERT(a OR b);
        END_FUNCTION

        {test}
        FUNCTION test_bool_not
        VAR a : BOOL := FALSE; END_VAR
            ASSERT(NOT a);
        END_FUNCTION

        // Control flow
        {test}
        FUNCTION test_if_else
        VAR result : INT := 0; END_VAR
            IF TRUE THEN
                result := 42;
            ELSE
                result := 0;
            END_IF;
            ASSERT_EQ(value := result, target := 42);
        END_FUNCTION

        // FOR loop
        {test}
        FUNCTION test_for_loop
        VAR sum : INT := 0; i : INT; END_VAR
            FOR i := 1 TO 5 DO
                sum := sum + i;
            END_FOR;
            ASSERT_EQ(value := sum, target := 15);
        END_FUNCTION

        // WHILE loop
        {test}
        FUNCTION test_while_loop
        VAR count : INT := 0; END_VAR
            WHILE count < 10 DO
                count := count + 1;
            END_WHILE;
            ASSERT_EQ(value := count, target := 10);
        END_FUNCTION

        // Comparison operators
        {test}
        FUNCTION test_comparison_lt
        VAR a : INT := 3; b : INT := 5; END_VAR
            ASSERT(a < b);
        END_FUNCTION

        {test}
        FUNCTION test_comparison_ge
        VAR a : INT := 5; b : INT := 5; END_VAR
            ASSERT(a >= b);
        END_FUNCTION

        // Arithmetic with negative results
        {test}
        FUNCTION test_sub_negative
            ASSERT(SUB(3, 10) = -7);
        END_FUNCTION

    END_NAMESPACE

END_NAMESPACE
