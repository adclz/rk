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

    counter(CU := sensor, reset_btn, PV := 100);
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

    counter(CU := up_btn, CD := down_btn, FALSE, LD := FALSE, PV := 100);
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

	NAMESPACE Test
		USING Std.Unit;
		USING Std.Edge;
		USING Std.Counters;

		// ── CTU (Count Up) ────────────────────────────────────────

		{test}
		FUNCTION test_ctu_counts_up
			VAR
				counter: CTU;
			END_VAR
			counter(CU := TRUE, R := FALSE, PV := 3);
			ASSERT(counter.CV = 1, 'CV should be 1 after first rising edge');
			counter(CU := FALSE, R := FALSE, PV := 3);
			counter(CU := TRUE, R := FALSE, PV := 3);
			ASSERT(counter.CV = 2, 'CV should be 2 after second rising edge');
		END_FUNCTION

		{test}
		FUNCTION test_ctu_reaches_preset
			VAR
				counter: CTU;
				i: INT;
			END_VAR
			FOR i := 1 TO 5 DO
				counter(CU := TRUE, R := FALSE, PV := 3);
				counter(CU := FALSE, R := FALSE, PV := 3);
			END_FOR;
			ASSERT(counter.Q, 'Q should be TRUE when CV >= PV');
			ASSERT(counter.CV = 3, 'CV should stop at PV');
		END_FUNCTION

		{test}
		FUNCTION test_ctu_reset
			VAR
				counter: CTU;
			END_VAR
			counter(CU := TRUE, R := FALSE, PV := 10);
			counter(CU := FALSE, R := FALSE, PV := 10);
			counter(CU := TRUE, R := FALSE, PV := 10);
			ASSERT(counter.CV = 2, 'CV should be 2 before reset');
			counter(CU := FALSE, R := TRUE, PV := 10);
			ASSERT(counter.CV = 0, 'CV should be 0 after reset');
			ASSERT(NOT counter.Q, 'Q should be FALSE after reset');
		END_FUNCTION

		{test}
		FUNCTION test_ctu_no_count_without_edge
			VAR
				counter: CTU;
			END_VAR
			counter(CU := TRUE, R := FALSE, PV := 10);
			ASSERT(counter.CV = 1, 'CV should be 1 after first call');
			counter(CU := TRUE, R := FALSE, PV := 10);
			ASSERT(counter.CV = 1, 'CV should stay 1 without falling then rising edge');
		END_FUNCTION

		// ── CTD (Count Down) ──────────────────────────────────────

		{test}
		FUNCTION test_ctd_counts_down
			VAR
				counter: CTD;
			END_VAR
			counter(CD := FALSE, LD := TRUE, PV := 3);
			ASSERT(counter.CV = 3, 'CV should be PV after load');
			counter(CD := TRUE, LD := FALSE, PV := 3);
			ASSERT(counter.CV = 2, 'CV should be 2 after first count');
			counter(CD := FALSE, LD := FALSE, PV := 3);
			counter(CD := TRUE, LD := FALSE, PV := 3);
			ASSERT(counter.CV = 1, 'CV should be 1 after second count');
		END_FUNCTION

		{test}
		FUNCTION test_ctd_reaches_zero
			VAR
				counter: CTD;
				i: INT;
			END_VAR
			counter(CD := FALSE, LD := TRUE, PV := 2);
			FOR i := 1 TO 5 DO
				counter(CD := TRUE, LD := FALSE, PV := 2);
				counter(CD := FALSE, LD := FALSE, PV := 2);
			END_FOR;
			ASSERT(counter.Q, 'Q should be TRUE when CV <= 0');
			ASSERT(counter.CV = 0, 'CV should stop at 0');
		END_FUNCTION

		{test}
		FUNCTION test_ctd_load_resets
			VAR
				counter: CTD;
			END_VAR
			counter(CD := FALSE, LD := TRUE, PV := 5);
			counter(CD := TRUE, LD := FALSE, PV := 5);
			counter(CD := FALSE, LD := FALSE, PV := 5);
			counter(CD := TRUE, LD := FALSE, PV := 5);
			ASSERT(counter.CV = 3, 'CV should be 3 after two counts');
			counter(CD := FALSE, LD := TRUE, PV := 5);
			ASSERT(counter.CV = 5, 'CV should be PV after reload');
			ASSERT(NOT counter.Q, 'Q should be FALSE after reload');
		END_FUNCTION

		// ── CTUD (Count Up/Down) ──────────────────────────────────

		{test}
		FUNCTION test_ctud_counts_up
			VAR
				counter: CTUD;
			END_VAR
			counter(CU := TRUE, CD := FALSE, R := FALSE, LD := FALSE, PV := 5);
			ASSERT(counter.CV = 1, 'CV should be 1 after count up');
			ASSERT(NOT counter.QU, 'QU should be FALSE when CV < PV');
			ASSERT(NOT counter.QD, 'QD should be FALSE when CV > 0');
		END_FUNCTION

		{test}
		FUNCTION test_ctud_counts_down
			VAR
				counter: CTUD;
			END_VAR
			counter(CU := FALSE, CD := FALSE, R := FALSE, LD := TRUE, PV := 3);
			ASSERT(counter.CV = 3, 'CV should be PV after load');
			counter(CU := FALSE, CD := TRUE, R := FALSE, LD := FALSE, PV := 3);
			ASSERT(counter.CV = 2, 'CV should be 2 after count down');
		END_FUNCTION

		{test}
		FUNCTION test_ctud_reset
			VAR
				counter: CTUD;
			END_VAR
			counter(CU := FALSE, CD := FALSE, R := FALSE, LD := TRUE, PV := 5);
			ASSERT(counter.CV = 5, 'CV should be PV after load');
			counter(CU := FALSE, CD := FALSE, R := TRUE, LD := FALSE, PV := 5);
			ASSERT(counter.CV = 0, 'CV should be 0 after reset');
			ASSERT(counter.QD, 'QD should be TRUE after reset');
		END_FUNCTION

		{test}
		FUNCTION test_ctud_load
			VAR
				counter: CTUD;
			END_VAR
			counter(CU := TRUE, CD := FALSE, R := FALSE, LD := FALSE, PV := 5);
			counter(CU := FALSE, CD := FALSE, R := FALSE, LD := FALSE, PV := 5);
			counter(CU := TRUE, CD := FALSE, R := FALSE, LD := FALSE, PV := 5);
			ASSERT(counter.CV = 2, 'CV should be 2');
			counter(CU := FALSE, CD := FALSE, R := FALSE, LD := TRUE, PV := 5);
			ASSERT(counter.CV = 5, 'CV should be PV after load');
			ASSERT(counter.QU, 'QU should be TRUE when CV >= PV');
		END_FUNCTION

		{test}
		FUNCTION test_ctud_simultaneous_edges_ignored
			VAR
				counter: CTUD;
			END_VAR
			counter(CU := FALSE, CD := FALSE, R := FALSE, LD := TRUE, PV := 3);
			ASSERT(counter.CV = 3, 'CV should be PV after load');
			// Both CU and CD rising edges simultaneously - should be ignored
			counter(CU := TRUE, CD := TRUE, R := FALSE, LD := FALSE, PV := 3);
			ASSERT(counter.CV = 3, 'CV should not change on simultaneous edges');
		END_FUNCTION

		{test}
		FUNCTION test_ctud_reset_takes_priority_over_load
			VAR
				counter: CTUD;
			END_VAR
			counter(CU := FALSE, CD := FALSE, R := FALSE, LD := TRUE, PV := 5);
			ASSERT(counter.CV = 5, 'CV should be PV after load');
			// R and LD both TRUE - R should take priority
			counter(CU := FALSE, CD := FALSE, R := TRUE, LD := TRUE, PV := 5);
			ASSERT(counter.CV = 0, 'CV should be 0 - reset takes priority over load');
		END_FUNCTION
	END_NAMESPACE
END_NAMESPACE
