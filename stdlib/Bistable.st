/*
# Standard Bistable Function Blocks

IEC 61131-3 **Table 34** - Set/Reset latches.

`SR` is set-dominant (set wins on simultaneous inputs).
`RS` is reset-dominant (reset wins on simultaneous inputs).
*/
NAMESPACE Std.Bistable

	/*
    ## SR - Set-Dominant Bistable

    - `S1` = **TRUE** sets output `Q1` to TRUE
    - `R` = **TRUE** resets output `Q1` to FALSE
    - If both are TRUE, **set wins**

    ```iecst
    VAR
        latch: SR;
    END_VAR
    
    latch(start_btn, stop_btn);
    motor := latch.Q1;
    ```
    */
	FUNCTION_BLOCK SR
		VAR_INPUT
			S1, R: BOOL;
		END_VAR
		VAR_OUTPUT
			Q1: BOOL;
		END_VAR Q1 := S1 OR ((NOT R) AND Q1);
	END_FUNCTION_BLOCK

	/*
    ## RS - Reset-Dominant Bistable

    - `S` = **TRUE** sets output `Q1` to TRUE
    - `R1` = **TRUE** resets output `Q1` to FALSE
    - If both are TRUE, **reset wins**

    ```iecst
    VAR
        latch: RS;
    END_VAR
    
    latch(start_btn, emergency_stop);
    motor := latch.Q1;
    ```
    */
	FUNCTION_BLOCK RS
		VAR_INPUT
			S, R1: BOOL;
		END_VAR
		VAR_OUTPUT
			Q1: BOOL;
		END_VAR Q1 := (NOT R1) AND (S OR Q1);
	END_FUNCTION_BLOCK

	NAMESPACE Test
		USING Std.Unit
		USING Std.Bits

		{test}
		FUNCTION test_sr_set
			VAR
				latch: SR;
			END_VAR
			latch(TRUE, FALSE);
			ASSERT(latch.Q1);
		END_FUNCTION

		{test}
		FUNCTION test_sr_reset
			VAR
				latch: SR;
			END_VAR
			// First set
			latch(TRUE, FALSE);
			// Then reset
			latch(FALSE, TRUE);
			ASSERT(NOT latch.Q1);
		END_FUNCTION

		{test}
		FUNCTION test_sr_set_dominant
			VAR
				latch: SR;
			END_VAR
			// Both inputs TRUE: set wins
			latch(TRUE, TRUE);
			ASSERT(latch.Q1);
		END_FUNCTION

		{test}
		FUNCTION test_rs_set
			VAR
				latch: RS;
			END_VAR
			latch(TRUE, FALSE);
			ASSERT(latch.Q1);
		END_FUNCTION

		{test}
		FUNCTION test_rs_reset_dominant
			VAR
				latch: RS;
			END_VAR
			// Both inputs TRUE: reset wins
			latch(TRUE, TRUE);
			ASSERT(NOT latch.Q1);
		END_FUNCTION
	END_NAMESPACE
END_NAMESPACE
