/*
Timers.

Each takes `IN` and a duration `PT`, and gives `Q` and the elapsed time
`ET`, which stops at `PT`. They read the clock when they are called, so a
timer is as precise as the scan that calls it.

`TP`, `TON` and `TOF` measure in `TIME`. The `_LTIME` blocks are the same
timers in `LTIME`.
*/
NAMESPACE Std.Timers
	USING Std.Convert;

	/* Returns the host's monotonic clock, which never goes back.

	   This is the `now` of the WASI
	   [monotonic clock](https://github.com/WebAssembly/WASI/blob/main/proposals/clocks/README.md),
	   and the one import a module that uses a timer has.
	*/
	{extern 'wasi:clocks/monotonic-clock@0.2.6' 'now'}
	FUNCTION PRIVATE MONOTONIC_LTIME: LTIME
	END_FUNCTION

	/* Returns the monotonic clock as a `TIME`. Only the difference between two readings means something. */
	FUNCTION PLC_TIME: TIME
		PLC_TIME := LTIME_TO_TIME(MONOTONIC_LTIME());
	END_FUNCTION

	/* Returns the monotonic clock as an `LTIME`. Only the difference between two readings means something. */
	FUNCTION PLC_LTIME: LTIME
		PLC_LTIME := MONOTONIC_LTIME();
	END_FUNCTION

	/* A pulse of a fixed length.

	   A rising edge of `IN` sets `Q` for `PT`, whatever `IN` does meanwhile. A new
	   pulse starts on the next rising edge after this one has ended.

	   # Examples

	   ```iecst
	   VAR
	       pulse: TP_TIME;
	   END_VAR

	   pulse(IN := trigger, PT := T#5s);
	   valve := pulse.Q;  // open for five seconds
	   ```
	*/
	FUNCTION_BLOCK TP_TIME
		VAR_INPUT
			IN: BOOL;
			PT: TIME;
		END_VAR
		VAR_OUTPUT
			Q: BOOL := FALSE;
			ET: TIME
		END_VAR
		VAR
			STATE: SINT := 0;
			PREV_IN: BOOL := FALSE;
			CURRENT_TIME, START_TIME: TIME;
		END_VAR

		CURRENT_TIME := PLC_TIME();
		IF ((STATE = 0) AND NOT (PREV_IN) AND IN) THEN
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
		IF ((STATE = 2) AND NOT (IN)) THEN
			ET := T#0s;
			STATE := 0;
		END_IF;
		PREV_IN := IN;
	END_FUNCTION_BLOCK

	TYPE TP: TP_TIME END_TYPE

	/* [Std.Timers.TP_TIME] in `LTIME`. */
	FUNCTION_BLOCK TP_LTIME
		VAR_INPUT
			IN: BOOL;
			PT: LTIME;
		END_VAR
		VAR_OUTPUT
			Q: BOOL := FALSE;
			ET: LTIME
		END_VAR
		VAR
			STATE: SINT := 0;
			PREV_IN: BOOL := FALSE;
			CURRENT_TIME, START_TIME: LTIME;
		END_VAR

		CURRENT_TIME := PLC_LTIME();
		IF ((STATE = 0) AND NOT (PREV_IN) AND IN) THEN
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
		IF ((STATE = 2) AND NOT (IN)) THEN
			ET := LTIME#0s;
			STATE := 0;
		END_IF;
		PREV_IN := IN;
	END_FUNCTION_BLOCK

	/* Delays turning on.

	   `Q` becomes `TRUE` once `IN` has been `TRUE` for `PT`, and `FALSE` as soon
	   as `IN` is. An `IN` that drops before `PT` starts the delay over.

	   # Examples

	   ```iecst
	   VAR
	       delay: TON_TIME;
	   END_VAR

	   delay(IN := start_signal, PT := T#3s);
	   IF delay.Q THEN
	       // start_signal has held for three seconds
	   END_IF;
	   ```
	*/
	FUNCTION_BLOCK TON_TIME
		VAR_INPUT
			IN: BOOL;
			PT: TIME;
		END_VAR
		VAR_OUTPUT
			Q: BOOL := FALSE;
			ET: TIME;
		END_VAR
		VAR
			STATE: SINT := 0;
			PREV_IN: BOOL := FALSE;
			CURRENT_TIME, START_TIME: TIME;
		END_VAR

		CURRENT_TIME := PLC_TIME();
		IF ((STATE = 0) AND NOT (PREV_IN) AND IN) THEN
			STATE := 1;
			Q := FALSE;
			START_TIME := CURRENT_TIME;
		ELSE
			IF (NOT (IN)) THEN
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

	TYPE TON: TON_TIME END_TYPE

	/* [Std.Timers.TON_TIME] in `LTIME`. */
	FUNCTION_BLOCK TON_LTIME
		VAR_INPUT
			IN: BOOL;
			PT: LTIME;
		END_VAR
		VAR_OUTPUT
			Q: BOOL := FALSE;
			ET: LTIME;
		END_VAR
		VAR
			STATE: SINT := 0;
			PREV_IN: BOOL := FALSE;
			CURRENT_TIME, START_TIME: LTIME;
		END_VAR

		CURRENT_TIME := PLC_LTIME();
		IF ((STATE = 0) AND NOT (PREV_IN) AND IN) THEN
			STATE := 1;
			Q := FALSE;
			START_TIME := CURRENT_TIME;
		ELSE
			IF (NOT (IN)) THEN
				ET := LTIME#0s;
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

	/* Delays turning off.

	   `Q` is `TRUE` while `IN` is, and stays `TRUE` for `PT` after `IN` drops. An
	   `IN` that comes back before `PT` starts the delay over.

	   # Examples

	   ```iecst
	   VAR
	       off_delay: TOF_TIME;
	   END_VAR

	   off_delay(IN := motor_running, PT := T#10s);
	   cooling_fan := off_delay.Q;  // runs ten seconds after the motor stops
	   ```
	*/
	FUNCTION_BLOCK TOF_TIME
		VAR_INPUT
			IN: BOOL;
			PT: TIME;
		END_VAR
		VAR_OUTPUT
			Q: BOOL := FALSE;
			ET: TIME;
		END_VAR
		VAR
			STATE: SINT := 0;
			PREV_IN: BOOL := FALSE;
			CURRENT_TIME, START_TIME: TIME;
		END_VAR

		CURRENT_TIME := PLC_TIME();
		IF ((STATE = 0) AND PREV_IN AND NOT (IN)) THEN
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

	TYPE TOF: TOF_TIME END_TYPE

	/* [Std.Timers.TOF_TIME] in `LTIME`. */
	FUNCTION_BLOCK TOF_LTIME
		VAR_INPUT
			IN: BOOL;
			PT: LTIME;
		END_VAR
		VAR_OUTPUT
			Q: BOOL := FALSE;
			ET: LTIME;
		END_VAR
		VAR
			STATE: SINT := 0;
			PREV_IN: BOOL := FALSE;
			CURRENT_TIME, START_TIME: LTIME;
		END_VAR

		CURRENT_TIME := PLC_LTIME();
		IF ((STATE = 0) AND PREV_IN AND NOT (IN)) THEN
			STATE := 1;
			START_TIME := CURRENT_TIME;
		ELSE
			IF (IN) THEN
				ET := LTIME#0s;
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
			VAR
				timer: TON_TIME;
			END_VAR
			timer(IN := TRUE, PT := T#1ms);
			WHILE NOT timer.Q DO
				timer(IN := TRUE, PT := T#1ms);
			END_WHILE;
			ASSERT(timer.Q, 'TON should have expired');
		END_FUNCTION

		{test}
		FUNCTION test_ton_not_started
			VAR
				timer: TON_TIME;
			END_VAR
			timer(IN := FALSE, PT := T#1ms);
			ASSERT(NOT timer.Q, 'TON Q should be FALSE when IN is FALSE');
		END_FUNCTION

		{test}
		FUNCTION test_ton_reset
			VAR
				timer: TON_TIME;
			END_VAR
			timer(IN := TRUE, PT := T#100ms);
			ASSERT(NOT timer.Q, 'TON Q should be FALSE immediately');
			timer(IN := FALSE, PT := T#100ms);
			ASSERT(NOT timer.Q, 'TON Q should be FALSE after reset');
			ASSERT(timer.ET = T#0s, 'TON ET should be 0 after reset');
		END_FUNCTION

		// -- TOF (Off-Delay Timer) -----------------------------------

		{test}
		FUNCTION test_tof_stays_true_after_falling_edge
			VAR
				timer: TOF_TIME;
			END_VAR
			// Rising edge: Q becomes TRUE immediately
			timer(IN := TRUE, PT := T#100ms);
			ASSERT(timer.Q, 'TOF Q should be TRUE when IN is TRUE');
			// Falling edge: Q stays TRUE during delay
			timer(IN := FALSE, PT := T#100ms);
			ASSERT(timer.Q, 'TOF Q should stay TRUE during off-delay');
		END_FUNCTION

		{test}
		FUNCTION test_tof_expires
			VAR
				timer: TOF_TIME;
			END_VAR
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
			VAR
				timer: TOF_TIME;
			END_VAR
			timer(IN := TRUE, PT := T#1ms);
			timer(IN := FALSE, PT := T#1ms);
			// Re-assert IN before timer expires
			timer(IN := TRUE, PT := T#1ms);
			ASSERT(timer.Q, 'TOF Q should be TRUE after re-assertion');
		END_FUNCTION

		// -- TP (Pulse Timer) ----------------------------------------

		{test}
		FUNCTION test_tp_pulse_starts
			VAR
				timer: TP_TIME;
			END_VAR
			timer(IN := TRUE, PT := T#100ms);
			ASSERT(timer.Q, 'TP Q should be TRUE on rising edge');
		END_FUNCTION

		{test}
		FUNCTION test_tp_pulse_expires
			VAR
				timer: TP_TIME;
			END_VAR
			timer(IN := TRUE, PT := T#1ms);
			WHILE timer.Q DO
				timer(IN := TRUE, PT := T#1ms);
			END_WHILE;
			ASSERT(NOT timer.Q, 'TP Q should be FALSE after pulse expired');
		END_FUNCTION

		{test}
		FUNCTION test_tp_ignores_input_during_pulse
			VAR
				timer: TP_TIME;
			END_VAR
			timer(IN := TRUE, PT := T#100ms);
			ASSERT(timer.Q, 'TP Q should be TRUE');
			// Dropping IN during pulse doesn't affect Q
			timer(IN := FALSE, PT := T#100ms);
			ASSERT(timer.Q, 'TP Q should stay TRUE during pulse even if IN drops');
		END_FUNCTION

		// --- TON must NOT fire before PT: an instant-expiry TON (PT ignored
		// or inverted compare) passes every spin-loop test above ---

		{test}
		FUNCTION test_ton_stays_false_before_expiry
			VAR
				timer: TON_TIME;
				i: INT;
			END_VAR
			timer(IN := TRUE, PT := T#1h);
			FOR i := 1 TO 10 DO
				timer(IN := TRUE, PT := T#1h);
				ASSERT(NOT timer.Q, 'TON must not fire long before PT elapses');
			END_FOR;
			ASSERT(timer.ET < T#1h, 'ET tracks elapsed time below PT');
		END_FUNCTION

		{test}
		FUNCTION test_ton_et_clamps_at_pt
			VAR
				timer: TON_TIME;
			END_VAR
			timer(IN := TRUE, PT := T#1ms);
			WHILE NOT timer.Q DO
				timer(IN := TRUE, PT := T#1ms);
			END_WHILE;
			ASSERT(timer.ET = T#1ms, 'ET is exactly PT after expiry');
		END_FUNCTION

		// --- TP terminal state: no auto-refire while IN is held, and a fresh
		// rising edge after release starts a new pulse ---

		{test}
		FUNCTION test_tp_post_pulse_retrigger
			VAR
				timer: TP_TIME;
			END_VAR
			timer(IN := TRUE, PT := T#1ms);
			WHILE timer.Q DO
				timer(IN := TRUE, PT := T#1ms);
			END_WHILE;
			ASSERT(timer.ET = T#1ms, 'ET clamps at PT when the pulse ends');
			timer(IN := TRUE, PT := T#1ms);
			ASSERT(NOT timer.Q, 'held-high IN must not refire the pulse');
			timer(IN := FALSE, PT := T#1ms);
			timer(IN := TRUE, PT := T#1ms);
			ASSERT(timer.Q, 'a new rising edge after release starts a new pulse');
		END_FUNCTION

		// --- TOF: re-asserting IN cancels the pending expiry; the NEXT
		// falling edge must get a fresh deadline, not the stale one ---

		{test}
		FUNCTION test_tof_fresh_deadline_after_reassert
			VAR
				timer: TOF_TIME;
				t0: TIME;
			END_VAR
			timer(IN := TRUE, PT := T#50ms);
			timer(IN := FALSE, PT := T#50ms);
			// Let the FIRST off-delay deadline pass while IN is re-asserted.
			t0 := PLC_TIME();
			WHILE PLC_TIME() - t0 < T#60ms DO
				timer(IN := TRUE, PT := T#50ms);
			END_WHILE;
			// New falling edge: deadline must be now+PT, not the stale t0+PT.
			timer(IN := FALSE, PT := T#50ms);
			ASSERT(timer.Q, 'TOF must use a fresh deadline for a new falling edge');
		END_FUNCTION

		{test}
		FUNCTION test_tof_et_clamps_at_pt
			VAR
				timer: TOF_TIME;
			END_VAR
			timer(IN := TRUE, PT := T#1ms);
			timer(IN := FALSE, PT := T#1ms);
			WHILE timer.Q DO
				timer(IN := FALSE, PT := T#1ms);
			END_WHILE;
			ASSERT(timer.ET = T#1ms, 'ET is exactly PT after the off-delay expires');
		END_FUNCTION

		// --- PT = 0 (pins current behavior: the deadline check runs on the
		// call AFTER the starting edge, so a zero preset needs two calls) ---

		{test}
		FUNCTION test_pt_zero_pins_current_behavior
			VAR
				ton_timer: TON_TIME;
				tp_timer: TP_TIME;
			END_VAR
			ton_timer(IN := TRUE, PT := T#0s);
			ASSERT(NOT ton_timer.Q, 'TON PT=0: edge call does not evaluate the deadline yet');
			ton_timer(IN := TRUE, PT := T#0s);
			ASSERT(ton_timer.Q, 'TON PT=0: fires on the next call');
			tp_timer(IN := TRUE, PT := T#0s);
			ASSERT(tp_timer.Q, 'TP PT=0: pulse starts on the edge call');
			tp_timer(IN := TRUE, PT := T#0s);
			ASSERT(NOT tp_timer.Q, 'TP PT=0: pulse over on the next call');
		END_FUNCTION

		// -- the LTIME variants run the same machine on the ns clock --

		{test}
		FUNCTION test_ton_ltime_expires
			VAR
				timer: TON_LTIME;
			END_VAR
			timer(IN := TRUE, PT := LTIME#1ms);
			WHILE NOT timer.Q DO
				timer(IN := TRUE, PT := LTIME#1ms);
			END_WHILE;
			ASSERT(timer.ET = LTIME#1ms, 'TON_LTIME ET is exactly PT after expiry');
		END_FUNCTION

		{test}
		FUNCTION test_tof_ltime_expires
			VAR
				timer: TOF_LTIME;
			END_VAR
			timer(IN := TRUE, PT := LTIME#1ms);
			ASSERT(timer.Q, 'TOF_LTIME Q should be TRUE while IN is TRUE');
			timer(IN := FALSE, PT := LTIME#1ms);
			WHILE timer.Q DO
				timer(IN := FALSE, PT := LTIME#1ms);
			END_WHILE;
			ASSERT(timer.ET = LTIME#1ms, 'TOF_LTIME ET clamps at PT after expiry');
		END_FUNCTION

		{test}
		FUNCTION test_tp_ltime_pulse
			VAR
				timer: TP_LTIME;
			END_VAR
			timer(IN := TRUE, PT := LTIME#1ms);
			ASSERT(timer.Q, 'TP_LTIME Q should be TRUE on rising edge');
			WHILE timer.Q DO
				timer(IN := TRUE, PT := LTIME#1ms);
			END_WHILE;
			ASSERT(timer.ET = LTIME#1ms, 'TP_LTIME ET clamps at PT when the pulse ends');
		END_FUNCTION

		// -- the standard names are aliases; instances declared through
		// them must build, call, and expose the outputs --

		{test}
		FUNCTION test_standard_names_are_usable
			VAR
				pulse: TP;
				on_delay: TON;
				off_delay: TOF;
			END_VAR
			pulse(IN := TRUE, PT := T#100ms);
			ASSERT(pulse.Q, 'a TP-typed instance pulses on the rising edge');
			on_delay(IN := TRUE, PT := T#1ms);
			WHILE NOT on_delay.Q DO
				on_delay(IN := TRUE, PT := T#1ms);
			END_WHILE;
			ASSERT(on_delay.ET = T#1ms, 'a TON-typed instance expires at PT');
			off_delay(IN := TRUE, PT := T#100ms);
			off_delay(IN := FALSE, PT := T#100ms);
			ASSERT(off_delay.Q, 'a TOF-typed instance holds Q through the off-delay');
		END_FUNCTION

		// --- the clock itself ---

		{test}
		FUNCTION test_plc_time_monotonic
			VAR
				t1, t2: TIME;
			END_VAR
			t1 := PLC_TIME();
			t2 := PLC_TIME();
			ASSERT(t2 >= t1, 'PLC_TIME must be monotonic');
		END_FUNCTION

		{test}
		FUNCTION test_plc_ltime_monotonic
			VAR
				t1, t2: LTIME;
			END_VAR
			t1 := PLC_LTIME();
			t2 := PLC_LTIME();
			ASSERT(t2 >= t1, 'PLC_LTIME must be monotonic');
		END_FUNCTION
	END_NAMESPACE
END_NAMESPACE
