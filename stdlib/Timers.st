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
    ## WASI monotonic clock

    Provides a monotonic clock for accurate timer implementations. 
    
    A monotonic clock is a clock that guarantees non-decreasing time values, unaffected by system clock adjustments. 
    
    [See WASI documentation](https://github.com/WebAssembly/WASI/blob/main/proposals/clocks/README.md)
    */
    FUNCTION MONOTONIC_TIME : TIME
        {extern 'wasi:clocks/monotonic-clock@0.2.6' 'now' (result MONOTONIC_TIME)}
    END_FUNCTION

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
    
    pulse(trigger, PT := T#5s);
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

        CURRENT_TIME := MONOTONIC_TIME();

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
    
    delay(start_signal, PT := T#3s);
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

        CURRENT_TIME := MONOTONIC_TIME();

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
    
    off_delay(motor_running, PT := T#10s);
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

        CURRENT_TIME := MONOTONIC_TIME();

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

    NAMESPACE Test
        USING Std.Unit;
        USING Std.Timers;

        // -- TON (On-Delay Timer) ------------------------------------

        {test}
        FUNCTION test_ton_expires
        VAR timer: TON; END_VAR
            timer(IN := TRUE, PT := T#1ms);
            WHILE NOT timer.Q DO
                timer(IN := TRUE, PT := T#1ms);
            END_WHILE;
            ASSERT(timer.Q, 'TON should have expired');
        END_FUNCTION

        {test}
        FUNCTION test_ton_not_started
        VAR timer: TON; END_VAR
            timer(IN := FALSE, PT := T#1ms);
            ASSERT(NOT timer.Q, 'TON Q should be FALSE when IN is FALSE');
        END_FUNCTION

        {test}
        FUNCTION test_ton_reset
        VAR timer: TON; END_VAR
            timer(IN := TRUE, PT := T#100ms);
            ASSERT(NOT timer.Q, 'TON Q should be FALSE immediately');
            timer(IN := FALSE, PT := T#100ms);
            ASSERT(NOT timer.Q, 'TON Q should be FALSE after reset');
            ASSERT(timer.ET = T#0s, 'TON ET should be 0 after reset');
        END_FUNCTION

        // -- TOF (Off-Delay Timer) -----------------------------------

        {test}
        FUNCTION test_tof_stays_true_after_falling_edge
        VAR timer: TOF; END_VAR
            // Rising edge: Q becomes TRUE immediately
            timer(IN := TRUE, PT := T#100ms);
            ASSERT(timer.Q, 'TOF Q should be TRUE when IN is TRUE');
            // Falling edge: Q stays TRUE during delay
            timer(IN := FALSE, PT := T#100ms);
            ASSERT(timer.Q, 'TOF Q should stay TRUE during off-delay');
        END_FUNCTION

        {test}
        FUNCTION test_tof_expires
        VAR timer: TOF; END_VAR
            timer(IN := TRUE, PT := T#1ms);
            timer(IN := FALSE, PT := T#1ms);
            // Spin until Q becomes FALSE
            WHILE timer.Q DO
                timer(IN := FALSE, PT := T#1ms);
            END_WHILE;
            ASSERT(NOT timer.Q, 'TOF Q should be FALSE after delay expired');
        END_FUNCTION

        {test}
        FUNCTION test_tof_reset_on_rising_edge
        VAR timer: TOF; END_VAR
            timer(IN := TRUE, PT := T#1ms);
            timer(IN := FALSE, PT := T#1ms);
            // Re-assert IN before timer expires
            timer(IN := TRUE, PT := T#1ms);
            ASSERT(timer.Q, 'TOF Q should be TRUE after re-assertion');
        END_FUNCTION

        // -- TP (Pulse Timer) ----------------------------------------

        {test}
        FUNCTION test_tp_pulse_starts
        VAR timer: TP; END_VAR
            timer(IN := TRUE, PT := T#100ms);
            ASSERT(timer.Q, 'TP Q should be TRUE on rising edge');
        END_FUNCTION

        {test}
        FUNCTION test_tp_pulse_expires
        VAR timer: TP; END_VAR
            timer(IN := TRUE, PT := T#1ms);
            WHILE timer.Q DO
                timer(IN := TRUE, PT := T#1ms);
            END_WHILE;
            ASSERT(NOT timer.Q, 'TP Q should be FALSE after pulse expired');
        END_FUNCTION

        {test}
        FUNCTION test_tp_ignores_input_during_pulse
        VAR timer: TP; END_VAR
            timer(IN := TRUE, PT := T#100ms);
            ASSERT(timer.Q, 'TP Q should be TRUE');
            // Dropping IN during pulse doesn't affect Q
            timer(IN := FALSE, PT := T#100ms);
            ASSERT(timer.Q, 'TP Q should stay TRUE during pulse even if IN drops');
        END_FUNCTION

    END_NAMESPACE

END_NAMESPACE
