
FUNCTION f : DINT
VAR CONSTANT
    N : DINT := 200 * 200;
    HALF : INT := 20000;
END_VAR
VAR
    x : DINT;
END_VAR
    x := 32767 + 1;
    x := INT#200 * INT#200;
    x := (HALF * 2) + 1;
    f := N;
END_FUNCTION
