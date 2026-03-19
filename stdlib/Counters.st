/*
# Standard Counter Function Blocks

IEC 61131-3 **Table 36** - Up, down, and up/down counters.

All counters use `ANY_INT` type for the count value `CV`
and preset value `PV`. Edge detection is performed internally using `R_TRIG`.
*/
NAMESPACE Std.Counters
    USING Std.Edge

    /*
    ## CTU - Count Up

    Increments `CV` on each rising edge of `CU`, up to `PV`.

    - `CU`: count input (rising edge triggers)
    - `R`: reset - sets `CV` to 0
    - `PV`: preset value
    - `Q`: **TRUE** when `CV >= PV`

    ```iecst
    VAR
        counter: CTU;
    END_VAR

    counter(CU := sensor, R := reset_btn, PV := 100);
    IF counter.Q THEN
        // target count reached
    END_IF;
    ```
    */
    FUNCTION_BLOCK CTU
        VAR_INPUT
            CU: BOOL;
            R: BOOL;
            PV: ANY_INT;
        END_VAR
        VAR_OUTPUT
            Q: BOOL;
            CV: INTO(PV);
        END_VAR
        VAR
            CU_T: R_TRIG;
        END_VAR

        CU_T(CU);
        IF R THEN
            CV := 0;
        ELSIF CU_T.Q AND (CV < PV) THEN
            CV := CV + 1;
        END_IF;
        Q := (CV >= PV);
    END_FUNCTION_BLOCK

    /*
    ## CTD - Count Down

    Decrements `CV` on each rising edge of `CD`, down to 0.

    - `CD`: count input (rising edge triggers)
    - `LD`: load - sets `CV` to `PV`
    - `PV`: preset value
    - `Q`: **TRUE** when `CV <= 0`

    ```iecst
    VAR
        counter: CTD;
    END_VAR

    counter(CD := sensor, LD := load_btn, PV := 50);
    IF counter.Q THEN
        // countdown complete
    END_IF;
    ```
    */
    FUNCTION_BLOCK CTD
        VAR_INPUT
            CD: BOOL;
            LD: BOOL;
            PV: ANY_INT;
        END_VAR
        VAR_OUTPUT
            Q: BOOL;
            CV: INTO(PV);
        END_VAR
        VAR
            CD_T: R_TRIG;
        END_VAR

        CD_T(CD);
        IF LD THEN
            CV := PV;
        ELSIF CD_T.Q AND (CV > 0) THEN
            CV := CV - 1;
        END_IF;
        Q := (CV <= 0);
    END_FUNCTION_BLOCK

    /*
    ## CTUD - Count Up/Down

    Counts up on `CU` rising edges, down on `CD` rising edges.
    Simultaneous edges are ignored to prevent ambiguity.

    - `CU` / `CD`: count inputs (rising edge triggers)
    - `R`: reset - sets `CV` to 0
    - `LD`: load - sets `CV` to `PV`
    - `PV`: preset value
    - `QU`: **TRUE** when `CV >= PV`
    - `QD`: **TRUE** when `CV <= 0`

    ```iecst
    VAR
        counter: CTUD;
    END_VAR

    counter(CU := up_btn, CD := down_btn, R := FALSE, LD := FALSE, PV := 100);
    ```
    */
    FUNCTION_BLOCK CTUD
        VAR_INPUT
            CU: BOOL;
            CD: BOOL;
            R: BOOL;
            LD: BOOL;
            PV: ANY_INT;
        END_VAR
        VAR_OUTPUT
            QU: BOOL;
            QD: BOOL;
            CV: INTO(PV);
        END_VAR
        VAR
            CD_T: R_TRIG;
            CU_T: R_TRIG;
        END_VAR

        CD_T(CD);
        CU_T(CU);
        IF R THEN
            CV := 0;
        ELSIF LD THEN
            CV := PV;
        ELSE
            IF NOT (CU_T.Q AND CD_T.Q) THEN
                IF CU_T.Q AND (CV < PV) THEN
                    CV := CV + 1;
                ELSIF CD_T.Q AND (CV > 0) THEN
                    CV := CV - 1;
                END_IF;
            END_IF;
        END_IF;
        QU := (CV >= PV);
        QD := (CV <= 0);
    END_FUNCTION_BLOCK

END_NAMESPACE
