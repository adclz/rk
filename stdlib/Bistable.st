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


	/*
    ## SEMA - Semaphore

    A claim/release semaphore over a shared resource.

    - `CLAIM` = **TRUE** takes the semaphore
    - `RELEASE` = **TRUE** frees it
    - `BUSY` reports whether it was already taken when this call CLAIMed,
      and stays FALSE once released
    - If both are TRUE, **claim wins**

    ```iecst
    VAR
        sem: SEMA;
    END_VAR

    sem(CLAIM := want_it, RELEASE := done_with_it);
    IF NOT sem.BUSY THEN
        // the resource is ours
    END_IF;
    ```
    */
	FUNCTION_BLOCK SEMA
		VAR_INPUT
			CLAIM, RELEASE: BOOL;
		END_VAR
		VAR_OUTPUT
			BUSY: BOOL;
		END_VAR
		VAR
			X: BOOL;
		END_VAR

		BUSY := X;
		IF CLAIM THEN
			X := TRUE;
		ELSIF RELEASE THEN
			BUSY := FALSE;
			X := FALSE;
		END_IF;
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

		// --- the latch must LATCH: state persists through (FALSE, FALSE).
		// A memoryless body (`Q1 := S1`) passes every test above ---

		{test}
		FUNCTION test_sr_holds_state_after_release
			VAR
				latch: SR;
			END_VAR
			latch(TRUE, FALSE);
			latch(FALSE, FALSE);
			ASSERT(latch.Q1, 'SR must stay set once S1 is released');
			latch(FALSE, FALSE);
			ASSERT(latch.Q1, 'and stay set across further idle calls');
		END_FUNCTION

		{test}
		FUNCTION test_sr_stays_reset_after_release
			VAR
				latch: SR;
			END_VAR
			latch(TRUE, FALSE);
			latch(FALSE, TRUE);
			latch(FALSE, FALSE);
			ASSERT(NOT latch.Q1, 'SR must stay reset once R is released');
		END_FUNCTION

		{test}
		FUNCTION test_rs_holds_state_after_release
			VAR
				latch: RS;
			END_VAR
			latch(TRUE, FALSE);
			latch(FALSE, FALSE);
			ASSERT(latch.Q1, 'RS must stay set once S is released');
		END_FUNCTION

		{test}
		FUNCTION test_rs_dominance_is_not_sticky
			VAR
				latch: RS;
			END_VAR
			// Reset-dominant call, then a plain set must succeed again.
			latch(TRUE, TRUE);
			ASSERT(NOT latch.Q1);
			latch(TRUE, FALSE);
			ASSERT(latch.Q1, 'RS must set normally after a dominant reset');
			latch(FALSE, FALSE);
			ASSERT(latch.Q1, 'and hold it');
		END_FUNCTION

		{test}
		FUNCTION test_sr_toggle_sequence
			VAR
				latch: SR;
			END_VAR
			latch(TRUE, FALSE);
			latch(FALSE, TRUE);
			latch(TRUE, FALSE);
			latch(FALSE, FALSE);
			ASSERT(latch.Q1, 'set-reset-set-idle ends set');
		END_FUNCTION

		// -- SEMA -----------------------------------------------------

		{test}
		FUNCTION test_sema_first_claim_not_busy
			VAR
				sem: SEMA;
			END_VAR
			sem(CLAIM := TRUE, RELEASE := FALSE);
			ASSERT(NOT sem.BUSY, 'first claimant sees the semaphore free');
		END_FUNCTION

		{test}
		FUNCTION test_sema_second_claim_busy
			VAR
				sem: SEMA;
			END_VAR
			sem(CLAIM := TRUE, RELEASE := FALSE);
			sem(CLAIM := TRUE, RELEASE := FALSE);
			ASSERT(sem.BUSY, 'a taken semaphore reports BUSY');
		END_FUNCTION

		{test}
		FUNCTION test_sema_release_frees
			VAR
				sem: SEMA;
			END_VAR
			sem(CLAIM := TRUE, RELEASE := FALSE);
			sem(CLAIM := FALSE, RELEASE := TRUE);
			ASSERT(NOT sem.BUSY, 'released semaphore is not BUSY');
			sem(CLAIM := TRUE, RELEASE := FALSE);
			ASSERT(NOT sem.BUSY, 'and can be claimed again');
		END_FUNCTION

		{test}
		FUNCTION test_sema_claim_wins
			VAR
				sem: SEMA;
			END_VAR
			sem(CLAIM := TRUE, RELEASE := TRUE);
			sem(CLAIM := TRUE, RELEASE := FALSE);
			ASSERT(sem.BUSY, 'simultaneous claim and release: claim wins');
		END_FUNCTION
	END_NAMESPACE
END_NAMESPACE
