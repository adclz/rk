(* rk-fuzz expects, after 1 scans:
P1.f.m = 3.0
g = 3.0
*)

FUNCTION_BLOCK Fb
VAR CONSTANT
    k0 : REAL := 1.5;
    k1 : REAL := k0 * 2.0;
END_VAR
VAR
    m : REAL := k1;
END_VAR
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    f : Fb;
END_VAR
    f();
END_PROGRAM

CONFIGURATION Cfg
    VAR_GLOBAL CONSTANT
        g0 : REAL := 1.5;
        g1 : REAL := g0 * 2.0;
    END_VAR
    VAR_GLOBAL
        g : REAL := g1;
    END_VAR
    RESOURCE R ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH T : Main;
    END_RESOURCE
END_CONFIGURATION
