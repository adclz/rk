/* Generated from the WIT of wasi:clocks@0.3.0. DO NOT EDIT! */
NAMESPACE INTERNAL Wasi
	NAMESPACE Clocks
		NAMESPACE Timezone
			/* The number of nanoseconds difference between UTC time and the local
			   time of the currently configured timezone, at the exact time of
			   `instant`.

			   The magnitude of the returned value will always be less than
			   86,400,000,000,000 which is the number of nanoseconds in a day
			   (24*60*60*1e9).

			   If the implementation does not expose an actual timezone, or is unable
			   to provide mappings from times to deltas between the configured timezone
			   and UTC, or determining the current timezone fails, this returns
			   nothing. */
			{extern 'wasi:clocks/timezone@0.3.0' 'utc-offset'}
			FUNCTION UTC_OFFSET
			VAR_INPUT
				when_seconds : LINT;
				when_nanoseconds : UDINT;
			END_VAR
			VAR_OUTPUT
				is_some : BOOL;
				value : LINT;
			END_VAR
			END_FUNCTION
		END_NAMESPACE
	END_NAMESPACE
END_NAMESPACE
/* Not generated:
   - `iana-id`: its result holds a string, which an import cannot give back
     yet.
   - `to-debug-string`: its result holds a string, which an import cannot
     give back yet. */
