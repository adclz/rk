/*
Counters.

Each counts the rising edges of its input. There is one block per integer
type, named after it: `CTU_SINT`, `CTU_INT`, `CTU_DINT`, `CTU_LINT`,
`CTU_UDINT` and `CTU_ULINT`, and the same for `CTD` and `CTUD`. The plain
names [Std.Counters.CTU], [Std.Counters.CTD] and [Std.Counters.CTUD] count
in `INT`.
*/
NAMESPACE Std.Counters
	USING Std.Edge

	/* Counts up to `PV`.

	   `CV` goes up by one on each rising edge of `CU`, and stops at `PV`. `R` sets
	   it back to 0 and wins over a count. `Q` is `TRUE` when `CV` has reached
	   `PV`.

	   # Examples

	   ```iecst
	   VAR
	       counter: CTU_INT;
	   END_VAR

	   counter(CU := sensor, R := reset_btn, PV := 100);
	   IF counter.Q THEN
	       // a hundred parts went by
	   END_IF;
	   ```
	*/
	FUNCTION_BLOCK CTU_SINT
		VAR_INPUT
			CU: BOOL;
			R: BOOL;
			PV: SINT;
		END_VAR
		VAR_OUTPUT
			Q: BOOL;
			CV: SINT;
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

	// Count Up with `INT` count value
	FUNCTION_BLOCK CTU_INT
		VAR_INPUT
			CU: BOOL;
			R: BOOL;
			PV: INT;
		END_VAR
		VAR_OUTPUT
			Q: BOOL;
			CV: INT;
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

	// Count Up with `DINT` count value
	FUNCTION_BLOCK CTU_DINT
		VAR_INPUT
			CU: BOOL;
			R: BOOL;
			PV: DINT;
		END_VAR
		VAR_OUTPUT
			Q: BOOL;
			CV: DINT;
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

	// Count Up with `LINT` count value
	FUNCTION_BLOCK CTU_LINT
		VAR_INPUT
			CU: BOOL;
			R: BOOL;
			PV: LINT;
		END_VAR
		VAR_OUTPUT
			Q: BOOL;
			CV: LINT;
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

	/* Counts down from `PV`.

	   `LD` loads `CV` with `PV`. `CV` then goes down by one on each rising edge of
	   `CD`, and stops at 0. `Q` is `TRUE` when `CV` has reached 0.

	   # Examples

	   ```iecst
	   VAR
	       counter: CTD_INT;
	   END_VAR

	   counter(CD := sensor, LD := load_btn, PV := 50);
	   IF counter.Q THEN
	       // none left
	   END_IF;
	   ```
	*/
	FUNCTION_BLOCK CTD_SINT
		VAR_INPUT
			CD: BOOL;
			LD: BOOL;
			PV: SINT;
		END_VAR
		VAR_OUTPUT
			Q: BOOL;
			CV: SINT;
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

	// Count Down with `INT` count value
	FUNCTION_BLOCK CTD_INT
		VAR_INPUT
			CD: BOOL;
			LD: BOOL;
			PV: INT;
		END_VAR
		VAR_OUTPUT
			Q: BOOL;
			CV: INT;
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

	// Count Down with `DINT` count value
	FUNCTION_BLOCK CTD_DINT
		VAR_INPUT
			CD: BOOL;
			LD: BOOL;
			PV: DINT;
		END_VAR
		VAR_OUTPUT
			Q: BOOL;
			CV: DINT;
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

	// Count Down with `LINT` count value
	FUNCTION_BLOCK CTD_LINT
		VAR_INPUT
			CD: BOOL;
			LD: BOOL;
			PV: LINT;
		END_VAR
		VAR_OUTPUT
			Q: BOOL;
			CV: LINT;
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

	/* Counts up and down, between 0 and `PV`.

	   `CV` goes up on a rising edge of `CU` and down on one of `CD`, and stays
	   within 0 and `PV`. Two edges in the same call cancel out. `R` sets `CV` to 0
	   and `LD` loads it with `PV`. `R` wins over `LD`, and both win over a count.

	   `QU` is `TRUE` when `CV` has reached `PV`, and `QD` when it has reached 0.

	   # Examples

	   ```iecst
	   VAR
	       counter: CTUD_INT;
	   END_VAR

	   counter(CU := entering, CD := leaving, R := FALSE, LD := FALSE, PV := 100);
	   full := counter.QU;
	   ```
	*/
	FUNCTION_BLOCK CTUD_SINT
		VAR_INPUT
			CU: BOOL;
			CD: BOOL;
			R: BOOL;
			LD: BOOL;
			PV: SINT;
		END_VAR
		VAR_OUTPUT
			QU: BOOL;
			QD: BOOL;
			CV: SINT;
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

	// Count Up/Down with `INT` count value
	FUNCTION_BLOCK CTUD_INT
		VAR_INPUT
			CU: BOOL;
			CD: BOOL;
			R: BOOL;
			LD: BOOL;
			PV: INT;
		END_VAR
		VAR_OUTPUT
			QU: BOOL;
			QD: BOOL;
			CV: INT;
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

	// Count Up/Down with `DINT` count value
	FUNCTION_BLOCK CTUD_DINT
		VAR_INPUT
			CU: BOOL;
			CD: BOOL;
			R: BOOL;
			LD: BOOL;
			PV: DINT;
		END_VAR
		VAR_OUTPUT
			QU: BOOL;
			QD: BOOL;
			CV: DINT;
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

	// Count Up/Down with `LINT` count value
	FUNCTION_BLOCK CTUD_LINT
		VAR_INPUT
			CU: BOOL;
			CD: BOOL;
			R: BOOL;
			LD: BOOL;
			PV: LINT;
		END_VAR
		VAR_OUTPUT
			QU: BOOL;
			QD: BOOL;
			CV: LINT;
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


	/* [Std.Counters.CTU_INT] under the name the standard gives it. */
	FUNCTION_BLOCK CTU
		VAR_INPUT
			CU: BOOL;
			R: BOOL;
			PV: INT;
		END_VAR
		VAR_OUTPUT
			Q: BOOL;
			CV: INT;
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

	/* [Std.Counters.CTD_INT] under the name the standard gives it. */
	FUNCTION_BLOCK CTD
		VAR_INPUT
			CD: BOOL;
			LD: BOOL;
			PV: INT;
		END_VAR
		VAR_OUTPUT
			Q: BOOL;
			CV: INT;
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

	/* [Std.Counters.CTUD_INT] under the name the standard gives it. */
	FUNCTION_BLOCK CTUD
		VAR_INPUT
			CU: BOOL;
			CD: BOOL;
			R: BOOL;
			LD: BOOL;
			PV: INT;
		END_VAR
		VAR_OUTPUT
			QU: BOOL;
			QD: BOOL;
			CV: INT;
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

	// Count Up with `UDINT` count value
	FUNCTION_BLOCK CTU_UDINT
		VAR_INPUT
			CU: BOOL;
			R: BOOL;
			PV: UDINT;
		END_VAR
		VAR_OUTPUT
			Q: BOOL;
			CV: UDINT;
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

	// Count Up with `ULINT` count value
	FUNCTION_BLOCK CTU_ULINT
		VAR_INPUT
			CU: BOOL;
			R: BOOL;
			PV: ULINT;
		END_VAR
		VAR_OUTPUT
			Q: BOOL;
			CV: ULINT;
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

	// Count Down with `UDINT` count value
	FUNCTION_BLOCK CTD_UDINT
		VAR_INPUT
			CD: BOOL;
			LD: BOOL;
			PV: UDINT;
		END_VAR
		VAR_OUTPUT
			Q: BOOL;
			CV: UDINT;
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

	// Count Down with `ULINT` count value
	FUNCTION_BLOCK CTD_ULINT
		VAR_INPUT
			CD: BOOL;
			LD: BOOL;
			PV: ULINT;
		END_VAR
		VAR_OUTPUT
			Q: BOOL;
			CV: ULINT;
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

	// Count Up/Down with `UDINT` count value
	FUNCTION_BLOCK CTUD_UDINT
		VAR_INPUT
			CU: BOOL;
			CD: BOOL;
			R: BOOL;
			LD: BOOL;
			PV: UDINT;
		END_VAR
		VAR_OUTPUT
			QU: BOOL;
			QD: BOOL;
			CV: UDINT;
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

	// Count Up/Down with `ULINT` count value
	FUNCTION_BLOCK CTUD_ULINT
		VAR_INPUT
			CU: BOOL;
			CD: BOOL;
			R: BOOL;
			LD: BOOL;
			PV: ULINT;
		END_VAR
		VAR_OUTPUT
			QU: BOOL;
			QD: BOOL;
			CV: ULINT;
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
		USING Std.Counters;

		// -- CTU (Count Up) ----------------------------------------

		{test}
		FUNCTION test_ctu_counts_up
			VAR
				counter: CTU_INT;
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
				counter: CTU_INT;
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
				counter: CTU_INT;
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
				counter: CTU_INT;
			END_VAR
			counter(CU := TRUE, R := FALSE, PV := 10);
			ASSERT(counter.CV = 1, 'CV should be 1 after first call');
			counter(CU := TRUE, R := FALSE, PV := 10);
			ASSERT(counter.CV = 1, 'CV should stay 1 without falling then rising edge');
		END_FUNCTION

		// -- CTD (Count Down) --------------------------------------

		{test}
		FUNCTION test_ctd_counts_down
			VAR
				counter: CTD_INT;
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
				counter: CTD_INT;
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
				counter: CTD_INT;
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

		// -- CTUD (Count Up/Down) ----------------------------------

		{test}
		FUNCTION test_ctud_counts_up
			VAR
				counter: CTUD_INT;
			END_VAR
			counter(CU := TRUE, CD := FALSE, R := FALSE, LD := FALSE, PV := 5);
			ASSERT(counter.CV = 1, 'CV should be 1 after count up');
			ASSERT(NOT counter.QU, 'QU should be FALSE when CV < PV');
			ASSERT(NOT counter.QD, 'QD should be FALSE when CV > 0');
		END_FUNCTION

		{test}
		FUNCTION test_ctud_counts_down
			VAR
				counter: CTUD_INT;
			END_VAR
			counter(CU := FALSE, CD := FALSE, R := FALSE, LD := TRUE, PV := 3);
			ASSERT(counter.CV = 3, 'CV should be PV after load');
			counter(CU := FALSE, CD := TRUE, R := FALSE, LD := FALSE, PV := 3);
			ASSERT(counter.CV = 2, 'CV should be 2 after count down');
		END_FUNCTION

		{test}
		FUNCTION test_ctud_reset
			VAR
				counter: CTUD_INT;
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
				counter: CTUD_INT;
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
				counter: CTUD_INT;
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
				counter: CTUD_INT;
			END_VAR
			counter(CU := FALSE, CD := FALSE, R := FALSE, LD := TRUE, PV := 5);
			ASSERT(counter.CV = 5, 'CV should be PV after load');
			// R and LD both TRUE - R should take priority
			counter(CU := FALSE, CD := FALSE, R := TRUE, LD := TRUE, PV := 5);
			ASSERT(counter.CV = 0, 'CV should be 0 - reset takes priority over load');
		END_FUNCTION

		// --- edge semantics on the DOWN paths: a level-triggered CTD/CTUD
		// (edge detection dropped) passes every test above ---

		{test}
		FUNCTION test_ctd_no_count_without_edge
			VAR
				counter: CTD_INT;
			END_VAR
			counter(CD := FALSE, LD := TRUE, PV := 3);
			ASSERT(counter.CV = 3, 'loaded to PV');
			counter(CD := TRUE, LD := FALSE, PV := 3);
			ASSERT(counter.CV = 2, 'first rising edge decrements');
			counter(CD := TRUE, LD := FALSE, PV := 3);
			ASSERT(counter.CV = 2, 'held-high CD must not decrement again');
		END_FUNCTION

		{test}
		FUNCTION test_ctud_no_count_without_edge
			VAR
				counter: CTUD_INT;
			END_VAR
			counter(CU := TRUE, CD := FALSE, R := FALSE, LD := FALSE, PV := 5);
			ASSERT(counter.CV = 1, 'first CU edge counts');
			counter(CU := TRUE, CD := FALSE, R := FALSE, LD := FALSE, PV := 5);
			ASSERT(counter.CV = 1, 'held-high CU must not count again');
			counter(CU := FALSE, CD := TRUE, R := FALSE, LD := FALSE, PV := 5);
			ASSERT(counter.CV = 0, 'CD edge counts down');
			counter(CU := FALSE, CD := TRUE, R := FALSE, LD := FALSE, PV := 5);
			ASSERT(counter.CV = 0, 'held-high CD must not count again');
		END_FUNCTION

		// --- clamps reached by COUNTING, not only via LD/R ---

		{test}
		FUNCTION test_ctud_counts_clamp_at_bounds
			VAR
				counter: CTUD_INT;
			END_VAR
			counter(CU := TRUE, CD := FALSE, R := FALSE, LD := FALSE, PV := 2);
			counter(CU := FALSE, CD := FALSE, R := FALSE, LD := FALSE, PV := 2);
			counter(CU := TRUE, CD := FALSE, R := FALSE, LD := FALSE, PV := 2);
			ASSERT(counter.CV = 2, 'counted up to PV');
			ASSERT(counter.QU, 'QU at PV');
			counter(CU := FALSE, CD := FALSE, R := FALSE, LD := FALSE, PV := 2);
			counter(CU := TRUE, CD := FALSE, R := FALSE, LD := FALSE, PV := 2);
			ASSERT(counter.CV = 2, 'counting past PV clamps at PV');
			counter(CU := FALSE, CD := TRUE, R := FALSE, LD := FALSE, PV := 2);
			counter(CU := FALSE, CD := FALSE, R := FALSE, LD := FALSE, PV := 2);
			counter(CU := FALSE, CD := TRUE, R := FALSE, LD := FALSE, PV := 2);
			ASSERT(counter.CV = 0, 'counted back down to 0');
			ASSERT(counter.QD, 'QD at 0');
			counter(CU := FALSE, CD := FALSE, R := FALSE, LD := FALSE, PV := 2);
			counter(CU := FALSE, CD := TRUE, R := FALSE, LD := FALSE, PV := 2);
			ASSERT(counter.CV = 0, 'counting below 0 clamps at 0');
		END_FUNCTION

		// --- negative PV: Q is CV >= PV, so it must be TRUE from the start ---

		{test}
		FUNCTION test_ctu_negative_pv
			VAR
				counter: CTU_INT;
			END_VAR
			counter(CU := FALSE, R := FALSE, PV := -5);
			ASSERT(counter.Q, 'CV=0 >= PV=-5, Q immediately TRUE');
			ASSERT(counter.CV = 0, 'CV does not move without an edge');
		END_FUNCTION

		// --- width variants: each FB is independent compiled code; only the
		// _INT family is exercised above ---

		{test}
		FUNCTION test_ctu_sint_reaches_preset
			VAR
				counter: CTU_SINT;
			END_VAR
			counter(CU := TRUE, R := FALSE, PV := SINT#2);
			counter(CU := FALSE, R := FALSE, PV := SINT#2);
			counter(CU := TRUE, R := FALSE, PV := SINT#2);
			ASSERT(counter.CV = SINT#2, 'SINT counter reaches PV');
			ASSERT(counter.Q, 'SINT Q at PV');
		END_FUNCTION

		{test}
		FUNCTION test_ctd_dint_counts_down
			VAR
				counter: CTD_DINT;
			END_VAR
			counter(CD := FALSE, LD := TRUE, PV := DINT#100000);
			counter(CD := TRUE, LD := FALSE, PV := DINT#100000);
			ASSERT(counter.CV = DINT#99999, 'DINT counter decrements');
		END_FUNCTION

		{test}
		FUNCTION test_ctd_lint_i64_lane
			VAR
				counter: CTD_LINT;
			END_VAR
			counter(CD := FALSE, LD := TRUE, PV := LINT#5000000000);
			ASSERT(counter.CV = LINT#5000000000, 'i64-lane load beyond i32 range');
			counter(CD := TRUE, LD := FALSE, PV := LINT#5000000000);
			ASSERT(counter.CV = LINT#4999999999, 'i64-lane decrement');
		END_FUNCTION

		{test}
		FUNCTION test_ctud_lint_up_down
			VAR
				counter: CTUD_LINT;
			END_VAR
			counter(CU := TRUE, CD := FALSE, R := FALSE, LD := FALSE, PV := LINT#3);
			ASSERT(counter.CV = LINT#1, 'LINT CTUD counts up');
			counter(CU := FALSE, CD := TRUE, R := FALSE, LD := FALSE, PV := LINT#3);
			ASSERT(counter.CV = LINT#0, 'LINT CTUD counts down');
		END_FUNCTION

		// -- standard base names ------------------------------------

		{test}
		FUNCTION test_plain_ctu_counts_to_pv
			VAR
				c: CTU;
			END_VAR
			c(CU := TRUE, R := FALSE, PV := 2);
			c(CU := FALSE, R := FALSE, PV := 2);
			c(CU := TRUE, R := FALSE, PV := 2);
			ASSERT_EQ(c.CV, 2);
			ASSERT(c.Q, 'CTU Q at PV');
		END_FUNCTION

		{test}
		FUNCTION test_plain_ctd_counts_down
			VAR
				c: CTD;
			END_VAR
			c(CD := FALSE, LD := TRUE, PV := 2);
			ASSERT_EQ(c.CV, 2);
			c(CD := TRUE, LD := FALSE, PV := 2);
			c(CD := FALSE, LD := FALSE, PV := 2);
			c(CD := TRUE, LD := FALSE, PV := 2);
			ASSERT_EQ(c.CV, 0);
			ASSERT(c.Q, 'CTD Q at zero');
		END_FUNCTION

		{test}
		FUNCTION test_plain_ctud_both_directions
			VAR
				c: CTUD;
			END_VAR
			c(CU := TRUE, CD := FALSE, R := FALSE, LD := FALSE, PV := 3);
			c(CU := FALSE, CD := FALSE, R := FALSE, LD := FALSE, PV := 3);
			c(CU := TRUE, CD := FALSE, R := FALSE, LD := FALSE, PV := 3);
			ASSERT_EQ(c.CV, 2);
			c(CU := FALSE, CD := TRUE, R := FALSE, LD := FALSE, PV := 3);
			ASSERT_EQ(c.CV, 1);
			ASSERT(NOT c.QU);
			ASSERT(NOT c.QD);
		END_FUNCTION

		{test}
		FUNCTION test_ctu_udint_counts
			VAR
				c: CTU_UDINT;
			END_VAR
			c(CU := TRUE, R := FALSE, PV := 1);
			ASSERT(c.Q, 'unsigned counter reaches PV');
		END_FUNCTION

		{test}
		FUNCTION test_ctd_ulint_loads_and_counts
			VAR
				c: CTD_ULINT;
			END_VAR
			c(CD := FALSE, LD := TRUE, PV := 1);
			c(CD := TRUE, LD := FALSE, PV := 1);
			ASSERT(c.Q, 'unsigned down-counter reaches zero');
		END_FUNCTION
	END_NAMESPACE
END_NAMESPACE
