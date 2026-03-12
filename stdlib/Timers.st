/*
# Standard Timer Function Blocks

IEC 61131-3 **Table 37** - Pulse, on-delay, and off-delay timers.

All timers share a common interface:
- `IN`: BOOL trigger input
- `PT`: TIME preset duration
- `Q`: BOOL output
- `ET`: TIME elapsed time
*/
NAMESPACE Std.Timers

    /*
    ## TP - Pulse Timer

    Generates a pulse of duration `PT` on a rising edge of `IN`.

    - Rising edge on `IN` starts the timer and sets `Q` to TRUE
    - `Q` remains TRUE for exactly `PT` duration, regardless of `IN`
    - `ET` tracks elapsed time, capped at `PT`

    ```iecst
    VAR
        pulse: TP;
    END_VAR
    
    pulse(IN := trigger, PT := T#5s);
    IF pulse.Q THEN
        // pulse is active
    END_IF;
    ```
    */
    FUNCTION_BLOCK TP
        VAR_INPUT
            IN: BOOL;
            PT: TIME;
        END_VAR
        VAR_OUTPUT
            Q: BOOL := FALSE;
            ET: TIME := T#0s;
        END_VAR
        VAR
            STATE: SINT := 0;
            PREV_IN: BOOL := FALSE;
            CURRENT_TIME, START_TIME: TIME;
        END_VAR

        IF ((STATE = 0) AND NOT(PREV_IN) AND IN) THEN
            STATE := 1;
            Q := TRUE;
            START_TIME := CURRENT_TIME;
        ELSIF (STATE = 1) THEN
            IF ((START_TIME + PT) <= CURRENT_TIME) THEN
                STATE := 2;
                Q := FALSE;
                ET := PT;
            ELSE
                ET := CURRENT_TIME - START_TIME;
            END_IF;
        END_IF;

        IF ((STATE = 2) AND NOT(IN)) THEN
            ET := T#0s;
            STATE := 0;
        END_IF;

        PREV_IN := IN;
    END_FUNCTION_BLOCK

    /*
    ## TON - On-Delay Timer

    Delays the rising edge of `IN` by `PT`.

    - Rising edge on `IN` starts the timer
    - `Q` becomes TRUE after `PT` has elapsed, while `IN` is still TRUE
    - `Q` resets immediately when `IN` goes FALSE
    - `ET` tracks elapsed time, capped at `PT`

    ```iecst
    VAR
        delay: TON;
    END_VAR
    
    delay(IN := start_signal, PT := T#3s);
    IF delay.Q THEN
        // 3 seconds have passed since start_signal went TRUE
    END_IF;
    ```
    */
    FUNCTION_BLOCK TON
        VAR_INPUT
            IN: BOOL;
            PT: TIME;
        END_VAR
        VAR_OUTPUT
            Q: BOOL := FALSE;
            ET: TIME := T#0s;
        END_VAR
        VAR
            STATE: SINT := 0;
            PREV_IN: BOOL := FALSE;
            CURRENT_TIME, START_TIME: TIME;
        END_VAR

        IF ((STATE = 0) AND NOT(PREV_IN) AND IN) THEN
            STATE := 1;
            Q := FALSE;
            START_TIME := CURRENT_TIME;
        ELSE
            IF (NOT(IN)) THEN
                ET := T#0s;
                Q := FALSE;
                STATE := 0;
            ELSIF (STATE = 1) THEN
                IF ((START_TIME + PT) <= CURRENT_TIME) THEN
                    STATE := 2;
                    Q := TRUE;
                    ET := PT;
                ELSE
                    ET := CURRENT_TIME - START_TIME;
                END_IF;
            END_IF;
        END_IF;

        PREV_IN := IN;
    END_FUNCTION_BLOCK

    /*
    ## TOF - Off-Delay Timer

    Delays the falling edge of `IN` by `PT`.

    - Falling edge on `IN` starts the timer, `Q` remains TRUE
    - `Q` becomes FALSE after `PT` has elapsed
    - `Q` resets immediately when `IN` goes TRUE again
    - `ET` tracks elapsed time, capped at `PT`

    ```iecst
    VAR
        off_delay: TOF;
    END_VAR
    
    off_delay(IN := motor_running, PT := T#10s);
    cooling_fan := off_delay.Q; // fan runs 10s after motor stops
    ```
    */
    FUNCTION_BLOCK TOF
        VAR_INPUT
            IN: BOOL;
            PT: TIME;
        END_VAR
        VAR_OUTPUT
            Q: BOOL := FALSE;
            ET: TIME := T#0s;
        END_VAR
        VAR
            STATE: SINT := 0;
            PREV_IN: BOOL := FALSE;
            CURRENT_TIME, START_TIME: TIME;
        END_VAR

        IF ((STATE = 0) AND PREV_IN AND NOT(IN)) THEN
            STATE := 1;
            START_TIME := CURRENT_TIME;
        ELSE
            IF (IN) THEN
                ET := T#0s;
                STATE := 0;
            ELSIF (STATE = 1) THEN
                IF ((START_TIME + PT) <= CURRENT_TIME) THEN
                    STATE := 2;
                    ET := PT;
                ELSE
                    ET := CURRENT_TIME - START_TIME;
                END_IF;
            END_IF;
        END_IF;

        Q := IN OR (STATE = 1);
        PREV_IN := IN;
    END_FUNCTION_BLOCK

END_NAMESPACE
