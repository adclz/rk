(* `Point` and `Holder` are declared nowhere. Past an unknown type the walk
   says nothing more and places nothing, and the assertion that no value is
   dropped panicked on a program that never compiles. *)

FUNCTION f
VAR
    rows : ARRAY[0..1] OF Point := [(1, 2), (3, 4)];
    nested : Holder := (p := (y := 3, 4), 5);
END_VAR
END_FUNCTION
