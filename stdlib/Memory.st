/*
# Standard Memory Functions

*/
NAMESPACE Std.Memory

    /*
    ## MOVE - Memory Copy

    Copies the input value to the output.

    - **Constraint**: `ANY` (all types)
    */
    FUNCTION MOVE
        VAR_INPUT
            IN: ANY;
        END_VAR
        VAR_OUTPUT
            OUT: INTO(IN);
        END_VAR

        OUT := IN;

    END_FUNCTION

    NAMESPACE Test
        USING Std.Unit
        USING Std.Memory

        {test}
        FUNCTION test_move_int
        VAR x : INT; END_VAR
            MOVE(42, OUT => x);
            ASSERT_EQ(x, 42);
        END_FUNCTION

        {test}
        FUNCTION test_move_real
        VAR x : REAL; END_VAR
            MOVE(3.14, OUT => x);
            ASSERT_EQ(x, 3.14);
        END_FUNCTION

        {test}
        FUNCTION test_move_bool
        VAR x : BOOL; END_VAR
            MOVE(TRUE, OUT => x);
            ASSERT(x);
        END_FUNCTION

    END_NAMESPACE
END_NAMESPACE
