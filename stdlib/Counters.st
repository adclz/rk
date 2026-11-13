/*
Counters.

Each counts the rising edges of its input in one integer type, named after
it: `CTU_SINT`, `CTU_INT`, `CTU_DINT`, `CTU_LINT`, `CTU_UDINT` and
`CTU_ULINT`, and the same for `CTD` and `CTUD`. The plain names
[Std.Counters.CTU], [Std.Counters.CTD] and [Std.Counters.CTUD] are the `INT`
ones.

`CV` does not stop at `PV` or at 0: it counts up to the largest value of its
type and down to the smallest, 0 for `UDINT` and `ULINT`. These are the two
limits the standard leaves to each implementation. `Q`, `QU` and `QD` tell
where `CV` stands against `PV` and 0.
*/
NAMESPACE Std.Counters
	USING Std.Edge;

	/* [Std.Counters.CTU_INT] in `SINT`. */
	{must_call}
	{once}
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
		ELSIF CU_T.Q AND (CV < SINT#127) THEN
			CV := CV + 1;
		END_IF;
		Q := (CV >= PV);
	END_FUNCTION_BLOCK

	/* Counts up.

	   `CV` goes up by one on each rising edge of `CU`, past `PV`, up to the
	   largest value of its type. `R` sets it back to 0 and wins over a count.
	   `Q` is `TRUE` while `CV` is at `PV` or above.

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
	{must_call}
	{once}
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
		ELSIF CU_T.Q AND (CV < INT#32767) THEN
			CV := CV + 1;
		END_IF;
		Q := (CV >= PV);
	END_FUNCTION_BLOCK

	/* [Std.Counters.CTU_INT] in `DINT`. */
	{must_call}
	{once}
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
		ELSIF CU_T.Q AND (CV < DINT#2147483647) THEN
			CV := CV + 1;
		END_IF;
		Q := (CV >= PV);
	END_FUNCTION_BLOCK

	/* [Std.Counters.CTU_INT] in `LINT`. */
	{must_call}
	{once}
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
		ELSIF CU_T.Q AND (CV < LINT#9223372036854775807) THEN
			CV := CV + 1;
		END_IF;
		Q := (CV >= PV);
	END_FUNCTION_BLOCK

	/* [Std.Counters.CTD_INT] in `SINT`. */
	{must_call}
	{once}
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
		ELSIF CD_T.Q AND (CV > SINT#-128) THEN
			CV := CV - 1;
		END_IF;
		Q := (CV <= 0);
	END_FUNCTION_BLOCK

	/* Counts down from `PV`.

	   `LD` loads `CV` with `PV`. `CV` then goes down by one on each rising edge of
	   `CD`, past 0, down to the smallest value of its type. `Q` is `TRUE` while
	   `CV` is at 0 or below.

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
	{must_call}
	{once}
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
		ELSIF CD_T.Q AND (CV > INT#-32768) THEN
			CV := CV - 1;
		END_IF;
		Q := (CV <= 0);
	END_FUNCTION_BLOCK

	/* [Std.Counters.CTD_INT] in `DINT`. */
	{must_call}
	{once}
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
		ELSIF CD_T.Q AND (CV > DINT#-2147483648) THEN
			CV := CV - 1;
		END_IF;
		Q := (CV <= 0);
	END_FUNCTION_BLOCK

	/* [Std.Counters.CTD_INT] in `LINT`. */
	{must_call}
	{once}
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
		ELSIF CD_T.Q AND (CV > LINT#-9223372036854775808) THEN
			CV := CV - 1;
		END_IF;
		Q := (CV <= 0);
	END_FUNCTION_BLOCK

	/* [Std.Counters.CTUD_INT] in `SINT`. */
	{must_call}
	{once}
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
				IF CU_T.Q AND (CV < SINT#127) THEN
					CV := CV + 1;
				ELSIF CD_T.Q AND (CV > SINT#-128) THEN
					CV := CV - 1;
				END_IF;
			END_IF;
		END_IF;
		QU := (CV >= PV);
		QD := (CV <= 0);
	END_FUNCTION_BLOCK

	/* Counts up and down.

	   `CV` goes up on a rising edge of `CU` and down on one of `CD`, past `PV`
	   and 0, within the values of its type. Two edges in the same call cancel
	   out. `R` sets `CV` to 0 and `LD` loads it with `PV`. `R` wins over `LD`,
	   and both win over a count.

	   `QU` is `TRUE` while `CV` is at `PV` or above, and `QD` while it is at 0
	   or below.

	   # Examples

	   ```iecst
	   VAR
	       counter: CTUD_INT;
	   END_VAR

	   counter(CU := entering, CD := leaving, R := FALSE, LD := FALSE, PV := 100);
	   full := counter.QU;
	   ```
	*/
	{must_call}
	{once}
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
				IF CU_T.Q AND (CV < INT#32767) THEN
					CV := CV + 1;
				ELSIF CD_T.Q AND (CV > INT#-32768) THEN
					CV := CV - 1;
				END_IF;
			END_IF;
		END_IF;
		QU := (CV >= PV);
		QD := (CV <= 0);
	END_FUNCTION_BLOCK

	/* [Std.Counters.CTUD_INT] in `DINT`. */
	{must_call}
	{once}
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
				IF CU_T.Q AND (CV < DINT#2147483647) THEN
					CV := CV + 1;
				ELSIF CD_T.Q AND (CV > DINT#-2147483648) THEN
					CV := CV - 1;
				END_IF;
			END_IF;
		END_IF;
		QU := (CV >= PV);
		QD := (CV <= 0);
	END_FUNCTION_BLOCK

	/* [Std.Counters.CTUD_INT] in `LINT`. */
	{must_call}
	{once}
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
				IF CU_T.Q AND (CV < LINT#9223372036854775807) THEN
					CV := CV + 1;
				ELSIF CD_T.Q AND (CV > LINT#-9223372036854775808) THEN
					CV := CV - 1;
				END_IF;
			END_IF;
		END_IF;
		QU := (CV >= PV);
		QD := (CV <= 0);
	END_FUNCTION_BLOCK


	/* [Std.Counters.CTU_INT] under the name the standard gives it. */
	TYPE CTU: CTU_INT END_TYPE

	/* [Std.Counters.CTD_INT] under the name the standard gives it. */
	TYPE CTD: CTD_INT END_TYPE

	/* [Std.Counters.CTUD_INT] under the name the standard gives it. */
	TYPE CTUD: CTUD_INT END_TYPE

	/* [Std.Counters.CTU_INT] in `UDINT`. */
	{must_call}
	{once}
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
		ELSIF CU_T.Q AND (CV < UDINT#4294967295) THEN
			CV := CV + 1;
		END_IF;
		Q := (CV >= PV);
	END_FUNCTION_BLOCK

	/* [Std.Counters.CTU_INT] in `ULINT`. */
	{must_call}
	{once}
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
		ELSIF CU_T.Q AND (CV < ULINT#18446744073709551615) THEN
			CV := CV + 1;
		END_IF;
		Q := (CV >= PV);
	END_FUNCTION_BLOCK

	/* [Std.Counters.CTD_INT] in `UDINT`. */
	{must_call}
	{once}
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

	/* [Std.Counters.CTD_INT] in `ULINT`. */
	{must_call}
	{once}
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

	/* [Std.Counters.CTUD_INT] in `UDINT`. */
	{must_call}
	{once}
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
				IF CU_T.Q AND (CV < UDINT#4294967295) THEN
					CV := CV + 1;
				ELSIF CD_T.Q AND (CV > 0) THEN
					CV := CV - 1;
				END_IF;
			END_IF;
		END_IF;
		QU := (CV >= PV);
		QD := (CV <= 0);
	END_FUNCTION_BLOCK

	/* [Std.Counters.CTUD_INT] in `ULINT`. */
	{must_call}
	{once}
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
				IF CU_T.Q AND (CV < ULINT#18446744073709551615) THEN
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

		// A test that drives a counter over several calls is a block: it
		// calls the counter once per cycle, as a program does, with the
		// inputs of the cycle's step.

		// -- CTU (Count Up) ----------------------------------------

		{test}
		FUNCTION_BLOCK test_ctu_counts_up
			VAR_OUTPUT
				DONE: BOOL;
			END_VAR
			VAR
				counter: CTU_INT;
				step: INT;
			END_VAR
			// CU rises at steps 0 and 2.
			counter(CU := step <> 1, R := FALSE, PV := 3);
			IF step = 0 THEN
				ASSERT(counter.CV = 1, 'CV should be 1 after first rising edge');
			ELSIF step = 2 THEN
				ASSERT(counter.CV = 2, 'CV should be 2 after second rising edge');
				DONE := TRUE;
			END_IF;
			step := step + 1;
		END_FUNCTION_BLOCK

		{test}
		FUNCTION_BLOCK test_ctu_counts_past_preset
			VAR_OUTPUT
				DONE: BOOL;
			END_VAR
			VAR
				counter: CTU_INT;
				step: INT;
			END_VAR
			// CU rises every other cycle: five times.
			counter(CU := step MOD 2 = 0, R := FALSE, PV := 3);
			IF step = 4 THEN
				ASSERT(counter.Q AND counter.CV = 3, 'Q should be TRUE when CV reaches PV');
			ELSIF step = 9 THEN
				ASSERT(counter.CV = 5, 'CV should count on past PV');
				ASSERT(counter.Q, 'Q should stay TRUE past PV');
				DONE := TRUE;
			END_IF;
			step := step + 1;
		END_FUNCTION_BLOCK

		{test}
		FUNCTION_BLOCK test_ctu_reset
			VAR_OUTPUT
				DONE: BOOL;
			END_VAR
			VAR
				counter: CTU_INT;
				step: INT;
			END_VAR
			// CU rises at steps 0 and 2, R is set at step 3.
			counter(CU := step = 0 OR step = 2, R := step = 3, PV := 10);
			IF step = 2 THEN
				ASSERT(counter.CV = 2, 'CV should be 2 before reset');
			ELSIF step = 3 THEN
				ASSERT(counter.CV = 0, 'CV should be 0 after reset');
				ASSERT(NOT counter.Q, 'Q should be FALSE after reset');
				DONE := TRUE;
			END_IF;
			step := step + 1;
		END_FUNCTION_BLOCK

		{test}
		FUNCTION_BLOCK test_ctu_no_count_without_edge
			VAR_OUTPUT
				DONE: BOOL;
			END_VAR
			VAR
				counter: CTU_INT;
				step: INT;
			END_VAR
			// CU held TRUE: one rising edge.
			counter(CU := TRUE, R := FALSE, PV := 10);
			IF step = 0 THEN
				ASSERT(counter.CV = 1, 'CV should be 1 after first call');
			ELSE
				ASSERT(counter.CV = 1, 'CV should stay 1 without falling then rising edge');
				DONE := TRUE;
			END_IF;
			step := step + 1;
		END_FUNCTION_BLOCK

		// -- CTD (Count Down) --------------------------------------

		{test}
		FUNCTION_BLOCK test_ctd_counts_down
			VAR_OUTPUT
				DONE: BOOL;
			END_VAR
			VAR
				counter: CTD_INT;
				step: INT;
			END_VAR
			// LD at step 0, CD rises at steps 1 and 3.
			counter(CD := step = 1 OR step = 3, LD := step = 0, PV := 3);
			IF step = 0 THEN
				ASSERT(counter.CV = 3, 'CV should be PV after load');
			ELSIF step = 1 THEN
				ASSERT(counter.CV = 2, 'CV should be 2 after first count');
			ELSIF step = 3 THEN
				ASSERT(counter.CV = 1, 'CV should be 1 after second count');
				DONE := TRUE;
			END_IF;
			step := step + 1;
		END_FUNCTION_BLOCK

		{test}
		FUNCTION_BLOCK test_ctd_counts_below_zero
			VAR_OUTPUT
				DONE: BOOL;
			END_VAR
			VAR
				counter: CTD_INT;
				step: INT;
			END_VAR
			// LD at step 0, then CD rises every other cycle: five times.
			counter(CD := step MOD 2 = 1, LD := step = 0, PV := 2);
			IF step = 3 THEN
				ASSERT(counter.Q AND counter.CV = 0, 'Q should be TRUE when CV reaches 0');
			ELSIF step = 10 THEN
				ASSERT(counter.CV = -3, 'CV should count on below 0');
				ASSERT(counter.Q, 'Q should stay TRUE below 0');
				DONE := TRUE;
			END_IF;
			step := step + 1;
		END_FUNCTION_BLOCK

		{test}
		FUNCTION_BLOCK test_ctd_load_resets
			VAR_OUTPUT
				DONE: BOOL;
			END_VAR
			VAR
				counter: CTD_INT;
				step: INT;
			END_VAR
			// LD at steps 0 and 4, CD rises at steps 1 and 3.
			counter(CD := step = 1 OR step = 3, LD := step = 0 OR step = 4, PV := 5);
			IF step = 3 THEN
				ASSERT(counter.CV = 3, 'CV should be 3 after two counts');
			ELSIF step = 4 THEN
				ASSERT(counter.CV = 5, 'CV should be PV after reload');
				ASSERT(NOT counter.Q, 'Q should be FALSE after reload');
				DONE := TRUE;
			END_IF;
			step := step + 1;
		END_FUNCTION_BLOCK

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
		FUNCTION_BLOCK test_ctud_counts_down
			VAR_OUTPUT
				DONE: BOOL;
			END_VAR
			VAR
				counter: CTUD_INT;
				step: INT;
			END_VAR
			// LD at step 0, CD rises at step 1.
			counter(CU := FALSE, CD := step = 1, R := FALSE, LD := step = 0, PV := 3);
			IF step = 0 THEN
				ASSERT(counter.CV = 3, 'CV should be PV after load');
			ELSE
				ASSERT(counter.CV = 2, 'CV should be 2 after count down');
				DONE := TRUE;
			END_IF;
			step := step + 1;
		END_FUNCTION_BLOCK

		{test}
		FUNCTION_BLOCK test_ctud_reset
			VAR_OUTPUT
				DONE: BOOL;
			END_VAR
			VAR
				counter: CTUD_INT;
				step: INT;
			END_VAR
			// LD at step 0, R at step 1.
			counter(CU := FALSE, CD := FALSE, R := step = 1, LD := step = 0, PV := 5);
			IF step = 0 THEN
				ASSERT(counter.CV = 5, 'CV should be PV after load');
			ELSE
				ASSERT(counter.CV = 0, 'CV should be 0 after reset');
				ASSERT(counter.QD, 'QD should be TRUE after reset');
				DONE := TRUE;
			END_IF;
			step := step + 1;
		END_FUNCTION_BLOCK

		{test}
		FUNCTION_BLOCK test_ctud_load
			VAR_OUTPUT
				DONE: BOOL;
			END_VAR
			VAR
				counter: CTUD_INT;
				step: INT;
			END_VAR
			// CU rises at steps 0 and 2, LD at step 3.
			counter(CU := step = 0 OR step = 2, CD := FALSE, R := FALSE, LD := step = 3, PV := 5);
			IF step = 2 THEN
				ASSERT(counter.CV = 2, 'CV should be 2');
			ELSIF step = 3 THEN
				ASSERT(counter.CV = 5, 'CV should be PV after load');
				ASSERT(counter.QU, 'QU should be TRUE when CV >= PV');
				DONE := TRUE;
			END_IF;
			step := step + 1;
		END_FUNCTION_BLOCK

		{test}
		FUNCTION_BLOCK test_ctud_simultaneous_edges_ignored
			VAR_OUTPUT
				DONE: BOOL;
			END_VAR
			VAR
				counter: CTUD_INT;
				step: INT;
			END_VAR
			// LD at step 0, CU and CD rise together at step 1.
			counter(CU := step = 1, CD := step = 1, R := FALSE, LD := step = 0, PV := 3);
			IF step = 0 THEN
				ASSERT(counter.CV = 3, 'CV should be PV after load');
			ELSE
				ASSERT(counter.CV = 3, 'CV should not change on simultaneous edges');
				DONE := TRUE;
			END_IF;
			step := step + 1;
		END_FUNCTION_BLOCK

		{test}
		FUNCTION_BLOCK test_ctud_reset_takes_priority_over_load
			VAR_OUTPUT
				DONE: BOOL;
			END_VAR
			VAR
				counter: CTUD_INT;
				step: INT;
			END_VAR
			// LD held, R set at step 1.
			counter(CU := FALSE, CD := FALSE, R := step = 1, LD := TRUE, PV := 5);
			IF step = 0 THEN
				ASSERT(counter.CV = 5, 'CV should be PV after load');
			ELSE
				ASSERT(counter.CV = 0, 'CV should be 0 - reset takes priority over load');
				DONE := TRUE;
			END_IF;
			step := step + 1;
		END_FUNCTION_BLOCK

		// --- edge semantics on the DOWN paths: a level-triggered CTD/CTUD
		// (edge detection dropped) passes every test above ---

		{test}
		FUNCTION_BLOCK test_ctd_no_count_without_edge
			VAR_OUTPUT
				DONE: BOOL;
			END_VAR
			VAR
				counter: CTD_INT;
				step: INT;
			END_VAR
			// LD at step 0, then CD held TRUE: one rising edge.
			counter(CD := step > 0, LD := step = 0, PV := 3);
			IF step = 0 THEN
				ASSERT(counter.CV = 3, 'loaded to PV');
			ELSIF step = 1 THEN
				ASSERT(counter.CV = 2, 'first rising edge decrements');
			ELSE
				ASSERT(counter.CV = 2, 'held-high CD must not decrement again');
				DONE := TRUE;
			END_IF;
			step := step + 1;
		END_FUNCTION_BLOCK

		{test}
		FUNCTION_BLOCK test_ctud_no_count_without_edge
			VAR_OUTPUT
				DONE: BOOL;
			END_VAR
			VAR
				counter: CTUD_INT;
				step: INT;
			END_VAR
			// CU held for steps 0 and 1, then CD for steps 2 and 3.
			counter(CU := step < 2, CD := step >= 2, R := FALSE, LD := FALSE, PV := 5);
			IF step = 0 THEN
				ASSERT(counter.CV = 1, 'first CU edge counts');
			ELSIF step = 1 THEN
				ASSERT(counter.CV = 1, 'held-high CU must not count again');
			ELSIF step = 2 THEN
				ASSERT(counter.CV = 0, 'CD edge counts down');
			ELSE
				ASSERT(counter.CV = 0, 'held-high CD must not count again');
				DONE := TRUE;
			END_IF;
			step := step + 1;
		END_FUNCTION_BLOCK

		// --- PV and 0 are no limits: CV counts on past them ---

		{test}
		FUNCTION_BLOCK test_ctud_counts_past_pv_and_below_zero
			VAR_OUTPUT
				DONE: BOOL;
			END_VAR
			VAR
				counter: CTUD_INT;
				step: INT;
			END_VAR
			// CU rises at steps 0, 2 and 4, then CD at steps 6, 8, 10 and 12.
			counter(
				CU := step < 6 AND step MOD 2 = 0,
				CD := step >= 6 AND step MOD 2 = 0,
				R := FALSE,
				LD := FALSE,
				PV := 2);
			IF step = 2 THEN
				ASSERT(counter.CV = 2 AND counter.QU, 'counted up to PV');
			ELSIF step = 4 THEN
				ASSERT(counter.CV = 3 AND counter.QU, 'counting on past PV');
			ELSIF step = 10 THEN
				ASSERT(counter.CV = 0 AND counter.QD, 'counted back down to 0');
			ELSIF step = 12 THEN
				ASSERT(counter.CV = -1 AND counter.QD, 'counting on below 0');
				DONE := TRUE;
			END_IF;
			step := step + 1;
		END_FUNCTION_BLOCK

		// --- the limits are the type's: CV stops at the largest and the
		// smallest value it holds, and never wraps around ---

		{test}
		FUNCTION_BLOCK test_ctu_stops_at_the_largest_of_its_type
			VAR_OUTPUT
				DONE: BOOL;
			END_VAR
			VAR
				s: CTU_SINT;
				i: CTU_INT;
				step: DINT;
			END_VAR
			// CU rises every other cycle, one time more than an INT holds.
			// Wider counters cannot be counted that far.
			s(CU := step MOD 2 = 0, R := FALSE, PV := SINT#100);
			i(CU := step MOD 2 = 0, R := FALSE, PV := INT#100);
			IF step = DINT#65536 THEN
				ASSERT(s.CV = SINT#127, 'CTU_SINT stops at the largest SINT');
				ASSERT(i.CV = INT#32767, 'CTU_INT stops at the largest INT');
				DONE := TRUE;
			END_IF;
			step := step + 1;
		END_FUNCTION_BLOCK

		{test}
		FUNCTION_BLOCK test_ctd_stops_at_the_smallest_of_its_type
			VAR_OUTPUT
				DONE: BOOL;
			END_VAR
			VAR
				s: CTD_SINT;
				i: CTD_INT;
				d: CTD_DINT;
				l: CTD_LINT;
				ud: CTD_UDINT;
				ul: CTD_ULINT;
				step: INT;
			END_VAR
			// Loaded one above the smallest value at step 0, CD rises at steps
			// 1 and 3: the first edge reaches it, the second leaves it there.
			s(CD := step = 1 OR step = 3, LD := step = 0, PV := SINT#-127);
			i(CD := step = 1 OR step = 3, LD := step = 0, PV := INT#-32767);
			d(CD := step = 1 OR step = 3, LD := step = 0, PV := DINT#-2147483647);
			l(CD := step = 1 OR step = 3, LD := step = 0, PV := LINT#-9223372036854775807);
			ud(CD := step = 1 OR step = 3, LD := step = 0, PV := UDINT#1);
			ul(CD := step = 1 OR step = 3, LD := step = 0, PV := ULINT#1);
			IF step = 3 THEN
				ASSERT(s.CV = SINT#-128, 'CTD_SINT stops at the smallest SINT');
				ASSERT(i.CV = INT#-32768, 'CTD_INT stops at the smallest INT');
				ASSERT(d.CV = DINT#-2147483648, 'CTD_DINT stops at the smallest DINT');
				ASSERT(l.CV = LINT#-9223372036854775808, 'CTD_LINT stops at the smallest LINT');
				ASSERT(ud.CV = UDINT#0, 'CTD_UDINT stops at 0');
				ASSERT(ul.CV = ULINT#0, 'CTD_ULINT stops at 0');
				DONE := TRUE;
			END_IF;
			step := step + 1;
		END_FUNCTION_BLOCK

		{test}
		FUNCTION_BLOCK test_ctud_stops_at_the_largest_of_its_type
			VAR_OUTPUT
				DONE: BOOL;
			END_VAR
			VAR
				s: CTUD_SINT;
				i: CTUD_INT;
				d: CTUD_DINT;
				l: CTUD_LINT;
				ud: CTUD_UDINT;
				ul: CTUD_ULINT;
				step: INT;
			END_VAR
			// Loaded one below the largest value at step 0, CU rises at steps
			// 1 and 3: the first edge reaches it, the second leaves it there.
			s(CU := step = 1 OR step = 3, CD := FALSE, R := FALSE, LD := step = 0, PV := SINT#126);
			i(CU := step = 1 OR step = 3, CD := FALSE, R := FALSE, LD := step = 0, PV := INT#32766);
			d(CU := step = 1 OR step = 3, CD := FALSE, R := FALSE, LD := step = 0, PV := DINT#2147483646);
			l(CU := step = 1 OR step = 3, CD := FALSE, R := FALSE, LD := step = 0, PV := LINT#9223372036854775806);
			ud(CU := step = 1 OR step = 3, CD := FALSE, R := FALSE, LD := step = 0, PV := UDINT#4294967294);
			ul(CU := step = 1 OR step = 3, CD := FALSE, R := FALSE, LD := step = 0, PV := ULINT#18446744073709551614);
			IF step = 3 THEN
				ASSERT(s.CV = SINT#127, 'CTUD_SINT stops at the largest SINT');
				ASSERT(i.CV = INT#32767, 'CTUD_INT stops at the largest INT');
				ASSERT(d.CV = DINT#2147483647, 'CTUD_DINT stops at the largest DINT');
				ASSERT(l.CV = LINT#9223372036854775807, 'CTUD_LINT stops at the largest LINT');
				ASSERT(ud.CV = UDINT#4294967295, 'CTUD_UDINT stops at the largest UDINT');
				ASSERT(ul.CV = ULINT#18446744073709551615, 'CTUD_ULINT stops at the largest ULINT');
				DONE := TRUE;
			END_IF;
			step := step + 1;
		END_FUNCTION_BLOCK

		{test}
		FUNCTION_BLOCK test_ctud_stops_at_the_smallest_of_its_type
			VAR_OUTPUT
				DONE: BOOL;
			END_VAR
			VAR
				s: CTUD_SINT;
				i: CTUD_INT;
				d: CTUD_DINT;
				l: CTUD_LINT;
				ud: CTUD_UDINT;
				ul: CTUD_ULINT;
				step: INT;
			END_VAR
			// Loaded one above the smallest value at step 0, CD rises at steps
			// 1 and 3: the first edge reaches it, the second leaves it there.
			s(CU := FALSE, CD := step = 1 OR step = 3, R := FALSE, LD := step = 0, PV := SINT#-127);
			i(CU := FALSE, CD := step = 1 OR step = 3, R := FALSE, LD := step = 0, PV := INT#-32767);
			d(CU := FALSE, CD := step = 1 OR step = 3, R := FALSE, LD := step = 0, PV := DINT#-2147483647);
			l(CU := FALSE, CD := step = 1 OR step = 3, R := FALSE, LD := step = 0, PV := LINT#-9223372036854775807);
			ud(CU := FALSE, CD := step = 1 OR step = 3, R := FALSE, LD := step = 0, PV := UDINT#1);
			ul(CU := FALSE, CD := step = 1 OR step = 3, R := FALSE, LD := step = 0, PV := ULINT#1);
			IF step = 3 THEN
				ASSERT(s.CV = SINT#-128, 'CTUD_SINT stops at the smallest SINT');
				ASSERT(i.CV = INT#-32768, 'CTUD_INT stops at the smallest INT');
				ASSERT(d.CV = DINT#-2147483648, 'CTUD_DINT stops at the smallest DINT');
				ASSERT(l.CV = LINT#-9223372036854775808, 'CTUD_LINT stops at the smallest LINT');
				ASSERT(ud.CV = UDINT#0, 'CTUD_UDINT stops at 0');
				ASSERT(ul.CV = ULINT#0, 'CTUD_ULINT stops at 0');
				DONE := TRUE;
			END_IF;
			step := step + 1;
		END_FUNCTION_BLOCK

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

		// --- width variants: each FB is independent compiled code, and apart
		// from the type limits the tests above count in INT ---

		{test}
		FUNCTION_BLOCK test_ctu_sint_reaches_preset
			VAR_OUTPUT
				DONE: BOOL;
			END_VAR
			VAR
				counter: CTU_SINT;
				step: INT;
			END_VAR
			// CU rises at steps 0 and 2.
			counter(CU := step <> 1, R := FALSE, PV := SINT#2);
			IF step = 2 THEN
				ASSERT(counter.CV = SINT#2, 'SINT counter reaches PV');
				ASSERT(counter.Q, 'SINT Q at PV');
				DONE := TRUE;
			END_IF;
			step := step + 1;
		END_FUNCTION_BLOCK

		{test}
		FUNCTION_BLOCK test_ctd_dint_counts_down
			VAR_OUTPUT
				DONE: BOOL;
			END_VAR
			VAR
				counter: CTD_DINT;
				step: INT;
			END_VAR
			// LD at step 0, CD rises at step 1.
			counter(CD := step = 1, LD := step = 0, PV := DINT#100000);
			IF step = 1 THEN
				ASSERT(counter.CV = DINT#99999, 'DINT counter decrements');
				DONE := TRUE;
			END_IF;
			step := step + 1;
		END_FUNCTION_BLOCK

		{test}
		FUNCTION_BLOCK test_ctd_lint_i64_lane
			VAR_OUTPUT
				DONE: BOOL;
			END_VAR
			VAR
				counter: CTD_LINT;
				step: INT;
			END_VAR
			// LD at step 0, CD rises at step 1.
			counter(CD := step = 1, LD := step = 0, PV := LINT#5000000000);
			IF step = 0 THEN
				ASSERT(counter.CV = LINT#5000000000, 'i64-lane load beyond i32 range');
			ELSE
				ASSERT(counter.CV = LINT#4999999999, 'i64-lane decrement');
				DONE := TRUE;
			END_IF;
			step := step + 1;
		END_FUNCTION_BLOCK

		{test}
		FUNCTION_BLOCK test_ctud_lint_up_down
			VAR_OUTPUT
				DONE: BOOL;
			END_VAR
			VAR
				counter: CTUD_LINT;
				step: INT;
			END_VAR
			// CU rises at step 0, CD at step 1.
			counter(CU := step = 0, CD := step = 1, R := FALSE, LD := FALSE, PV := LINT#3);
			IF step = 0 THEN
				ASSERT(counter.CV = LINT#1, 'LINT CTUD counts up');
			ELSE
				ASSERT(counter.CV = LINT#0, 'LINT CTUD counts down');
				DONE := TRUE;
			END_IF;
			step := step + 1;
		END_FUNCTION_BLOCK

		// -- standard base names ------------------------------------

		{test}
		FUNCTION_BLOCK test_plain_ctu_counts_to_pv
			VAR_OUTPUT
				DONE: BOOL;
			END_VAR
			VAR
				c: CTU;
				step: INT;
			END_VAR
			// CU rises at steps 0 and 2.
			c(CU := step <> 1, R := FALSE, PV := 2);
			IF step = 2 THEN
				ASSERT_EQ(c.CV, 2);
				ASSERT(c.Q, 'CTU Q at PV');
				DONE := TRUE;
			END_IF;
			step := step + 1;
		END_FUNCTION_BLOCK

		{test}
		FUNCTION_BLOCK test_plain_ctd_counts_down
			VAR_OUTPUT
				DONE: BOOL;
			END_VAR
			VAR
				c: CTD;
				step: INT;
			END_VAR
			// LD at step 0, CD rises at steps 1 and 3.
			c(CD := step = 1 OR step = 3, LD := step = 0, PV := 2);
			IF step = 0 THEN
				ASSERT_EQ(c.CV, 2);
			ELSIF step = 3 THEN
				ASSERT_EQ(c.CV, 0);
				ASSERT(c.Q, 'CTD Q at zero');
				DONE := TRUE;
			END_IF;
			step := step + 1;
		END_FUNCTION_BLOCK

		{test}
		FUNCTION_BLOCK test_plain_ctud_both_directions
			VAR_OUTPUT
				DONE: BOOL;
			END_VAR
			VAR
				c: CTUD;
				step: INT;
			END_VAR
			// CU rises at steps 0 and 2, CD at step 3.
			c(CU := step = 0 OR step = 2, CD := step = 3, R := FALSE, LD := FALSE, PV := 3);
			IF step = 2 THEN
				ASSERT_EQ(c.CV, 2);
			ELSIF step = 3 THEN
				ASSERT_EQ(c.CV, 1);
				ASSERT(NOT c.QU);
				ASSERT(NOT c.QD);
				DONE := TRUE;
			END_IF;
			step := step + 1;
		END_FUNCTION_BLOCK

		// -- unsigned counters: a preset past the signed range reads as a
		// negative number in a signed comparison, and each check below fails
		// with one ---

		{test}
		FUNCTION test_ctu_udint_counts
			VAR
				c: CTU_UDINT;
			END_VAR
			c(CU := TRUE, R := FALSE, PV := 1);
			ASSERT(c.Q, 'unsigned counter reaches PV');
		END_FUNCTION

		// From 0, where a signed comparison against an all-ones largest value
		// reads `CV < -1`, and the counter never counts.
		{test}
		FUNCTION test_unsigned_counters_count_up_from_0
			VAR
				a: CTU_ULINT;
				b: CTUD_UDINT;
				c: CTUD_ULINT;
			END_VAR
			a(CU := TRUE, R := FALSE, PV := 1);
			b(CU := TRUE, CD := FALSE, R := FALSE, LD := FALSE, PV := 1);
			c(CU := TRUE, CD := FALSE, R := FALSE, LD := FALSE, PV := 1);
			ASSERT(a.CV = 1, 'CTU_ULINT counts up from 0');
			ASSERT(b.CV = 1, 'CTUD_UDINT counts up from 0');
			ASSERT(c.CV = 1, 'CTUD_ULINT counts up from 0');
		END_FUNCTION

		{test}
		FUNCTION test_ctu_udint_preset_past_2_31
			VAR
				c: CTU_UDINT;
			END_VAR
			c(CU := FALSE, R := FALSE, PV := UDINT#3000000000);
			ASSERT(NOT c.Q, 'CV 0 is below PV 3000000000');
		END_FUNCTION

		{test}
		FUNCTION test_ctu_ulint_preset_past_2_63
			VAR
				c: CTU_ULINT;
			END_VAR
			c(CU := FALSE, R := FALSE, PV := ULINT#10000000000000000000);
			ASSERT(NOT c.Q, 'CV 0 is below PV 10000000000000000000');
		END_FUNCTION

		{test}
		FUNCTION_BLOCK test_ctd_ulint_loads_and_counts
			VAR_OUTPUT
				DONE: BOOL;
			END_VAR
			VAR
				c: CTD_ULINT;
				step: INT;
			END_VAR
			// LD at step 0, CD rises at step 1.
			c(CD := step = 1, LD := step = 0, PV := 1);
			IF step = 1 THEN
				ASSERT(c.Q, 'unsigned down-counter reaches zero');
				DONE := TRUE;
			END_IF;
			step := step + 1;
		END_FUNCTION_BLOCK

		{test}
		FUNCTION_BLOCK test_ctd_udint_preset_past_2_31
			VAR_OUTPUT
				DONE: BOOL;
			END_VAR
			VAR
				c: CTD_UDINT;
				step: INT;
			END_VAR
			// LD at step 0, CD rises at step 1.
			c(CD := step = 1, LD := step = 0, PV := UDINT#3000000000);
			IF step = 0 THEN
				ASSERT(NOT c.Q, 'CV 3000000000 is above 0');
			ELSE
				ASSERT(c.CV = UDINT#2999999999, 'CV 3000000000 counts down');
				DONE := TRUE;
			END_IF;
			step := step + 1;
		END_FUNCTION_BLOCK

		{test}
		FUNCTION_BLOCK test_ctd_ulint_preset_past_2_63
			VAR_OUTPUT
				DONE: BOOL;
			END_VAR
			VAR
				c: CTD_ULINT;
				step: INT;
			END_VAR
			// LD at step 0, CD rises at step 1.
			c(CD := step = 1, LD := step = 0, PV := ULINT#10000000000000000000);
			IF step = 0 THEN
				ASSERT(NOT c.Q, 'CV 10000000000000000000 is above 0');
			ELSE
				ASSERT(c.CV = ULINT#9999999999999999999, 'CV 10000000000000000000 counts down');
				DONE := TRUE;
			END_IF;
			step := step + 1;
		END_FUNCTION_BLOCK

		{test}
		FUNCTION_BLOCK test_ctud_udint_preset_past_2_31
			VAR_OUTPUT
				DONE: BOOL;
			END_VAR
			VAR
				c: CTUD_UDINT;
				step: INT;
			END_VAR
			// LD at step 1, CD rises at step 2.
			c(CU := FALSE, CD := step = 2, R := FALSE, LD := step = 1, PV := UDINT#3000000000);
			IF step = 0 THEN
				ASSERT(NOT c.QU AND c.QD, 'CV 0 is below PV 3000000000');
			ELSIF step = 1 THEN
				ASSERT(c.QU AND NOT c.QD, 'CV 3000000000 is at PV and above 0');
			ELSE
				ASSERT(c.CV = UDINT#2999999999 AND NOT c.QU, 'CV 3000000000 counts down');
				DONE := TRUE;
			END_IF;
			step := step + 1;
		END_FUNCTION_BLOCK

		{test}
		FUNCTION_BLOCK test_ctud_ulint_preset_past_2_63
			VAR_OUTPUT
				DONE: BOOL;
			END_VAR
			VAR
				c: CTUD_ULINT;
				step: INT;
			END_VAR
			// LD at step 1, CD rises at step 2.
			c(CU := FALSE, CD := step = 2, R := FALSE, LD := step = 1, PV := ULINT#10000000000000000000);
			IF step = 0 THEN
				ASSERT(NOT c.QU AND c.QD, 'CV 0 is below PV 10000000000000000000');
			ELSIF step = 1 THEN
				ASSERT(c.QU AND NOT c.QD, 'CV 10000000000000000000 is at PV and above 0');
			ELSE
				ASSERT(c.CV = ULINT#9999999999999999999 AND NOT c.QU, 'CV 10000000000000000000 counts down');
				DONE := TRUE;
			END_IF;
			step := step + 1;
		END_FUNCTION_BLOCK
	END_NAMESPACE
END_NAMESPACE
