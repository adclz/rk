/*
# Standard Edge Detection Function Blocks

IEC 61131-3 **Table 35** - Rising and falling edge detectors.

Both blocks latch the previous `CLK` value to detect transitions.
*/
NAMESPACE Std.Edge

    /*
    ## R_TRIG - Rising Edge Detector

    Output `Q` is **TRUE** for one cycle when `CLK` transitions from FALSE to TRUE.

    ```iecst
    VAR
        trigger: R_TRIG;
    END_VAR
    trigger(CLK := my_input);
    IF trigger.Q THEN
        // rising edge detected
    END_IF;
    ```
    */
    FUNCTION_BLOCK R_TRIG
        VAR_INPUT
            CLK: BOOL;
        END_VAR
        VAR_OUTPUT
            Q: BOOL;
        END_VAR
        VAR
            M: BOOL;
        END_VAR
        Q := CLK AND NOT M;
        M := CLK;
    END_FUNCTION_BLOCK

    /*
    ## F_TRIG - Falling Edge Detector

    Output `Q` is **TRUE** for one cycle when `CLK` transitions from TRUE to FALSE.

    ```iecst
    VAR
        trigger: F_TRIG;
    END_VAR
    trigger(CLK := my_input);
    IF trigger.Q THEN
        // falling edge detected
    END_IF;
    ```
    */
    FUNCTION_BLOCK F_TRIG
        VAR_INPUT
            CLK: BOOL;
        END_VAR
        VAR_OUTPUT
            Q: BOOL;
        END_VAR
        VAR
            M: BOOL;
        END_VAR
        Q := NOT CLK AND NOT M;
        M := NOT CLK;
    END_FUNCTION_BLOCK

END_NAMESPACE
