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
