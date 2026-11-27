(* IEC 61131-3 Table 11, feature 5b, writes `[50(PT:=T#100ms)]` for an array
   of TON: each of the 50 instances gets PT. The member value is repeated
   without parentheses of its own: the checks pass it, nothing places PT, and
   the assertion that no value is dropped panics. *)

FUNCTION_BLOCK Timer
VAR_INPUT PT : TIME; END_VAR
END_FUNCTION_BLOCK

TYPE Timers : ARRAY[1..50] OF Timer := [50(PT := T#100ms)]; END_TYPE

PROGRAM P
VAR t : Timers; END_VAR
    t[1]();
END_PROGRAM
