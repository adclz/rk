TYPE
    Row : ARRAY[0..1] OF INT;
END_TYPE

PROGRAM Main
VAR
    l : LINT;
    x : REAL;
    s : STRING;
    row : Row;
    rl : REF_TO LINT;
    rx : REF_TO REAL;
    rs : REF_TO STRING;
    rrow : REF_TO Row;
    a, b, c, d : BOOL;
END_VAR
    rl := REF(l);
    rx := REF(x);
    rs := REF(s);
    rrow := REF(row);
    a := rl = REF(l);
    b := REF(x) = rx;
    c := rs <> REF(s);
    d := rrow = REF(row);
END_PROGRAM

CONFIGURATION Cfg
    RESOURCE Res ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH T : Main;
    END_RESOURCE
END_CONFIGURATION
