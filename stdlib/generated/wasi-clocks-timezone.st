/* Generated from the WIT of wasi:clocks@0.3.0. DO NOT EDIT! */
NAMESPACE INTERNAL Wasi
	NAMESPACE Clocks
		NAMESPACE Timezone
			/* `iana-id` as the host gives it: its result in words of 8 bytes, as the
			   canonical ABI lays it out, which `IANA_ID` reads, and its string in
			   `value`, where the host writes it. */
			{extern 'wasi:clocks/timezone@0.3.0' 'iana-id'}
			FUNCTION PRIVATE IANA_ID_RAW
			VAR_IN_OUT
				value : STRING;
			END_VAR
			VAR_OUTPUT
				w0 : LWORD;
				w1 : LWORD;
			END_VAR
			END_FUNCTION

			/* Return the IANA identifier of the currently configured timezone. This
			   should be an identifier from the IANA Time Zone Database.

			   For displaying to a user, the identifier should be converted into a
			   localized name by means of an internationalization API.

			   If the implementation does not expose an actual timezone, or is unable
			   to provide mappings from times to deltas between the configured timezone
			   and UTC, or determining the current timezone fails, or the timezone does
			   not have an IANA identifier, this returns nothing. */
			FUNCTION IANA_ID
			VAR_IN_OUT
				value : STRING;
			END_VAR
			VAR_OUTPUT
				is_some : BOOL;
			END_VAR
			VAR
				w0 : LWORD;
				w1 : LWORD;
			END_VAR
				IANA_ID_RAW(value := value, w0 => w0, w1 => w1);
				is_some := w0.%X0;
			END_FUNCTION

			/* `utc-offset` as the host takes it: its parameters as the canonical ABI
			   passes them, which `UTC_OFFSET` passes field by field. */
			{extern 'wasi:clocks/timezone@0.3.0' 'utc-offset'}
			FUNCTION PRIVATE UTC_OFFSET_RAW
			VAR_INPUT
				when_seconds : LINT;
				when_nanoseconds : UDINT;
			END_VAR
			VAR_OUTPUT
				is_some : BOOL;
				value : LINT;
			END_VAR
			END_FUNCTION

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
			FUNCTION UTC_OFFSET
			VAR_INPUT
				when : Wasi.Clocks.SystemClock.INSTANT;
			END_VAR
			VAR_OUTPUT
				is_some : BOOL;
				value : LINT;
			END_VAR
				UTC_OFFSET_RAW(when_seconds := when.seconds, when_nanoseconds := when.nanoseconds, is_some => is_some, value => value);
			END_FUNCTION

			/* `to-debug-string` as the host gives it: its result in words of 8 bytes,
			   as the canonical ABI lays it out, which `TO_DEBUG_STRING` reads, and its
			   string in `value`, where the host writes it. */
			{extern 'wasi:clocks/timezone@0.3.0' 'to-debug-string'}
			FUNCTION PRIVATE TO_DEBUG_STRING_RAW
			VAR_IN_OUT
				value : STRING;
			END_VAR
			VAR_OUTPUT
				w0 : LWORD;
				w1 : LWORD;
			END_VAR
			END_FUNCTION

			/* Returns a string that is suitable to assist humans in debugging whether
			   any timezone is available, and if so, which. This may be the same string
			   as `iana-id`, or a formatted representation of the UTC offset such as
			   `-04:00`, or something else.

			   WARNING: The returned string should not be consumed mechanically! It may
			   change across platforms, hosts, or other implementation details. Parsing
			   this string is a major platform-compatibility hazard. */
			FUNCTION TO_DEBUG_STRING
			VAR_IN_OUT
				value : STRING;
			END_VAR
			VAR
				w0 : LWORD;
				w1 : LWORD;
			END_VAR
				TO_DEBUG_STRING_RAW(value := value, w0 => w0, w1 => w1);
			END_FUNCTION
		END_NAMESPACE
	END_NAMESPACE
END_NAMESPACE
