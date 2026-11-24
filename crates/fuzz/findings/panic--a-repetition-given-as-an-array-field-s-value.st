(* `arr` is given a repetition of brackets with no brackets of its own: the
   checks pass it, and nothing places its values, so `m.arr` stays 0. The
   assertion that an initializer drops no value panics on it. *)

TYPE M : STRUCT arr : ARRAY[0..3] OF INT; END_STRUCT; END_TYPE

FUNCTION F
VAR
    m : M := (arr := 2([1, 2]));
END_VAR
END_FUNCTION
