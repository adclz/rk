/*
# Standard Bistable Function Blocks

IEC 61131-3 **Table 34** - Set/Reset latches.

`SR` is set-dominant (set wins on simultaneous inputs).
`RS` is reset-dominant (reset wins on simultaneous inputs).
*/
NAMESPACE Std.Bistable

    /*
    ## SR - Set-Dominant Bistable

    - `S1` = **TRUE** sets output `Q1` to TRUE
    - `R` = **TRUE** resets output `Q1` to FALSE
    - If both are TRUE, **set wins**

    ```iecst
    VAR
        latch: SR;
    END_VAR
    latch(S1 := start_btn, R := stop_btn);
    motor := latch.Q1;
    ```
    */
    FUNCTION_BLOCK SR
        VAR_INPUT
            S1, R: BOOL;
        END_VAR
        VAR_OUTPUT
            Q1: BOOL;
        END_VAR
        Q1 := S1 OR ((NOT R) AND Q1);
    END_FUNCTION_BLOCK

    /*
    ## RS - Reset-Dominant Bistable

    - `S` = **TRUE** sets output `Q1` to TRUE
    - `R1` = **TRUE** resets output `Q1` to FALSE
    - If both are TRUE, **reset wins**

    ```iecst
    VAR
        latch: RS;
    END_VAR
    latch(S := start_btn, R1 := emergency_stop);
    motor := latch.Q1;
    ```
    */
    FUNCTION_BLOCK RS
        VAR_INPUT
            S, R1: BOOL;
        END_VAR
        VAR_OUTPUT
            Q1: BOOL;
        END_VAR
        Q1 := (NOT R1) AND (S OR Q1);
    END_FUNCTION_BLOCK

END_NAMESPACE
