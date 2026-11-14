/* Generated from the WIT of wasi:clocks@0.3.0. DO NOT EDIT! */
NAMESPACE INTERNAL Wasi
	NAMESPACE Clocks
		/* WASI Monotonic Clock is a clock API intended to let users measure elapsed
		   time.

		   It is intended to be portable at least between Unix-family platforms and
		   Windows.

		   A monotonic clock is a clock which has an unspecified initial value, and
		   successive reads of the clock will produce non-decreasing values. */
		NAMESPACE MonotonicClock
			/* A mark on a monotonic clock is a number of nanoseconds since an
			   unspecified initial value, and can only be compared to instances from
			   the same monotonic-clock. */
			TYPE MARK : ULINT; END_TYPE

			/* Read the current value of the clock.

			   The clock is monotonic, therefore calling this function repeatedly will
			   produce a sequence of non-decreasing values.

			   For completeness, this function traps if it's not possible to represent
			   the value of the clock in a `mark`. Consequently, implementations
			   should ensure that the starting time is low enough to avoid the
			   possibility of overflow in practice. */
			{extern 'wasi:clocks/monotonic-clock@0.3.0' 'now'}
			FUNCTION NOW : MARK
			END_FUNCTION

			/* Query the resolution of the clock. Returns the duration of time
			   corresponding to a clock tick. */
			{extern 'wasi:clocks/monotonic-clock@0.3.0' 'get-resolution'}
			FUNCTION GET_RESOLUTION : Wasi.Clocks.Types.DURATION
			END_FUNCTION

			/* Wait until the specified mark has occurred. */
			{extern async 'wasi:clocks/monotonic-clock@0.3.0' 'wait-until'}
			FUNCTION_BLOCK WAIT_UNTIL
			VAR_INPUT
				when : MARK;
			END_VAR
			VAR_OUTPUT
				DONE : BOOL;
			END_VAR
			END_FUNCTION_BLOCK

			/* Wait for the specified duration to elapse. */
			{extern async 'wasi:clocks/monotonic-clock@0.3.0' 'wait-for'}
			FUNCTION_BLOCK WAIT_FOR
			VAR_INPUT
				how_long : Wasi.Clocks.Types.DURATION;
			END_VAR
			VAR_OUTPUT
				DONE : BOOL;
			END_VAR
			END_FUNCTION_BLOCK
		END_NAMESPACE
	END_NAMESPACE
END_NAMESPACE
