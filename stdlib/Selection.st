/*
# Standard Selection Functions

IEC 61131-3 **Table 28** - Selection and comparison operations.

Binary selection (`SEL`), extremum (`MAX`, `MIN`), and bounding (`LIMIT`).
*/
NAMESPACE Std.Selection

    /*
    ## SEL - Binary Selection

    Selects one of two inputs based on a boolean selector.

    - `G` = **FALSE** returns `IN0`
    - `G` = **TRUE** returns `IN1`

    ```iecst
    result := SEL(G := TRUE, IN0 := 0, IN1 := 42); // 42
    result := SEL(G := FALSE, IN0 := 0, IN1 := 42); // 0
    ```
    */
    FUNCTION SEL : ANY
        VAR_INPUT
            G: BOOL;
            IN0: INTO(SEL);
            IN1: INTO(SEL);
        END_VAR

        IF G THEN
            SEL := IN1;
        ELSE
            SEL := IN0;
        END_IF;
    END_FUNCTION

    /*
    ## MAX - Maximum

    Returns the largest of all input arguments.

    - **Constraint**: `ANY_ELEMENTARY`

    ```iecst
    result := MAX(1, 5, 3);        // 5
    result := MAX(1.0, 2.5, 0.5);  // 2.5
    ```
    */
    FUNCTION MAX : ANY_ELEMENTARY
        VAR_INPUT
            args: INTO(MAX)...
        END_VAR
    END_FUNCTION

    /*
    ## MIN - Minimum

    Returns the smallest of all input arguments.

    - **Constraint**: `ANY_ELEMENTARY`

    ```iecst
    result := MIN(1, 5, 3);        // 1
    result := MIN(1.0, 2.5, 0.5);  // 0.5
    ```
    */
    FUNCTION MIN : ANY_ELEMENTARY
        VAR_INPUT
            args: INTO(MIN)...
        END_VAR
    END_FUNCTION

    /*
    ## LIMIT - Bounding

    Clamps `IN` to the range [`MN`, `MX`].

    - Returns `MN` if `IN < MN`
    - Returns `MX` if `IN > MX`
    - Returns `IN` otherwise

    ```iecst
    result := LIMIT(MN := 0, IN := 15, MX := 10);  // 10
    result := LIMIT(MN := 0, IN := -5, MX := 10);   // 0
    result := LIMIT(MN := 0, IN := 5, MX := 10);    // 5
    ```
    */
    FUNCTION LIMIT : ANY_ELEMENTARY
        VAR_INPUT
            MN: INTO(LIMIT);
            IN: INTO(LIMIT);
            MX: INTO(LIMIT);
        END_VAR

        IF IN < MN THEN
            LIMIT := MN;
        ELSIF IN > MX THEN
            LIMIT := MX;
        ELSE
            LIMIT := IN;
        END_IF;
    END_FUNCTION

    NAMESPACE Test
        USING Std.Unit

        {test}
        FUNCTION test_sel_true
            ASSERT(SEL(G := TRUE, IN0 := 0, IN1 := 42) = 42);
        END_FUNCTION

        {test}
        FUNCTION test_sel_false
            ASSERT(SEL(G := FALSE, IN0 := 10, IN1 := 42) = 10);
        END_FUNCTION

        {test}
        FUNCTION test_limit_in_range
            ASSERT(LIMIT(MN := 0, IN := 5, MX := 10) = 5);
        END_FUNCTION

        {test}
        FUNCTION test_limit_below
            ASSERT(LIMIT(MN := 0, IN := -5, MX := 10) = 0);
        END_FUNCTION

        {test}
        FUNCTION test_limit_above
            ASSERT(LIMIT(MN := 0, IN := 15, MX := 10) = 10);
        END_FUNCTION
    END_NAMESPACE

END_NAMESPACE
