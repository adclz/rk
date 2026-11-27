(* The standard's table of user-defined types gives each instance of an array
   of TON its PT with a repetition of the member value, which has no
   parentheses of its own. The checks passed it, nothing placed PT, and the
   assertion that no value is dropped panicked. Each instance gets it now. *)

FUNCTION_BLOCK Timer
VAR_INPUT PT : TIME; END_VAR
END_FUNCTION_BLOCK

TYPE Timers : ARRAY[1..20] OF Timer := [20(PT := T#250ms)]; END_TYPE

PROGRAM P
VAR t : Timers; END_VAR
    t[1]();
END_PROGRAM
