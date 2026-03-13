/*
# Standard Arithmetic Functions

IEC 61131-3 **Table 29** - Arithmetic operations on numeric and magnitude types.

All functions are variadic and accept a generic type parameter
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
    FUNCTION ADD<T: ANY_MAGNITUDE> : T
        VAR_INPUT
            args: T...
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
    FUNCTION MUL<T: ANY_NUM> : T
        VAR_INPUT
            args: T...
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
    FUNCTION SUB<T: ANY_MAGNITUDE> : T
        VAR_INPUT
            args: T...
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
    FUNCTION DIV<T: ANY_NUM> : T
        VAR_INPUT
            args: T...
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
    FUNCTION MOD<T: ANY_INT> : T
        VAR_INPUT
            args: T...
        END_VAR
        MOD := ...argsMOD
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
    FUNCTION ABS<T: ANY_NUM> : T
        VAR_INPUT
            IN: T
        END_VAR

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
    FUNCTION SQRT<T: ANY_REAL> : T
        VAR_INPUT
            IN: T
        END_VAR

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
    FUNCTION LN<T: ANY_REAL>: T
        VAR_INPUT
            IN: T
        END_VAR

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
    FUNCTION LOG<T: ANY_REAL>: T
        VAR_INPUT
            IN: T
        END_VAR

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
    FUNCTION EXP<T: ANY_REAL> : T
        VAR_INPUT
            IN: T
        END_VAR

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
    FUNCTION SIN<T: ANY_REAL> : T
        VAR_INPUT
            IN: T
        END_VAR

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
    FUNCTION COS<T: ANY_REAL> : T
        VAR_INPUT
            IN: T
        END_VAR

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
    FUNCTION TAN<T: ANY_REAL> : T
        VAR_INPUT
            IN: T
        END_VAR

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
    FUNCTION ASIN<T: ANY_REAL> : T
        VAR_INPUT
            IN: T
        END_VAR

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
    FUNCTION ACOS<T: ANY_REAL> : T
        VAR_INPUT
            IN: T
        END_VAR

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
    FUNCTION ATAN<T: ANY_REAL> : T
        VAR_INPUT
            IN: T
        END_VAR

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
    FUNCTION ATAN2<T: ANY_REAL> : T
        VAR_INPUT
            IN: T
            IN2: T
        END_VAR

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
    FUNCTION EXPT<T: ANY_REAL> : T
        VAR_INPUT
            args: T...
        END_VAR
        EXPT := ...args**
    END_FUNCTION

END_NAMESPACE
