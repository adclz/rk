(* `arr` is given a repetition of brackets with no brackets of its own. The
   checks passed it and nothing placed its values, so `m.arr` stayed 0, and
   the assertion that an initializer drops no value panicked. It is E0515. *)

TYPE M : STRUCT arr : ARRAY[0..3] OF INT; END_STRUCT; END_TYPE

FUNCTION F
VAR
    m : M := (arr := 2([1, 2]));
END_VAR
END_FUNCTION
