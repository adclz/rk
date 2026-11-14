/*
Timers, and the clocks they read.

Each takes `IN` and a duration `PT`, and gives `Q` and the elapsed time
`ET`, which stops at `PT`. They read the clock when they are called, so a
timer is as precise as the scan that calls it.

`TP`, `TON` and `TOF` measure in `TIME`. The `_LTIME` blocks are the same
timers in `LTIME`.

`PLC_TIME` and `PLC_LTIME` read the monotonic clock, for the time between
two readings. `WALL_LDT`, `WALL_DT`, `LOCAL_LDT` and `LOCAL_OFFSET` read
the wall clock and the host's zone. The wall clock goes back when the
host's clock is set: a duration is measured with `PLC_TIME`.

A timer decides on `CURRENT_TIME - START_TIME`, a difference, which wraps
with the clock where `START_TIME + PT` would not: the host picks where the
monotonic clock starts, and `PLC_TIME` cycles every 49.7 days in any case.

A timer's step and clock readings are `NON_RETAIN`: held by a retained
instance, it starts over after a warm start instead of measuring from a
reading of the previous run's clock. `Q` and `ET` follow the step on every
call, so the first call after the warm start ends what the step no longer
runs.
*/
NAMESPACE Std.Timers
	USING Std.Convert;

	/* Returns the host's monotonic clock, which never goes back.

	   This is the `now` of the WASI
	   [monotonic clock](https://github.com/WebAssembly/WASI/blob/main/proposals/clocks/README.md),
	   a count of nanoseconds, and the one import a module that uses a timer
	   has.
	*/
	FUNCTION PRIVATE MONOTONIC_LTIME: LTIME
		MONOTONIC_LTIME := LINT_TO_LTIME(ULINT_TO_LINT(Wasi.Clocks.MonotonicClock.NOW()));
	END_FUNCTION

	/* Returns the monotonic clock as a `TIME`. Only the difference between two readings means something. */
	FUNCTION PLC_TIME: TIME
		PLC_TIME := LTIME_TO_TIME(MONOTONIC_LTIME());
	END_FUNCTION

	/* Returns the monotonic clock as an `LTIME`. Only the difference between two readings means something. */
	FUNCTION PLC_LTIME: LTIME
		PLC_LTIME := MONOTONIC_LTIME();
	END_FUNCTION

	/* Returns the host's wall clock as an `LDT`, in UTC: the `now` of the
	   WASI [system clock](https://github.com/WebAssembly/WASI/blob/main/proposals/clocks/README.md),
	   the seconds since 1970-01-01, negative before, and the nanoseconds
	   after them. */
	FUNCTION WALL_LDT : LDT
		VAR
			seconds : LINT;
			nanoseconds : UDINT;
		END_VAR
		Wasi.Clocks.SystemClock.NOW(seconds => seconds, nanoseconds => nanoseconds);
		WALL_LDT := LINT_TO_LDT(seconds * LINT#1000000000 + nanoseconds);
	END_FUNCTION

	/* Returns the host's wall clock as a `DT`, in UTC. */
	FUNCTION WALL_DT : DT
		WALL_DT := LDT_TO_DT(WALL_LDT());
	END_FUNCTION

	/* Returns how far east of UTC the host's zone is at `IN`, daylight
	   saving included: `LT#1h` in Paris in winter, `LT#2h` in summer,
	   `LT#-5h` in New York in winter. Raises "the host has no time zone"
	   when the host has none.

	   The zone is WASI 0.3's `timezone`, which is still unstable
	   (`clocks-timezone`): a host has it only with that feature on, and
	   its signature can change in a 0.3 release. */
	{warn = 'reads the time zone, which WASI 0.3 marks unstable: a host has it only with its clocks-timezone feature on'}
	FUNCTION LOCAL_OFFSET : LTIME
		VAR_INPUT
			IN : LDT;
		END_VAR
		VAR
			seconds, nanoseconds : LINT;
			known : BOOL;
			offset : LINT;
		END_VAR
		// WASI takes the whole seconds, rounded down, and the nanoseconds after them.
		seconds := LDT_TO_LINT(IN) / LINT#1000000000;
		nanoseconds := LDT_TO_LINT(IN) MOD LINT#1000000000;
		IF nanoseconds < LINT#0 THEN
			seconds := seconds - LINT#1;
			nanoseconds := nanoseconds + LINT#1000000000;
		END_IF;
		// The `utc-offset` of the WASI time zone: an `option<s64>`, which
		// comes back as whether the host has a zone, then the offset.
		Wasi.Clocks.Timezone.UTC_OFFSET(
			when_seconds := seconds,
			when_nanoseconds := LINT_TO_UDINT(nanoseconds),
			is_some => known,
			value => offset);
		IF NOT known THEN
			__RAISE('the host has no time zone');
		END_IF;
		LOCAL_OFFSET := LINT_TO_LTIME(offset);
	END_FUNCTION

	/* Returns the host's wall clock as an `LDT` in the host's zone: the
	   instant `WALL_LDT` returns, shifted by `LOCAL_OFFSET`, in the same
	   encoding. What a clock on the wall shows. Raises "the host has no
	   time zone" when the host has none, as `LOCAL_OFFSET` does. */
	{warn = 'reads the time zone, which WASI 0.3 marks unstable: a host has it only with its clocks-timezone feature on'}
	FUNCTION LOCAL_LDT : LDT
		VAR
			utc : LDT;
		END_VAR
		utc := WALL_LDT();
		// The warning is LOCAL_LDT's own, at its callers.
		{allow 'warn-pragma'}
		LOCAL_LDT := LINT_TO_LDT(LDT_TO_LINT(utc) + LTIME_TO_LINT(LOCAL_OFFSET(IN := utc)));
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
	{must_call}
	{once}
	FUNCTION_BLOCK TP_TIME
		VAR_INPUT
			IN: BOOL;
			PT: TIME;
		END_VAR
		VAR_OUTPUT
			Q: BOOL := FALSE;
			ET: TIME
		END_VAR
		VAR NON_RETAIN
			STATE: SINT := 0;
			PREV_IN: BOOL := FALSE;
			CURRENT_TIME, START_TIME: TIME;
		END_VAR

		CURRENT_TIME := PLC_TIME();
		IF ((STATE = 0) AND NOT (PREV_IN) AND IN) THEN
			STATE := 1;
			START_TIME := CURRENT_TIME;
		ELSIF ((STATE = 1) AND (CURRENT_TIME - START_TIME >= PT)) THEN
			STATE := 2;
		END_IF;
		IF ((STATE = 2) AND NOT (IN)) THEN
			STATE := 0;
		END_IF;
		IF (STATE = 1) THEN
			ET := CURRENT_TIME - START_TIME;
		ELSIF (STATE = 2) THEN
			ET := PT;
		ELSE
			ET := T#0s;
		END_IF;
		Q := (STATE = 1);
		PREV_IN := IN;
	END_FUNCTION_BLOCK

	TYPE TP: TP_TIME END_TYPE

	/* [Std.Timers.TP_TIME] in `LTIME`. */
	{must_call}
	{once}
	FUNCTION_BLOCK TP_LTIME
		VAR_INPUT
			IN: BOOL;
			PT: LTIME;
		END_VAR
		VAR_OUTPUT
			Q: BOOL := FALSE;
			ET: LTIME
		END_VAR
		VAR NON_RETAIN
			STATE: SINT := 0;
			PREV_IN: BOOL := FALSE;
			CURRENT_TIME, START_TIME: LTIME;
		END_VAR

		CURRENT_TIME := PLC_LTIME();
		IF ((STATE = 0) AND NOT (PREV_IN) AND IN) THEN
			STATE := 1;
			START_TIME := CURRENT_TIME;
		ELSIF ((STATE = 1) AND (CURRENT_TIME - START_TIME >= PT)) THEN
			STATE := 2;
		END_IF;
		IF ((STATE = 2) AND NOT (IN)) THEN
			STATE := 0;
		END_IF;
		IF (STATE = 1) THEN
			ET := CURRENT_TIME - START_TIME;
		ELSIF (STATE = 2) THEN
			ET := PT;
		ELSE
			ET := LTIME#0s;
		END_IF;
		Q := (STATE = 1);
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
	{must_call}
	{once}
	FUNCTION_BLOCK TON_TIME
		VAR_INPUT
			IN: BOOL;
			PT: TIME;
		END_VAR
		VAR_OUTPUT
			Q: BOOL := FALSE;
			ET: TIME;
		END_VAR
		VAR NON_RETAIN
			STATE: SINT := 0;
			PREV_IN: BOOL := FALSE;
			CURRENT_TIME, START_TIME: TIME;
		END_VAR

		CURRENT_TIME := PLC_TIME();
		IF (NOT (IN)) THEN
			STATE := 0;
		ELSIF ((STATE = 0) AND NOT (PREV_IN)) THEN
			STATE := 1;
			START_TIME := CURRENT_TIME;
		ELSIF ((STATE = 1) AND (CURRENT_TIME - START_TIME >= PT)) THEN
			STATE := 2;
		END_IF;
		IF (STATE = 1) THEN
			ET := CURRENT_TIME - START_TIME;
		ELSIF (STATE = 2) THEN
			ET := PT;
		ELSE
			ET := T#0s;
		END_IF;
		Q := (STATE = 2);
		PREV_IN := IN;
	END_FUNCTION_BLOCK

	TYPE TON: TON_TIME END_TYPE

	/* [Std.Timers.TON_TIME] in `LTIME`. */
	{must_call}
	{once}
	FUNCTION_BLOCK TON_LTIME
		VAR_INPUT
			IN: BOOL;
			PT: LTIME;
		END_VAR
		VAR_OUTPUT
			Q: BOOL := FALSE;
			ET: LTIME;
		END_VAR
		VAR NON_RETAIN
			STATE: SINT := 0;
			PREV_IN: BOOL := FALSE;
			CURRENT_TIME, START_TIME: LTIME;
		END_VAR

		CURRENT_TIME := PLC_LTIME();
		IF (NOT (IN)) THEN
			STATE := 0;
		ELSIF ((STATE = 0) AND NOT (PREV_IN)) THEN
			STATE := 1;
			START_TIME := CURRENT_TIME;
		ELSIF ((STATE = 1) AND (CURRENT_TIME - START_TIME >= PT)) THEN
			STATE := 2;
		END_IF;
		IF (STATE = 1) THEN
			ET := CURRENT_TIME - START_TIME;
		ELSIF (STATE = 2) THEN
			ET := PT;
		ELSE
			ET := LTIME#0s;
		END_IF;
		Q := (STATE = 2);
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
	{must_call}
	{once}
	FUNCTION_BLOCK TOF_TIME
		VAR_INPUT
			IN: BOOL;
			PT: TIME;
		END_VAR
		VAR_OUTPUT
			Q: BOOL := FALSE;
			ET: TIME;
		END_VAR
		VAR NON_RETAIN
			STATE: SINT := 0;
			PREV_IN: BOOL := FALSE;
			CURRENT_TIME, START_TIME: TIME;
		END_VAR

		CURRENT_TIME := PLC_TIME();
		IF (IN) THEN
			STATE := 0;
		ELSIF ((STATE = 0) AND PREV_IN) THEN
			STATE := 1;
			START_TIME := CURRENT_TIME;
		ELSIF ((STATE = 1) AND (CURRENT_TIME - START_TIME >= PT)) THEN
			STATE := 2;
		END_IF;
		IF (STATE = 1) THEN
			ET := CURRENT_TIME - START_TIME;
		ELSIF (STATE = 2) THEN
			ET := PT;
		ELSE
			ET := T#0s;
		END_IF;
		Q := IN OR (STATE = 1);
		PREV_IN := IN;
	END_FUNCTION_BLOCK

	TYPE TOF: TOF_TIME END_TYPE

	/* [Std.Timers.TOF_TIME] in `LTIME`. */
	{must_call}
	{once}
	FUNCTION_BLOCK TOF_LTIME
		VAR_INPUT
			IN: BOOL;
			PT: LTIME;
		END_VAR
		VAR_OUTPUT
			Q: BOOL := FALSE;
			ET: LTIME;
		END_VAR
		VAR NON_RETAIN
			STATE: SINT := 0;
			PREV_IN: BOOL := FALSE;
			CURRENT_TIME, START_TIME: LTIME;
		END_VAR

		CURRENT_TIME := PLC_LTIME();
		IF (IN) THEN
			STATE := 0;
		ELSIF ((STATE = 0) AND PREV_IN) THEN
			STATE := 1;
			START_TIME := CURRENT_TIME;
		ELSIF ((STATE = 1) AND (CURRENT_TIME - START_TIME >= PT)) THEN
			STATE := 2;
		END_IF;
		IF (STATE = 1) THEN
			ET := CURRENT_TIME - START_TIME;
		ELSIF (STATE = 2) THEN
			ET := PT;
		ELSE
			ET := LTIME#0s;
		END_IF;
		Q := IN OR (STATE = 1);
		PREV_IN := IN;
	END_FUNCTION_BLOCK

	NAMESPACE Test
		USING Std.Unit;
		USING Std.Timers;

		// A test that waits for the clock is a block: it calls each timer once
		// per cycle, as a program does, and is DONE in the cycle where what it
		// waits for has happened.

		// -- TON (On-Delay Timer) ------------------------------------

		{test}
		FUNCTION_BLOCK test_ton_expires
			VAR_OUTPUT
				DONE: BOOL;
			END_VAR
			VAR
				timer: TON_TIME;
			END_VAR
			timer(IN := TRUE, PT := T#1ms);
			DONE := timer.Q;
		END_FUNCTION_BLOCK

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
		FUNCTION_BLOCK test_tof_expires
			VAR_OUTPUT
				DONE: BOOL;
			END_VAR
			VAR
				timer: TOF_TIME;
				step: INT;
			END_VAR
			// IN on the first cycle, off from the second.
			timer(IN := step = 0, PT := T#1ms);
			IF step = 0 THEN
				step := 1;
			ELSE
				DONE := NOT timer.Q;
			END_IF;
		END_FUNCTION_BLOCK

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
		FUNCTION_BLOCK test_tp_pulse_expires
			VAR_OUTPUT
				DONE: BOOL;
			END_VAR
			VAR
				timer: TP_TIME;
			END_VAR
			timer(IN := TRUE, PT := T#1ms);
			DONE := NOT timer.Q;
		END_FUNCTION_BLOCK

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
		// or inverted compare) passes every test above that waits for Q ---

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
		FUNCTION_BLOCK test_ton_et_clamps_at_pt
			VAR_OUTPUT
				DONE: BOOL;
			END_VAR
			VAR
				timer: TON_TIME;
				step: INT;
				t0: TIME;
			END_VAR
			timer(IN := TRUE, PT := T#1ms);
			IF step = 0 AND timer.Q THEN
				t0 := PLC_TIME();
				step := 1;
			END_IF;
			IF step = 1 THEN
				// IN held 2 ms past the deadline: ET stays at PT.
				ASSERT(timer.Q AND timer.ET = T#1ms, 'ET is exactly PT after expiry');
				DONE := PLC_TIME() - t0 >= T#2ms;
			END_IF;
		END_FUNCTION_BLOCK

		// --- TP terminal state: no auto-refire while IN is held, and a fresh
		// rising edge after release starts a new pulse ---

		{test}
		FUNCTION_BLOCK test_tp_post_pulse_retrigger
			VAR_OUTPUT
				DONE: BOOL;
			END_VAR
			VAR
				timer: TP_TIME;
				step: INT;
				t0: TIME;
			END_VAR
			// IN held, released for the one cycle of step 2, then held again.
			timer(IN := step <> 2, PT := T#1ms);
			IF step = 0 AND NOT timer.Q THEN
				// The pulse has ended.
				t0 := PLC_TIME();
				step := 1;
			END_IF;
			IF step = 1 THEN
				// IN held 2 ms past the pulse: ET stays at PT, and no new pulse.
				ASSERT(timer.ET = T#1ms, 'ET clamps at PT when the pulse ends');
				ASSERT(NOT timer.Q, 'held-high IN must not refire the pulse');
				IF PLC_TIME() - t0 >= T#2ms THEN
					step := 2;
				END_IF;
			ELSIF step = 2 THEN
				step := 3;
			ELSIF step = 3 THEN
				ASSERT(timer.Q, 'a new rising edge after release starts a new pulse');
				DONE := TRUE;
			END_IF;
		END_FUNCTION_BLOCK

		// --- TOF: re-asserting IN cancels the pending expiry; the NEXT
		// falling edge must get a fresh deadline, not the stale one ---

		{test}
		FUNCTION_BLOCK test_tof_fresh_deadline_after_reassert
			VAR_OUTPUT
				DONE: BOOL;
			END_VAR
			VAR
				timer: TOF_TIME;
				step: INT;
				t0, t1: TIME;
			END_VAR
			// IN rises, falls, rises again until the first off-delay's deadline
			// has passed, then falls for good.
			timer(IN := step = 0 OR step = 2, PT := T#50ms);
			IF step = 0 THEN
				step := 1;
			ELSIF step = 1 THEN
				// The first off-delay started in this cycle.
				t0 := PLC_TIME();
				step := 2;
			ELSIF step = 2 THEN
				IF PLC_TIME() - t0 >= T#60ms THEN
					// IN falls in the next cycle, after t1.
					t1 := PLC_TIME();
					step := 3;
				END_IF;
			ELSIF step = 3 THEN
				// New falling edge: deadline must be now+PT, not the stale t0+PT.
				ASSERT(timer.Q, 'TOF must use a fresh deadline for a new falling edge');
				step := 4;
			ELSIF NOT timer.Q THEN
				ASSERT(PLC_TIME() - t1 >= T#50ms, 'TOF must hold Q for PT after the new falling edge');
				DONE := TRUE;
			END_IF;
		END_FUNCTION_BLOCK

		{test}
		FUNCTION_BLOCK test_tof_et_clamps_at_pt
			VAR_OUTPUT
				DONE: BOOL;
			END_VAR
			VAR
				timer: TOF_TIME;
				step: INT;
				t0: TIME;
			END_VAR
			// IN on the first cycle, off from the second.
			timer(IN := step = 0, PT := T#1ms);
			IF step = 0 THEN
				step := 1;
			ELSIF step = 1 AND NOT timer.Q THEN
				t0 := PLC_TIME();
				step := 2;
			END_IF;
			IF step = 2 THEN
				// IN off 2 ms past the deadline: ET stays at PT.
				ASSERT(NOT timer.Q AND timer.ET = T#1ms, 'ET is exactly PT after the off-delay expires');
				DONE := PLC_TIME() - t0 >= T#2ms;
			END_IF;
		END_FUNCTION_BLOCK

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
		FUNCTION_BLOCK test_ton_ltime_expires
			VAR_OUTPUT
				DONE: BOOL;
			END_VAR
			VAR
				timer: TON_LTIME;
			END_VAR
			timer(IN := TRUE, PT := LTIME#1ms);
			IF timer.Q THEN
				ASSERT(timer.ET = LTIME#1ms, 'TON_LTIME ET is exactly PT after expiry');
				DONE := TRUE;
			END_IF;
		END_FUNCTION_BLOCK

		{test}
		FUNCTION_BLOCK test_tof_ltime_expires
			VAR_OUTPUT
				DONE: BOOL;
			END_VAR
			VAR
				timer: TOF_LTIME;
				step: INT;
			END_VAR
			// IN on the first cycle, off from the second.
			timer(IN := step = 0, PT := LTIME#1ms);
			IF step = 0 THEN
				ASSERT(timer.Q, 'TOF_LTIME Q should be TRUE while IN is TRUE');
				step := 1;
			ELSIF NOT timer.Q THEN
				ASSERT(timer.ET = LTIME#1ms, 'TOF_LTIME ET clamps at PT after expiry');
				DONE := TRUE;
			END_IF;
		END_FUNCTION_BLOCK

		{test}
		FUNCTION_BLOCK test_tp_ltime_pulse
			VAR_OUTPUT
				DONE: BOOL;
			END_VAR
			VAR
				timer: TP_LTIME;
				step: INT;
			END_VAR
			timer(IN := TRUE, PT := LTIME#1ms);
			IF step = 0 THEN
				ASSERT(timer.Q, 'TP_LTIME Q should be TRUE on rising edge');
				step := 1;
			ELSIF NOT timer.Q THEN
				ASSERT(timer.ET = LTIME#1ms, 'TP_LTIME ET clamps at PT when the pulse ends');
				DONE := TRUE;
			END_IF;
		END_FUNCTION_BLOCK

		// -- the standard names are aliases; instances declared through
		// them must build, call, and expose the outputs --

		{test}
		FUNCTION_BLOCK test_standard_names_are_usable
			VAR_OUTPUT
				DONE: BOOL;
			END_VAR
			VAR
				pulse: TP;
				on_delay: TON;
				off_delay: TOF;
				step: INT;
			END_VAR
			pulse(IN := TRUE, PT := T#100ms);
			on_delay(IN := TRUE, PT := T#1ms);
			// IN on the first cycle, off from the second.
			off_delay(IN := step = 0, PT := T#100ms);
			IF step = 0 THEN
				ASSERT(pulse.Q, 'a TP-typed instance pulses on the rising edge');
				step := 1;
			ELSIF step = 1 THEN
				ASSERT(off_delay.Q, 'a TOF-typed instance holds Q through the off-delay');
				step := 2;
			ELSIF on_delay.Q THEN
				ASSERT(on_delay.ET = T#1ms, 'a TON-typed instance expires at PT');
				DONE := TRUE;
			END_IF;
		END_FUNCTION_BLOCK

		// --- the clock itself ---

		{test}
		FUNCTION test_plc_time_monotonic
			VAR
				t1, t2: TIME;
			END_VAR
			t1 := PLC_TIME();
			t2 := PLC_TIME();
			// TIME wraps every 49.7 days: compare the difference, as a timer does.
			ASSERT(t2 - t1 >= T#0s, 'PLC_TIME must be monotonic');
		END_FUNCTION

		{test}
		FUNCTION test_plc_ltime_monotonic
			VAR
				t1, t2: LTIME;
			END_VAR
			t1 := PLC_LTIME();
			t2 := PLC_LTIME();
			ASSERT(t2 - t1 >= LTIME#0s, 'PLC_LTIME must be monotonic');
		END_FUNCTION

		// -- The wall clock ------------------------------------------

		{test}
		FUNCTION test_wall_clock_is_after_2026
			ASSERT(WALL_LDT() > LDT#2026-01-01-00:00:00, 'WALL_LDT is before 2026');
			ASSERT(WALL_DT() > DT#2026-01-01-00:00:00, 'WALL_DT is before 2026');
		END_FUNCTION

		{test}
		{allow 'warn-pragma'}
		FUNCTION test_local_clock_is_the_wall_clock_shifted
			VAR
				wall, local : LDT;
				offset : LTIME;
				shift : LINT;
			END_VAR
			wall := WALL_LDT();
			offset := LOCAL_OFFSET(IN := wall);
			local := LOCAL_LDT();
			ASSERT(offset >= LT#-18h AND offset <= LT#18h, 'the zone is more than a day from UTC');
			// `local` was read after `wall`: the readings are a moment apart.
			shift := LDT_TO_LINT(local) - LDT_TO_LINT(wall) - LTIME_TO_LINT(offset);
			ASSERT(shift >= LINT#0 AND shift < LINT#1000000000, 'LOCAL_LDT is not WALL_LDT shifted by LOCAL_OFFSET');
		END_FUNCTION
	END_NAMESPACE
END_NAMESPACE
