(* rk-fuzz expects, after 1 scans:
P1.p = 2.25
g = 8.0
*)

PROGRAM Main
VAR
    p : REAL := 1.5 ** 2.0;
END_VAR
    p := p;
END_PROGRAM

CONFIGURATION Cfg
    VAR_GLOBAL CONSTANT
        k : REAL := 3.0;
    END_VAR
    VAR_GLOBAL
        g : REAL := 2.0 ** k;
    END_VAR
    RESOURCE R ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH T : Main;
    END_RESOURCE
END_CONFIGURATION
