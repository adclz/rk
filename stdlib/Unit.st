/*
# Unit Testing Utilities

Helper functions for writing test assertions in Structured Text.
*/
NAMESPACE Std.Unit

    FUNCTION __ASSERT_FAIL
        {extern 'assert' 'fail'}
    END_FUNCTION

    FUNCTION ASSERT
        VAR_INPUT
            value: BOOL;
        END_VAR

        IF NOT value THEN
            __ASSERT_FAIL();
        END_IF;

    END_FUNCTION

    FUNCTION ASSERT_EQ
        VAR_INPUT
            value: ANY;
            target: INTO(value);
        END_VAR

        IF value <> target THEN
            __ASSERT_FAIL();
        END_IF;

    END_FUNCTION

    FUNCTION ASSERT_NEQ
        VAR_INPUT
            value: ANY;
            target: INTO(value);
        END_VAR

        IF value = target THEN
            __ASSERT_FAIL();
        END_IF;

    END_FUNCTION

END_NAMESPACE
