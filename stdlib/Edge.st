/*
Edge detectors.

Both remember the `CLK` of the previous call and compare it with this one.
*/
NAMESPACE Std.Edge

	/* Detects a rising edge.

	   `Q` is `TRUE` for the one call where `CLK` went from `FALSE` to `TRUE`.
	   The first call counts as one when `CLK` is already `TRUE`.

	   # Examples

	   ```iecst
	   VAR
	       trigger: R_TRIG;
	   END_VAR

	   trigger(CLK := my_input);
	   IF trigger.Q THEN
	       // my_input has just turned on
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

	/* Detects a falling edge.

	   `Q` is `TRUE` for the one call where `CLK` went from `TRUE` to `FALSE`.
	   The first call counts as one when `CLK` is `FALSE`.

	   # Examples

	   ```iecst
	   VAR
	       trigger: F_TRIG;
	   END_VAR

	   trigger(CLK := my_input);
	   IF trigger.Q THEN
	       // my_input has just turned off
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

	NAMESPACE Test
		USING Std.Unit
		USING Std.Edge

		{test}
		FUNCTION test_r_trig_rising_edge
			VAR
				trig: R_TRIG;
			END_VAR
			trig(CLK := FALSE);
			ASSERT(NOT trig.Q);
			trig(CLK := TRUE);
			ASSERT(trig.Q);
		END_FUNCTION

		{test}
		FUNCTION test_r_trig_init_pulse_when_clk_starts_high
			VAR
				trig: R_TRIG;
			END_VAR
			trig(CLK := TRUE);
			ASSERT(trig.Q, 'first call: R_TRIG Q should be TRUE (IEC Table 35 initial rising edge)');
			trig(CLK := TRUE);
			ASSERT(NOT trig.Q, 'second call: R_TRIG Q should be FALSE (CLK held high, no new edge)');
		END_FUNCTION

		{test}
		FUNCTION test_f_trig_falling_edge
			VAR
				trig: F_TRIG;
			END_VAR
			trig(CLK := TRUE);
			ASSERT(NOT trig.Q);
			trig(CLK := FALSE);
			ASSERT(trig.Q);
		END_FUNCTION

		{test}
		FUNCTION test_f_trig_init_pulse_when_clk_starts_low
			VAR
				trig: F_TRIG;
			END_VAR
			trig(CLK := FALSE);
			ASSERT(trig.Q, 'first call: F_TRIG Q should be TRUE (IEC Table 35 initial falling edge)');
			trig(CLK := FALSE);
			ASSERT(NOT trig.Q, 'second call: F_TRIG Q should be FALSE (CLK held low, no new edge)');
		END_FUNCTION

		// --- re-trigger: a one-shot mutant (fires once per lifetime) passes
		// every test above ---

		{test}
		FUNCTION test_r_trig_retriggers
			VAR
				trig: R_TRIG;
			END_VAR
			trig(CLK := FALSE);
			trig(CLK := TRUE);
			ASSERT(trig.Q, 'first rising edge');
			trig(CLK := TRUE);
			ASSERT(NOT trig.Q, 'Q pulses exactly one call');
			trig(CLK := FALSE);
			ASSERT(NOT trig.Q, 'falling edge is not a rising edge');
			trig(CLK := TRUE);
			ASSERT(trig.Q, 'second rising edge must fire again');
		END_FUNCTION

		{test}
		FUNCTION test_f_trig_retriggers
			VAR
				trig: F_TRIG;
			END_VAR
			trig(CLK := TRUE);
			trig(CLK := FALSE);
			ASSERT(trig.Q, 'first falling edge');
			trig(CLK := FALSE);
			ASSERT(NOT trig.Q, 'Q pulses exactly one call');
			trig(CLK := TRUE);
			ASSERT(NOT trig.Q, 'rising edge is not a falling edge');
			trig(CLK := FALSE);
			ASSERT(trig.Q, 'second falling edge must fire again');
		END_FUNCTION
	END_NAMESPACE
END_NAMESPACE
