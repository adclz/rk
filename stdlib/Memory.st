/*
# Standard Memory Functions

*/
NAMESPACE Std.Memory

    /*
    ## MOVE - Memory Copy

    Copies the input value to the output, unchanged.

    - **Constraint**: `ANY` (all types)

    ```iecst
    result := MOVE(42);         // 42
    result := MOVE(IN := 3.14); // 3.14
    ```
    */
    FUNCTION MOVE<T: ANY> : T
        VAR_INPUT
            args: T
        END_VAR
    END_FUNCTION
END_NAMESPACE