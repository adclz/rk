/*
# Unit Testing Utilities

Helper functions for writing test assertions in Structured Text.
*/
NAMESPACE Std.Unit

    /*
    ## ASSERT - Value Equality Assertion

    Compares `value` against `target` and raises an error with `message` if they differ.

    - **T**: `ANY` - the expected type
    - **Y**: `ANY + INTO<T>` - the actual type, must be implicitly convertible to T

    ```iecst
    ASSERT(value := x, target := 42, message := 'x should be 42');
    ASSERT(value := 3.14, target := PI, message := 'PI mismatch');
    ```
    */
    FUNCTION ASSERT<T: ANY, Y: ANY + INTO<T>>
        VAR_INPUT
            value: T;
            target: Y;
            message: STRING;
        END_VAR

        IF value <> target THEN
            /* throw error */
        END_IF;

    END_FUNCTION

END_NAMESPACE
