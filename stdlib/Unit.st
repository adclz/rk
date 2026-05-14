/*
# Unit Testing Utilities

Helper functions for writing test assertions in Structured Text.
*/
NAMESPACE Std.Unit
	USING Std.Strings;
	USING Std.Convert;

	FUNCTION ASSERT
		VAR_INPUT
			value: BOOL;
			message: STRING := '';
		END_VAR

		VAR
			final_message: STRING;
		END_VAR

		IF NOT value THEN
			IF LEN(message) = 0 THEN
				final_message := "assertion failed: ";
				final_message := CONCAT(final_message, ANY_TO_STRING(value));
				final_message := CONCAT(final_message, "is not true");
				__RAISE(final_message);
			ELSE
				__RAISE(CONCAT("assertion failed: ", message));
			END_IF
		END_IF;

	END_FUNCTION

	FUNCTION ASSERT_EQ
		VAR_INPUT
			value: ANY;
			target: INTO(value);
			message: STRING := '';
		END_VAR

		VAR 
			final_message: STRING;
		END_VAR

		IF value <> target THEN
			IF LEN(message) = 0 THEN
				final_message := "assertion failed: ";
				final_message := CONCAT(final_message, ANY_TO_STRING(value));
				final_message := CONCAT(final_message, " is not equal to ");
				final_message := CONCAT(final_message, ANY_TO_STRING(target));
				__RAISE(final_message);
			ELSE
				__RAISE(CONCAT("assertion failed: ", message));
			END_IF
		END_IF;

	END_FUNCTION

	FUNCTION ASSERT_NEQ
		VAR_INPUT
			value: ANY;
			target: INTO(value);
			message: STRING := '';
		END_VAR

		VAR 
			final_message: STRING;
		END_VAR

		IF value = target THEN
			IF LEN(message) = 0 THEN
				final_message := "assertion failed: ";
				final_message := CONCAT(final_message, ANY_TO_STRING(value));
				final_message := CONCAT(final_message, "is different than ");
				final_message := CONCAT(final_message, ANY_TO_STRING(target));
				__RAISE(final_message);
			ELSE
				__RAISE(CONCAT("assertion failed: ", message));
			END_IF
		END_IF;

	END_FUNCTION
END_NAMESPACE
