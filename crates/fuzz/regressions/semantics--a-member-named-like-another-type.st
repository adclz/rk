(* rk-fuzz expects, after 1 scans:
P1.f.out = 2
P1.f.e1 = 5
*)

TYPE E0 : INT (n0 := 5, n1 := 7);
END_TYPE

TYPE E1 : INT (n0 := 1, n1 := 2);
END_TYPE

FUNCTION_BLOCK fb
VAR
    e1 : E0 := E0#n0;
END_VAR
VAR_OUTPUT
    out : INT;
END_VAR
    out := E1#n1;
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    f : fb;
END_VAR
    f();
END_PROGRAM

CONFIGURATION Cfg
    RESOURCE R ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH T : Main;
    END_RESOURCE
END_CONFIGURATION
