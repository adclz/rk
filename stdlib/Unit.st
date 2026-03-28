/*
# Unit Testing Utilities

Helper functions for writing test assertions in Structured Text.
*/
NAMESPACE Std.Unit

    FUNCTION PRIVATE __ASSERT_FAIL
        VAR_INPUT
            message: STRING;
        END_VAR
        {extern 'assert' 'fail' (params message)}
    END_FUNCTION

    FUNCTION ASSERT
        VAR_INPUT
            value: BOOL;
            message: STRING := '';
        END_VAR

        IF NOT value THEN
            __ASSERT_FAIL(message);
        END_IF;

    END_FUNCTION

    FUNCTION ASSERT_EQ
        VAR_INPUT
            value: ANY;
            target: INTO(value);
            message: STRING := '';
        END_VAR

        IF value <> target THEN
            __ASSERT_FAIL(message);
        END_IF;

    END_FUNCTION

    FUNCTION ASSERT_NEQ
        VAR_INPUT
            value: ANY;
            target: INTO(value);
            message: STRING := '';
        END_VAR

        IF value = target THEN
            __ASSERT_FAIL(message);
        END_IF;

    END_FUNCTION

END_NAMESPACE
