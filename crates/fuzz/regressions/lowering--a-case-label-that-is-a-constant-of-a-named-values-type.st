(* rk-fuzz expects, after 1 scans:
P1.hit = 1
*)

TYPE Light : INT (Red := 1, Amber := 2, Green := 3);
END_TYPE

PROGRAM Main
VAR CONSTANT
    k : Light := Light#Amber;
END_VAR
VAR
    m : Light := Light#Amber;
    hit : INT;
END_VAR
    CASE m OF
        k: hit := 1;
    ELSE
        hit := 2;
    END_CASE;
END_PROGRAM

CONFIGURATION Cfg
    RESOURCE R ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH T : Main;
    END_RESOURCE
END_CONFIGURATION
