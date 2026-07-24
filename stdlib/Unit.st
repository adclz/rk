/*
# Unit Testing Utilities

Helper functions for writing test assertions in Structured Text.
*/
NAMESPACE Std.Unit
	USING Std.Strings;

	/*
    ## ASSERT - Boolean assertion

    Raises with `message` when `value` is FALSE.
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

	/*
    ## ASSERT_EQ - Equality assertion

    One overload per elementary type. Raises when `value <> target`.
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

	// NOTE: no STRING overload — codegen has no STRING comparison yet. Assert
	// string equality via `ASSERT(a = b)` once that lands, or compare lengths.

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

	/*
    ## ASSERT_NEQ - Inequality assertion

    One overload per elementary type. Raises when `value = target`.
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

	// NOTE: no STRING overload — codegen has no STRING comparison yet.

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

END_NAMESPACE
