/* Generated from the WIT of wasi:clocks@0.3.0. DO NOT EDIT! */
NAMESPACE INTERNAL Wasi
	NAMESPACE Clocks
		/* WASI System Clock is a clock API intended to let users query the current
		   time. The clock is not necessarily monotonic as it may be reset.

		   It is intended to be portable at least between Unix-family platforms and
		   Windows.

		   External references may be reset, so this clock is not necessarily
		   monotonic, making it unsuitable for measuring elapsed time.

		   It is intended for reporting the current date and time for humans. */
		NAMESPACE SystemClock
			/* Read the current value of the clock.

			   This clock is not monotonic, therefore calling this function repeatedly
			   will not necessarily produce a sequence of non-decreasing values.

			   The nanoseconds field of the output is always less than 1000000000. */
			{extern 'wasi:clocks/system-clock@0.3.0' 'now'}
			FUNCTION NOW
			VAR_OUTPUT
				seconds : LINT;
				nanoseconds : UDINT;
			END_VAR
			END_FUNCTION

			/* Query the resolution of the clock. Returns the smallest duration of time
			   that the implementation permits distinguishing. */
			{extern 'wasi:clocks/system-clock@0.3.0' 'get-resolution'}
			FUNCTION GET_RESOLUTION : Wasi.Clocks.Types.DURATION
			END_FUNCTION
		END_NAMESPACE
	END_NAMESPACE
END_NAMESPACE
