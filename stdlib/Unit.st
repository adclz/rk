/*
Assertions for `{test}` functions.

A failed assertion raises, and `rk test` reports the test as failed with
its message.
*/
NAMESPACE Std.Unit
	USING Std.Strings;

	/* Raises `message` when `value` is `FALSE`.

	   # Examples

	   ```iecst
	   ASSERT(level < 100, 'the tank overflowed');
	   ```
	*/
	FUNCTION ASSERT
		VAR_INPUT
			value: BOOL;
			message: STRING := '';
		END_VAR

		IF NOT value THEN
			__RAISE(CONCAT('assertion failed: ', message));
		END_IF
	END_FUNCTION

	/* Raises `message` when `value` and `target` differ.

	   There is one overload per elementary type. Both must have the same type,
	   so write a typed literal where the compiler cannot tell: `INT#5`.

	   # Examples

	   ```iecst
	   ASSERT_EQ(ABS(-5), 5, 'ABS of a negative');
	   ```
	*/
	FUNCTION ASSERT_EQ
		VAR_INPUT
			value: BOOL;
			target: BOOL;
			message: STRING := '';
		END_VAR

		IF value <> target THEN
			__RAISE(CONCAT('assertion failed: ', message));
		END_IF
	END_FUNCTION

	FUNCTION ASSERT_EQ
		VAR_INPUT
			value: SINT;
			target: SINT;
			message: STRING := '';
		END_VAR

		IF value <> target THEN
			__RAISE(CONCAT('assertion failed: ', message));
		END_IF
	END_FUNCTION

	FUNCTION ASSERT_EQ
		VAR_INPUT
			value: INT;
			target: INT;
			message: STRING := '';
		END_VAR

		IF value <> target THEN
			__RAISE(CONCAT('assertion failed: ', message));
		END_IF
	END_FUNCTION

	FUNCTION ASSERT_EQ
		VAR_INPUT
			value: DINT;
			target: DINT;
			message: STRING := '';
		END_VAR

		IF value <> target THEN
			__RAISE(CONCAT('assertion failed: ', message));
		END_IF
	END_FUNCTION

	FUNCTION ASSERT_EQ
		VAR_INPUT
			value: LINT;
			target: LINT;
			message: STRING := '';
		END_VAR

		IF value <> target THEN
			__RAISE(CONCAT('assertion failed: ', message));
		END_IF
	END_FUNCTION

	FUNCTION ASSERT_EQ
		VAR_INPUT
			value: USINT;
			target: USINT;
			message: STRING := '';
		END_VAR

		IF value <> target THEN
			__RAISE(CONCAT('assertion failed: ', message));
		END_IF
	END_FUNCTION

	FUNCTION ASSERT_EQ
		VAR_INPUT
			value: UINT;
			target: UINT;
			message: STRING := '';
		END_VAR

		IF value <> target THEN
			__RAISE(CONCAT('assertion failed: ', message));
		END_IF
	END_FUNCTION

	FUNCTION ASSERT_EQ
		VAR_INPUT
			value: UDINT;
			target: UDINT;
			message: STRING := '';
		END_VAR

		IF value <> target THEN
			__RAISE(CONCAT('assertion failed: ', message));
		END_IF
	END_FUNCTION

	FUNCTION ASSERT_EQ
		VAR_INPUT
			value: ULINT;
			target: ULINT;
			message: STRING := '';
		END_VAR

		IF value <> target THEN
			__RAISE(CONCAT('assertion failed: ', message));
		END_IF
	END_FUNCTION

	FUNCTION ASSERT_EQ
		VAR_INPUT
			value: BYTE;
			target: BYTE;
			message: STRING := '';
		END_VAR

		IF value <> target THEN
			__RAISE(CONCAT('assertion failed: ', message));
		END_IF
	END_FUNCTION

	FUNCTION ASSERT_EQ
		VAR_INPUT
			value: WORD;
			target: WORD;
			message: STRING := '';
		END_VAR

		IF value <> target THEN
			__RAISE(CONCAT('assertion failed: ', message));
		END_IF
	END_FUNCTION

	FUNCTION ASSERT_EQ
		VAR_INPUT
			value: DWORD;
			target: DWORD;
			message: STRING := '';
		END_VAR

		IF value <> target THEN
			__RAISE(CONCAT('assertion failed: ', message));
		END_IF
	END_FUNCTION

	FUNCTION ASSERT_EQ
		VAR_INPUT
			value: LWORD;
			target: LWORD;
			message: STRING := '';
		END_VAR

		IF value <> target THEN
			__RAISE(CONCAT('assertion failed: ', message));
		END_IF
	END_FUNCTION

	FUNCTION ASSERT_EQ
		VAR_INPUT
			value: REAL;
			target: REAL;
			message: STRING := '';
		END_VAR

		IF value <> target THEN
			__RAISE(CONCAT('assertion failed: ', message));
		END_IF
	END_FUNCTION

	FUNCTION ASSERT_EQ
		VAR_INPUT
			value: LREAL;
			target: LREAL;
			message: STRING := '';
		END_VAR

		IF value <> target THEN
			__RAISE(CONCAT('assertion failed: ', message));
		END_IF
	END_FUNCTION

	FUNCTION ASSERT_EQ
		VAR_INPUT
			value: TIME;
			target: TIME;
			message: STRING := '';
		END_VAR

		IF value <> target THEN
			__RAISE(CONCAT('assertion failed: ', message));
		END_IF
	END_FUNCTION

	FUNCTION ASSERT_EQ
		VAR_INPUT
			value: LTIME;
			target: LTIME;
			message: STRING := '';
		END_VAR

		IF value <> target THEN
			__RAISE(CONCAT('assertion failed: ', message));
		END_IF
	END_FUNCTION

	FUNCTION ASSERT_EQ
		VAR_INPUT
			value: STRING;
			target: STRING;
			message: STRING := '';
		END_VAR

		IF value <> target THEN
			__RAISE(CONCAT('assertion failed: ', message));
		END_IF
	END_FUNCTION

	FUNCTION ASSERT_EQ
		VAR_INPUT
			value: CHAR;
			target: CHAR;
			message: STRING := '';
		END_VAR

		IF value <> target THEN
			__RAISE(CONCAT('assertion failed: ', message));
		END_IF
	END_FUNCTION

	/* Raises `message` when `value` and `target` are equal.

	   There is one overload per elementary type.
	*/
	FUNCTION ASSERT_NEQ
		VAR_INPUT
			value: BOOL;
			target: BOOL;
			message: STRING := '';
		END_VAR

		IF value = target THEN
			__RAISE(CONCAT('assertion failed: ', message));
		END_IF
	END_FUNCTION

	FUNCTION ASSERT_NEQ
		VAR_INPUT
			value: SINT;
			target: SINT;
			message: STRING := '';
		END_VAR

		IF value = target THEN
			__RAISE(CONCAT('assertion failed: ', message));
		END_IF
	END_FUNCTION

	FUNCTION ASSERT_NEQ
		VAR_INPUT
			value: INT;
			target: INT;
			message: STRING := '';
		END_VAR

		IF value = target THEN
			__RAISE(CONCAT('assertion failed: ', message));
		END_IF
	END_FUNCTION

	FUNCTION ASSERT_NEQ
		VAR_INPUT
			value: DINT;
			target: DINT;
			message: STRING := '';
		END_VAR

		IF value = target THEN
			__RAISE(CONCAT('assertion failed: ', message));
		END_IF
	END_FUNCTION

	FUNCTION ASSERT_NEQ
		VAR_INPUT
			value: LINT;
			target: LINT;
			message: STRING := '';
		END_VAR

		IF value = target THEN
			__RAISE(CONCAT('assertion failed: ', message));
		END_IF
	END_FUNCTION

	FUNCTION ASSERT_NEQ
		VAR_INPUT
			value: USINT;
			target: USINT;
			message: STRING := '';
		END_VAR

		IF value = target THEN
			__RAISE(CONCAT('assertion failed: ', message));
		END_IF
	END_FUNCTION

	FUNCTION ASSERT_NEQ
		VAR_INPUT
			value: UINT;
			target: UINT;
			message: STRING := '';
		END_VAR

		IF value = target THEN
			__RAISE(CONCAT('assertion failed: ', message));
		END_IF
	END_FUNCTION

	FUNCTION ASSERT_NEQ
		VAR_INPUT
			value: UDINT;
			target: UDINT;
			message: STRING := '';
		END_VAR

		IF value = target THEN
			__RAISE(CONCAT('assertion failed: ', message));
		END_IF
	END_FUNCTION

	FUNCTION ASSERT_NEQ
		VAR_INPUT
			value: ULINT;
			target: ULINT;
			message: STRING := '';
		END_VAR

		IF value = target THEN
			__RAISE(CONCAT('assertion failed: ', message));
		END_IF
	END_FUNCTION

	FUNCTION ASSERT_NEQ
		VAR_INPUT
			value: BYTE;
			target: BYTE;
			message: STRING := '';
		END_VAR

		IF value = target THEN
			__RAISE(CONCAT('assertion failed: ', message));
		END_IF
	END_FUNCTION

	FUNCTION ASSERT_NEQ
		VAR_INPUT
			value: WORD;
			target: WORD;
			message: STRING := '';
		END_VAR

		IF value = target THEN
			__RAISE(CONCAT('assertion failed: ', message));
		END_IF
	END_FUNCTION

	FUNCTION ASSERT_NEQ
		VAR_INPUT
			value: DWORD;
			target: DWORD;
			message: STRING := '';
		END_VAR

		IF value = target THEN
			__RAISE(CONCAT('assertion failed: ', message));
		END_IF
	END_FUNCTION

	FUNCTION ASSERT_NEQ
		VAR_INPUT
			value: LWORD;
			target: LWORD;
			message: STRING := '';
		END_VAR

		IF value = target THEN
			__RAISE(CONCAT('assertion failed: ', message));
		END_IF
	END_FUNCTION

	FUNCTION ASSERT_NEQ
		VAR_INPUT
			value: REAL;
			target: REAL;
			message: STRING := '';
		END_VAR

		IF value = target THEN
			__RAISE(CONCAT('assertion failed: ', message));
		END_IF
	END_FUNCTION

	FUNCTION ASSERT_NEQ
		VAR_INPUT
			value: LREAL;
			target: LREAL;
			message: STRING := '';
		END_VAR

		IF value = target THEN
			__RAISE(CONCAT('assertion failed: ', message));
		END_IF
	END_FUNCTION

	FUNCTION ASSERT_NEQ
		VAR_INPUT
			value: TIME;
			target: TIME;
			message: STRING := '';
		END_VAR

		IF value = target THEN
			__RAISE(CONCAT('assertion failed: ', message));
		END_IF
	END_FUNCTION

	FUNCTION ASSERT_NEQ
		VAR_INPUT
			value: LTIME;
			target: LTIME;
			message: STRING := '';
		END_VAR

		IF value = target THEN
			__RAISE(CONCAT('assertion failed: ', message));
		END_IF
	END_FUNCTION

	FUNCTION ASSERT_NEQ
		VAR_INPUT
			value: STRING;
			target: STRING;
			message: STRING := '';
		END_VAR

		IF value = target THEN
			__RAISE(CONCAT('assertion failed: ', message));
		END_IF
	END_FUNCTION

	FUNCTION ASSERT_NEQ
		VAR_INPUT
			value: CHAR;
			target: CHAR;
			message: STRING := '';
		END_VAR

		IF value = target THEN
			__RAISE(CONCAT('assertion failed: ', message));
		END_IF
	END_FUNCTION

	// The six calendar types

	FUNCTION ASSERT_EQ
		VAR_INPUT
			value: DATE;
			target: DATE;
			message: STRING := '';
		END_VAR

		IF value <> target THEN
			__RAISE(CONCAT('assertion failed: ', message));
		END_IF
	END_FUNCTION

	FUNCTION ASSERT_EQ
		VAR_INPUT
			value: LDATE;
			target: LDATE;
			message: STRING := '';
		END_VAR

		IF value <> target THEN
			__RAISE(CONCAT('assertion failed: ', message));
		END_IF
	END_FUNCTION

	FUNCTION ASSERT_EQ
		VAR_INPUT
			value: TIME_OF_DAY;
			target: TIME_OF_DAY;
			message: STRING := '';
		END_VAR

		IF value <> target THEN
			__RAISE(CONCAT('assertion failed: ', message));
		END_IF
	END_FUNCTION

	FUNCTION ASSERT_EQ
		VAR_INPUT
			value: LTIME_OF_DAY;
			target: LTIME_OF_DAY;
			message: STRING := '';
		END_VAR

		IF value <> target THEN
			__RAISE(CONCAT('assertion failed: ', message));
		END_IF
	END_FUNCTION

	FUNCTION ASSERT_EQ
		VAR_INPUT
			value: DATE_AND_TIME;
			target: DATE_AND_TIME;
			message: STRING := '';
		END_VAR

		IF value <> target THEN
			__RAISE(CONCAT('assertion failed: ', message));
		END_IF
	END_FUNCTION

	FUNCTION ASSERT_EQ
		VAR_INPUT
			value: LDATE_AND_TIME;
			target: LDATE_AND_TIME;
			message: STRING := '';
		END_VAR

		IF value <> target THEN
			__RAISE(CONCAT('assertion failed: ', message));
		END_IF
	END_FUNCTION

	FUNCTION ASSERT_NEQ
		VAR_INPUT
			value: DATE;
			target: DATE;
			message: STRING := '';
		END_VAR

		IF value = target THEN
			__RAISE(CONCAT('assertion failed: ', message));
		END_IF
	END_FUNCTION

	FUNCTION ASSERT_NEQ
		VAR_INPUT
			value: LDATE;
			target: LDATE;
			message: STRING := '';
		END_VAR

		IF value = target THEN
			__RAISE(CONCAT('assertion failed: ', message));
		END_IF
	END_FUNCTION

	FUNCTION ASSERT_NEQ
		VAR_INPUT
			value: TIME_OF_DAY;
			target: TIME_OF_DAY;
			message: STRING := '';
		END_VAR

		IF value = target THEN
			__RAISE(CONCAT('assertion failed: ', message));
		END_IF
	END_FUNCTION

	FUNCTION ASSERT_NEQ
		VAR_INPUT
			value: LTIME_OF_DAY;
			target: LTIME_OF_DAY;
			message: STRING := '';
		END_VAR

		IF value = target THEN
			__RAISE(CONCAT('assertion failed: ', message));
		END_IF
	END_FUNCTION

	FUNCTION ASSERT_NEQ
		VAR_INPUT
			value: DATE_AND_TIME;
			target: DATE_AND_TIME;
			message: STRING := '';
		END_VAR

		IF value = target THEN
			__RAISE(CONCAT('assertion failed: ', message));
		END_IF
	END_FUNCTION

	FUNCTION ASSERT_NEQ
		VAR_INPUT
			value: LDATE_AND_TIME;
			target: LDATE_AND_TIME;
			message: STRING := '';
		END_VAR

		IF value = target THEN
			__RAISE(CONCAT('assertion failed: ', message));
		END_IF
	END_FUNCTION

	NAMESPACE Test
		USING Std.Unit;

		{test}
		FUNCTION test_assert_eq_accepts_every_date_and_time_type
			ASSERT_EQ(value := D#2024-02-29, target := D#2024-02-29, message := 'DATE');
			ASSERT_EQ(value := LD#2024-02-29, target := LD#2024-02-29, message := 'LDATE');
			ASSERT_EQ(value := TOD#12:30:45.123, target := TOD#12:30:45.123, message := 'TOD');
			ASSERT_EQ(value := LTOD#12:30:45.123456789, target := LTOD#12:30:45.123456789, message := 'LTOD');
			ASSERT_EQ(value := DT#2024-02-29-12:30:45, target := DT#2024-02-29-12:30:45, message := 'DT');
			ASSERT_EQ(value := LDT#2024-02-29-12:30:45.5, target := LDT#2024-02-29-12:30:45.5, message := 'LDT');
		END_FUNCTION

		{test}
		FUNCTION test_assert_neq_sees_a_different_date_or_time
			ASSERT_NEQ(value := D#2024-02-29, target := D#2024-03-01, message := 'DATE');
			ASSERT_NEQ(value := LD#1969-12-31, target := LD#1970-01-01, message := 'LDATE, across the epoch');
			ASSERT_NEQ(value := TOD#12:30:45, target := TOD#12:30:45.001, message := 'TOD, a millisecond apart');
			ASSERT_NEQ(value := LTOD#12:30:45, target := LTOD#12:30:45.000000001, message := 'LTOD, a nanosecond apart');
			ASSERT_NEQ(value := DT#2024-02-29-12:30:45, target := DT#2024-02-29-12:30:46, message := 'DT');
			ASSERT_NEQ(value := LDT#2024-02-29-12:30:45.5, target := LDT#2024-02-29-12:30:45.6, message := 'LDT');
		END_FUNCTION
	END_NAMESPACE

END_NAMESPACE
