/*
# Standard Bit Shift Functions

IEC 61131-3 **Table 30** - Bit shift and rotate operations.

Each function takes a bit-type input `IN` and an integer shift count `N`.
The result type matches the input type.
*/
NAMESPACE Std

    /*
    ## SHL - Shift Left

    Shifts `IN` left by `N` bit positions, filling with zeros.

    - **IN**: `ANY_BIT` (BOOL, BYTE, WORD, DWORD, LWORD)
    - **N**: `ANY_INT` (shift count)

    ```iecst
    result := SHL(IN := 16#0F, N := 4); // 16#F0
    result := SHL(IN := 16#01, N := 8); // 16#0100
    ```
    */
    FUNCTION SHL<T: ANY_BIT, Y: ANY_INT> : T
        VAR_INPUT
            IN: T;
            N: Y;
        END_VAR
    END_FUNCTION

    /*
    ## SHR - Shift Right

    Shifts `IN` right by `N` bit positions, filling with zeros.

    - **IN**: `ANY_BIT` (BOOL, BYTE, WORD, DWORD, LWORD)
    - **N**: `ANY_INT` (shift count)

    ```iecst
    result := SHR(IN := 16#F0, N := 4); // 16#0F
    ```
    */
    FUNCTION SHR<T: ANY_BIT, Y: ANY_INT> : T
        VAR_INPUT
            IN: T;
            N: Y;
        END_VAR
    END_FUNCTION

    /*
    ## ROL - Rotate Left

    Rotates `IN` left by `N` bit positions.
    Bits shifted out on the left re-enter on the right.

    - **IN**: `ANY_BIT` (BOOL, BYTE, WORD, DWORD, LWORD)
    - **N**: `ANY_INT` (rotate count)

    ```iecst
    result := ROL(IN := 16#81, N := 1); // 16#03 (BYTE)
    ```
    */
    FUNCTION ROL<T: ANY_BIT, Y: ANY_INT> : T
        VAR_INPUT
            IN: T;
            N: Y;
        END_VAR
    END_FUNCTION

    /*
    ## ROR - Rotate Right

    Rotates `IN` right by `N` bit positions.
    Bits shifted out on the right re-enter on the left.

    - **IN**: `ANY_BIT` (BOOL, BYTE, WORD, DWORD, LWORD)
    - **N**: `ANY_INT` (rotate count)

    ```iecst
    result := ROR(IN := 16#81, N := 1); // 16#C0 (BYTE)
    ```
    */
    FUNCTION ROR<T: ANY_BIT, Y: ANY_INT> : T
        VAR_INPUT
            IN: T;
            N: Y;
        END_VAR
    END_FUNCTION

END_NAMESPACE
