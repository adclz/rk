/*
# Standard Memory Functions

*/
NAMESPACE Std.Memory

    /*
    ## MOVE - Memory Copy

    Copies the input value to the output, unchanged.

    - **Constraint**: `ANY` (all types)
    */
    FUNCTION MOVE
        VAR_INPUT
            IN: ANY
            OUT: INTO(IN)
        END_VAR

        IN := OUT;

    END_FUNCTION
END_NAMESPACE
